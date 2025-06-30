use ndarray::{Array2, ArrayView2};

use crate::{
    components::{bounds::ReadBounds, DataType, Metadata},
    errors::Result,
};

pub trait BandInfo: std::fmt::Debug {
    fn description(&self) -> Result<String>;
    fn name(&self) -> String;
    fn metadata(&self) -> Result<Metadata>;
}

pub trait BandReader<T: DataType>: Send + Sync + std::fmt::Debug {
    fn read_as_array(
        &self,
        bounds: ReadBounds,
        mask: Option<ArrayView2<bool>>, //TODO: rethink this
    ) -> Result<Array2<T>>;

    fn read_into_slice(
        &self,
        bounds: ReadBounds,
        slice: &mut [T]
    ) -> Result<()>;

    /* fn read_block_as_array(
        &self,
        index: (usize, usize),
        band_index: usize,
        mask: &Option<ArrayView2<'a, bool>>,
    ) -> Result<Array2<T>>; */
}
