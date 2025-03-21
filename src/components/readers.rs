use gdal::raster::GdalType;
use ndarray::Array3;
use num::Num;

use crate::errors::Result;

pub trait Reader {
    fn read_window<T: GdalType + Num + Clone + Copy>(
        &self,
        band_indexes: &[usize],
        offset: (usize, usize),
        size: (usize, usize),
    ) -> Result<Array3<T>>;
}
