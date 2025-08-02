pub mod band;
pub mod group;

use log::info;
use std::{fmt::Debug, path::Path};

use crate::{
    components::{
        bounds::{Bounds, GeoBounds},
        file::File,
        raster::{
            band::RasterBands,
            group::{RasterGroup, RasterGroupInfo},
        },
        view::View,
        DataType,
    },
    errors::Result,
    intersection::Intersection,
    Indexes,
};

/// Collection of bands that share size,
/// resolution, data type.
pub struct Raster<T: DataType> {
    /// Bounds of full raster
    /// in 'geospace' with raster crs
    bounds: GeoBounds,
    bands: RasterBands<T>,
}

impl<T: DataType> Debug for Raster<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("Raster");
        let bands: Vec<String> = self.bands.iter().map(|band| band.info.name()).collect();
        f.field("geo_bounds", &(self.bounds))
            .field("geo_shape", &(self.bounds.shape()))
            .field("bands", &bands)
            .finish()
    }
}

impl<T: DataType> Raster<T> {
    fn init(bounds: GeoBounds, bands: RasterBands<T>) -> Self {
        let raster = Self { bounds, bands };
        info!("new {raster:?}");
        raster
    }

    pub fn new<F: File<T>>(path: impl AsRef<Path>, band_indexes: Indexes) -> Result<Self> {
        let file = F::open(path)?;

        let transform = file.transform()?;
        let transform = transform.inverse();
        let bounds = file.geo_bounds()?;
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

        Ok(Self::init(bounds, bands))
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
        Ok(Self::init(stack_geo_bounds, stack_bands))
    }

    pub fn view(&self, bounds: Option<GeoBounds>, band_indexes: Indexes) -> Result<View<T>> {
        let mut view_geo_bounds = self.bounds.clone();
        if let Some(geo_bounds) = bounds {
            view_geo_bounds = view_geo_bounds.intersection(&geo_bounds)?
        }

        let view_group_info_bands = band_indexes.select_from(self.bands.group_bands());

        View::new(view_geo_bounds, view_group_info_bands)
    }
}
