use gdal::raster::GdalType;
use ndarray::{Array2, ArrayView2};
use num::Num;

use crate::errors::Result;

pub trait BandReader<T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync>:
    Send + Sync
{
    fn read_window_as_array(
        &self,
        // Position of bottom left corner.
        offset: (usize, usize),
        size: (usize, usize),
        mask: Option<ArrayView2<bool>>,
    ) -> Result<Array2<T>>;

    /* fn read_block_as_array(
        &self,
        index: (usize, usize),
        band_index: usize,
        mask: &Option<ArrayView2<'a, bool>>,
    ) -> Result<Array2<T>>; */
}
