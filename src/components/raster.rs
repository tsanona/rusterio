use geo::{AffineOps, AffineTransform, Rect};
use num::Integer;
use std::{collections::HashSet, fmt::Debug, sync::Arc};

use crate::{
    cast_tuple,
    components::{view::RasterView, BandReader, DataType, File, GeoBounds, Metadata},
    errors::Result,
    Indexes,
};

use super::{view::ViewBand, PixelBounds};

#[derive(Debug)]
pub struct RasterBand<T: DataType> {
    pub description: String,
    pub name: String,
    pub metadata: Metadata,
    //chunk_size: (usize, usize),
    //data_type: String,
    pub reader: Arc<Box<dyn BandReader<T>>>,
}

#[derive(Debug)]
struct RasterGroupInfo {
    pub description: String,
    /// Geo to Pix transform.
    /// Affine transform from `bounds` to pixel coordinates.
    pub transform: AffineTransform,
    pub metadata: Metadata,
}

impl RasterGroupInfo {
    pub fn resolution(&self) -> (f64, f64) {
        let inv_trans = self.transform.inverse().unwrap();
        (inv_trans.a(), inv_trans.b())
    }
}

struct RasterGroup<T: DataType> {
    info: RasterGroupInfo,
    bands: Vec<RasterBand<T>>,
}

struct RasterBands<T: DataType>(Vec<RasterGroup<T>>);

impl<T: DataType> From<RasterGroup<T>> for RasterBands<T> {
    fn from(value: RasterGroup<T>) -> Self {
        Self(vec![value])
    }
}

impl<T: DataType> RasterBands<T> {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn iter(&self) -> impl Iterator<Item = &RasterBand<T>> {
        self.0.iter().flat_map(|group| group.bands.iter())
    }

    fn num_bands(&self) -> usize {
        self.groups()
            .fold(0, |sum, RasterGroup { info: _, bands }| sum + bands.len())
    }

    fn group(&self, index: usize) -> &RasterGroup<T> {
        &self.0[index]
    }

    fn groups(&self) -> impl Iterator<Item = &RasterGroup<T>> {
        self.0.iter()
    }

    fn append(&mut self, other: &mut RasterBands<T>) {
        self.0.append(other.0.as_mut())
    }

    fn zipped(&self) -> Vec<(usize, &RasterBand<T>)> {
        self.groups()
            .enumerate()
            .flat_map(|(idx, group)| group.bands.iter().map(move |band| (idx, band)))
            .collect()
    }
}

#[derive(Debug)]
struct PixelSapce {
    bounds: PixelBounds,
    transform: AffineTransform,
}

impl PixelSapce {
    fn from<'a>(
        bounds: &GeoBounds,
        transforms: impl Iterator<Item = &'a AffineTransform>,
    ) -> Result<Self> {
        let result_pixel_bounds: Result<Vec<PixelBounds>> = transforms
            .into_iter()
            .map(|transform| PixelBounds::try_from(bounds.geometry.affine_transform(transform)))
            .collect();
        let mut pixel_bounds = result_pixel_bounds?;
        let mut raster_pixel_shape = pixel_bounds.pop().unwrap().max().x_y();
        for pixel_shape in pixel_bounds.into_iter().map(|bounds| bounds.max().x_y()) {
            raster_pixel_shape = (
                raster_pixel_shape.0.lcm(&pixel_shape.0),
                raster_pixel_shape.1.lcm(&pixel_shape.1),
            )
        }
        let transform = AffineTransform::new(
            bounds.geometry.width() / (raster_pixel_shape.0 as f64),
            0.,
            bounds.geometry.min().x,
            0.,
            bounds.geometry.height() / (raster_pixel_shape.1 as f64),
            bounds.geometry.min().y,
        );
        Ok(Self {
            bounds: Rect::new((0, 0), raster_pixel_shape).try_into()?,
            transform,
        })
    }
}

/// Collection of bands that share size,
/// resolution, data type.
pub struct Raster<T: DataType> {
    /// Bounds in 'geospace' with raster crs
    /// such that,
    /// when projected to pixel coordinates (with transform),
    /// `min @ (0, 0)` and `max @ array_size`.
    bounds: GeoBounds,
    //pixel_bounds: PixelBounds,
    bands: RasterBands<T>,
}

impl<T: DataType> Debug for Raster<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("Raster");
        let bands: Vec<&String> = self.bands.iter().map(|band| &band.name).collect();
        let pixel_space = PixelSapce::from(
            &self.bounds,
            self.bands.groups().map(|group| &group.info.transform),
        )
        .unwrap();
        f.field("geo rect", &(self.bounds.geometry))
            .field("geo shape", &(self.bounds.shape()))
            .field("pixel space", &pixel_space)
            .field("bands", &bands)
            .finish()
    }
}

impl<T: DataType> Raster<T> {
    pub fn new<F: File>(file: F, band_indexes: Indexes, drop: bool) -> Result<Self> {
        //let file = F::open(path)?;

        let transform = file.transform()?;
        let pixel_bounds_rect = Rect::new((0., 0.), cast_tuple(file.size())?);
        let geo_bounds_rect = pixel_bounds_rect.affine_transform(&transform);
        let transform = transform.inverse().unwrap();

        let crs = file.crs();
        let bounds = (crs, geo_bounds_rect).into();

        let description = file.description()?;
        let metadata = file.metadata();
        let info = RasterGroupInfo {
            description,
            transform,
            metadata,
        };
        let raster_bands = file.bands(band_indexes, drop)?;
        let bands = RasterBands::from(RasterGroup {
            info,
            bands: raster_bands,
        });

        //TODO: assert!(bands.datatype == T)

        Ok(Self { bounds, bands })
    }

    pub fn stack(rasters: Vec<Raster<T>>) -> Result<Raster<T>> {
        let mut stack_iter = rasters
            .into_iter()
            .map(|raster| (raster.bounds, raster.bands));
        let (mut stack_geo_bounds, mut stack_bands) = stack_iter.next().unwrap();
        for (geo_bounds, mut bands) in stack_iter {
            stack_geo_bounds = stack_geo_bounds.intersection(&geo_bounds)?;
            stack_bands.append(&mut bands);
        }
        Ok(Raster {
            bounds: stack_geo_bounds,
            bands: stack_bands,
        })
    }

    pub fn view(
        &self,
        bounds: Option<GeoBounds>,
        band_indexes: Indexes,
        drop: bool,
    ) -> Result<RasterView<T>> {
        let mut view_geo_bounds = self.bounds.clone();
        if let Some(geo_bounds) = bounds {
            view_geo_bounds = view_geo_bounds.intersection(&geo_bounds)?
        }

        let view_band_idx = band_indexes.into_iter(self.bands.num_bands(), drop);

        let view_zipped_bands: Vec<(usize, &RasterBand<T>)> = view_band_idx
            .into_iter()
            .map(|idx| self.bands.zipped()[idx])
            .collect();

        let view_group_ids: HashSet<usize> = view_zipped_bands
            .iter()
            .map(|(group_idx, _)| *group_idx)
            .collect();
        let view_transforms = view_group_ids
            .into_iter()
            .map(|idx| &self.bands.group(idx).info.transform);

        let view_pixel_space = PixelSapce::from(&view_geo_bounds, view_transforms)?;
        let bands = view_zipped_bands
            .into_iter()
            .map(|(group_idx, raster_band)| {
                let transform = view_pixel_space
                    .transform
                    .compose(&self.bands.group(group_idx).info.transform);
                ViewBand::from((transform, raster_band))
            })
            .collect();
        Ok(RasterView::new(view_pixel_space.bounds, bands))
    }
}
