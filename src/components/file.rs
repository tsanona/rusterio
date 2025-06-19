use geo::AffineTransform;
use std::{collections::HashMap, fmt::Debug, path::Path};

use crate::{
    components::{engines::DataType, raster::RasterBand},
    errors::Result,
    Indexes,
};

pub trait File<T: DataType>: Debug + Sized {
    //type T: DataType;
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    fn description(&self) -> Result<String>;
    fn size(&self) -> (usize, usize);
    fn crs(&self) -> String;
    fn transform(&self) -> Result<AffineTransform>;
    fn num_bands(&self) -> usize;
    fn band(&self, index: usize) -> Result<RasterBand<T>>;
    fn bands(&self, indexes: Indexes, drop: bool) -> Result<Vec<RasterBand<T>>> {
        indexes
            .into_iter(self.num_bands(), drop)
            .map(|idx| self.band(idx))
            .collect()
    }
    fn metadata(&self) -> HashMap<String, String>;
}
