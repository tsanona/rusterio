use std::{collections::HashMap, fmt::Debug, path::Path, rc::Rc};

use crate::{
    components::{raster::RasterBand, transforms::BandGeoTransform, DataType},
    errors::Result,
    Indexes,
};

pub trait File<T: DataType>: Debug + Sized {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    fn description(&self) -> Result<String>;
    fn size(&self) -> (usize, usize);
    fn crs(&self) -> Rc<str>;
    fn transform(&self) -> Result<BandGeoTransform>;
    fn num_bands(&self) -> usize;
    fn band(&self, index: usize) -> Result<RasterBand<T>>;
    fn bands(&self, indexes: Indexes) -> Result<Vec<RasterBand<T>>> {
        indexes
            .into_iter(self.num_bands())
            .map(|idx| self.band(idx))
            .collect()
    }
    fn metadata(&self) -> HashMap<String, String>;
}
