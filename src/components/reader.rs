use gdal::raster::GdalType;
use ndarray::Array2;
use num::Num;

use crate::errors::Result;

pub trait Reader: Send + Sync {
    fn read_band_window_as_array<T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync>(
        &self,
        band_index: usize,
        offset: (isize, isize),
        size: (usize, usize),
        mask: &Option<Array2<bool>>,
    ) -> Result<Array2<T>>;

    fn read_band_block_as_array<T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync>(
        &self,
        index: (usize, usize),
        band_index: usize,
        mask: &Option<Array2<bool>>,
    ) -> Result<Array2<T>>;
}
