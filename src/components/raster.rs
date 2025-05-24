use geo::{AffineOps, AffineTransform, Rect};
use itertools::Itertools;
use num::Integer;
use std::{collections::HashMap, fmt::Debug, sync::Arc};

use crate::{
    cast_tuple,
    components::{view::RasterView, BandReader, DataType, File, GeoBounds, Metadata},
    errors::Result,
    Indexes,
};

use super::{view::ViewBand, PixelBounds};

#[derive(Debug, derive_new::new)]
pub struct RasterBand<T: DataType> {
    pub description: String,
    pub name: String,
    pub metadata: Metadata,
    //chunk_size: (usize, usize),
    //data_type: String,
    pub reader: Arc<Box<dyn BandReader<T>>>,
}

#[derive(Debug)]
pub struct RasterGroupInfo {
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

type BandOrder = HashMap<usize, (usize, usize)>;

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
struct RasterGroups<T: DataType>(Vec<RasterGroup<T>>);

impl<T: DataType> From<RasterGroup<T>> for RasterGroups<T> {
    fn from(value: RasterGroup<T>) -> Self {
        Self(vec![value])
    }
}

impl<T: DataType> RasterGroups<T> {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn group_indexed_raster_bands(&self) -> Vec<(usize, &RasterBand<T>)> {
        self.iter()
            .enumerate()
            .flat_map(|(idx, group)| group.bands.iter().map(move |band| (idx, band)))
            .collect()
    }

    fn num_bands(&self) -> usize {
        self.iter()
            .fold(0, |sum, RasterGroup { info: _, bands }| sum + bands.len())
    }

    fn raster_bands(&self) -> Vec<&RasterBand<T>> {
        self.iter().flat_map(|group| group.bands.iter()).collect()
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
    groups: RasterGroups<T>,
}

impl<T: DataType> Debug for Raster<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("Raster");
        let bands: Vec<&String> = self
            .groups
            .raster_bands()
            .into_iter()
            .map(|band| &band.name)
            .collect();
        f.field("geo rect", &(self.bounds.geometry))
            .field("geo shape", &(self.bounds.shape()))
            .field(
                "pixel shape",
                &Self::pixel_bounds(
                    &self.bounds,
                    self.groups.iter().map(|group| &group.info).collect(),
                )
                .unwrap(),
            )
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
        let groups = RasterGroups::from(RasterGroup {
            info,
            bands: raster_bands,
        });

        //TODO: assert!(bands.datatype == T)

        Ok(Self { bounds, groups })
    }

    pub fn stack(rasters: Vec<Raster<T>>) -> Result<Raster<T>> {
        let mut stack_iter = rasters
            .into_iter()
            .map(|raster| (raster.bounds, raster.groups));
        let (mut stack_geo_bounds, mut stack_groups) = stack_iter.next().unwrap();
        for (geo_bounds, mut groups) in stack_iter {
            stack_geo_bounds = stack_geo_bounds.intersection(&geo_bounds)?;
            stack_groups.append(groups.as_mut());
        }
        Ok(Raster {
            bounds: stack_geo_bounds,
            groups: stack_groups,
        })
    }

    fn pixel_bounds(
        geo_bounds: &GeoBounds,
        groups_info: Vec<&RasterGroupInfo>,
    ) -> Result<(PixelBounds, AffineTransform)> {
        let result_pixel_bounds: Result<Vec<PixelBounds>> = groups_info
            .into_iter()
            .map(|group_info| {
                PixelBounds::try_from(geo_bounds.geometry.affine_transform(&group_info.transform))
            })
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
            geo_bounds.geometry.width() / (raster_pixel_shape.0 as f64),
            0.,
            geo_bounds.geometry.min().x,
            0.,
            geo_bounds.geometry.height() / (raster_pixel_shape.1 as f64),
            geo_bounds.geometry.min().y,
        );
        Ok((Rect::new((0, 0), raster_pixel_shape).try_into()?, transform))
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
        let mut view_band_idx = band_indexes.0.into_iter();
        if drop {
            let mut non_dropped_indxs = Vec::from_iter(0..self.groups.num_bands());
            let sorted_indexes = view_band_idx.sorted().enumerate();
            for (shift, idx) in sorted_indexes {
                non_dropped_indxs.remove(idx - shift);
            }
            view_band_idx = non_dropped_indxs.into_iter()
        }

        let view_zipped: Vec<(usize, &RasterBand<T>)> = view_band_idx
            .into_iter()
            .map(|idx| self.groups.group_indexed_raster_bands()[idx])
            .collect();
        let view_group_infos = view_zipped
            .iter()
            .map(|(group_index, _)| &self.groups[*group_index].info)
            .collect();
        let (view_pixel_bounds, pix_geo_transform) =
            Self::pixel_bounds(&view_geo_bounds, view_group_infos)?;
        let bands = view_zipped
            .into_iter()
            .map(|(group_idx, raster_band)| {
                let transform = pix_geo_transform.compose(&self.groups[group_idx].info.transform);
                ViewBand::from((transform, raster_band))
            })
            .collect();
        Ok(RasterView::new(view_pixel_bounds, bands))
    }
}
