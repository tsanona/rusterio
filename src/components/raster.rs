use geo::{AffineOps, Rect};
use std::{fmt::Debug, hash::Hash, path::Path, rc::Rc, sync::Arc};

use crate::{
    cast_tuple,
    components::{
        band::{BandInfo, BandReader},
        bounds::GeoBounds,
        file::File,
        transforms::GeoBandTransform,
        view::View,
        DataType, Metadata,
    },
    errors::Result,
    Indexes,
};

#[derive(Debug)]
pub struct RasterBand<T: DataType> {
    pub info: Rc<Box<dyn BandInfo>>,
    pub reader: Arc<Box<dyn BandReader<T>>>,
}

#[derive(Debug)]
pub struct RasterGroupInfo {
    pub description: String,
    pub transform: GeoBandTransform,
    pub metadata: Metadata,
}

impl Hash for &RasterGroupInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let ptr: *const RasterGroupInfo = *self;
        ptr.hash(state);
    }
}

impl PartialEq for &RasterGroupInfo {
    fn eq(&self, other: &Self) -> bool {
        let lh_ptr: *const RasterGroupInfo = *self;
        let rh_ptr: *const RasterGroupInfo = *other;
        lh_ptr.eq(&rh_ptr)
    }
}

impl Eq for &RasterGroupInfo {}

impl RasterGroupInfo {
    pub fn resolution(&self) -> (f64, f64) {
        (self.transform.a(), self.transform.b())
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
    fn groups(&self) -> impl Iterator<Item = &RasterGroup<T>> {
        self.0.iter()
    }

    fn iter(&self) -> impl Iterator<Item = &RasterBand<T>> {
        self.0.iter().flat_map(|group| group.bands.iter())
    }

    fn num_bands(&self) -> usize {
        self.groups()
            .fold(0, |sum, RasterGroup { info: _, bands }| sum + bands.len())
    }

    fn append(&mut self, other: &mut RasterBands<T>) {
        self.0.append(other.0.as_mut())
    }

    fn group_bands(&self) -> Vec<(&RasterGroupInfo, &RasterBand<T>)> {
        self.groups()
            .flat_map(|group| group.bands.iter().map(move |band| (&group.info, band)))
            .collect()
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
    bands: RasterBands<T>,
}

impl<T: DataType> Debug for Raster<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("Raster");
        let bands: Vec<String> = self.bands.iter().map(|band| band.info.name()).collect();
        f.field("geo rect", &(self.bounds.geometry))
            .field("geo shape", &(self.bounds.shape()))
            .field("bands", &bands)
            .finish()
    }
}

impl<T: DataType> Raster<T> {
    pub fn new<F: File<T>, P: AsRef<Path>>(
        path: P,
        band_indexes: Indexes,
    ) -> Result<Self> {
        let file = F::open(path)?;

        let transform = file.transform()?;
        let pixel_bounds_rect = Rect::new((0., 0.), cast_tuple(file.size())?);
        let geo_bounds_rect = pixel_bounds_rect.affine_transform(&transform);
        let transform = transform.inverse();

        let crs = file.crs();
        let bounds = (crs, geo_bounds_rect).into();

        let description = file.description()?;
        let metadata = file.metadata();
        let info = RasterGroupInfo {
            description,
            transform,
            metadata,
        };
        let raster_bands = file.bands(band_indexes)?;
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
        band_indexes: Indexes
    ) -> Result<View<T>> {
        let mut view_geo_bounds = self.bounds.clone();
        if let Some(geo_bounds) = bounds {
            view_geo_bounds = view_geo_bounds.intersection(&geo_bounds)?
        }

        let view_group_info_bands = band_indexes.select_from(self.bands.group_bands());

        View::new(view_geo_bounds, view_group_info_bands)
    }
}
