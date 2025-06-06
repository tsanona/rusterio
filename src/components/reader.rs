use ndarray::{Array2, ArrayView2};

use crate::{components::DataType, errors::Result};

pub trait BandReader<T: DataType>: Send + Sync + std::fmt::Debug {
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
