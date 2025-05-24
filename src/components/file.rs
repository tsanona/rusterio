use geo::AffineTransform;
use itertools::Itertools;
use std::{collections::HashMap, fmt::Debug, path::Path};

use crate::{
    components::{raster::RasterBand, DataType},
    errors::Result,
    Indexes,
};

pub trait File: Debug + Sized {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    fn description(&self) -> Result<String>;
    fn size(&self) -> (usize, usize);
    fn crs(&self) -> String;
    fn transform(&self) -> Result<AffineTransform>;
    fn num_bands(&self) -> usize;
    fn band<T: DataType>(&self, index: usize) -> Result<RasterBand<T>>;
    fn bands<T: DataType>(&self, indexes: Indexes, drop: bool) -> Result<Vec<RasterBand<T>>> {
        let mut idxs = indexes.0.into_iter();
        if drop {
            let mut non_dropped_indxs = Vec::from_iter(0..self.num_bands());
            let sorted_indexes = idxs.sorted().enumerate();
            for (shift, idx) in sorted_indexes {
                non_dropped_indxs.remove(idx - shift);
            }
            idxs = non_dropped_indxs.into_iter()
        }
        idxs.map(|idx| self.band(idx)).collect()
    }
    fn metadata(&self) -> HashMap<String, String>;
}
