use std::{collections::HashMap, fmt::Debug, path::Path};

use crate::{
    components::{bounds::GeoBounds, raster::RasterBand, transforms::ReadGeoTransform, DataType},
    errors::Result,
    indexes::Indexes,
};

pub trait File<T: DataType>: Debug + Sized {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    fn description(&self) -> Result<String>;
    fn geo_bounds(&self) -> Result<GeoBounds>;
    fn transform(&self) -> Result<ReadGeoTransform>;
    fn num_bands(&self) -> usize;
    fn band(&self, index: usize) -> Result<RasterBand<T>>;
    fn bands(&self, indexes: Indexes) -> Result<Box<[RasterBand<T>]>> {
        indexes
            .indexes_from(self.num_bands())
            .iter()
            .map(|idx| self.band(*idx))
            .collect()
    }
    fn metadata(&self) -> HashMap<String, String>;
}
