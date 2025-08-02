use crate::{
    components::{bounds::ReadBounds, DataType, Metadata},
    errors::Result,
};

/// Trait for accessing name,
/// description and metadata of
/// a raster band.
pub trait BandInfo: std::fmt::Debug {
    fn name(&self) -> String;
    fn description(&self) -> Result<String>;
    fn metadata(&self) -> Result<Metadata>;
}

/// Trait for I/O on a raster band.
pub trait BandReader<T: DataType>: Send + Sync + std::fmt::Debug {
    fn read_into_slice(&self, bounds: &ReadBounds, slice: &mut [T]) -> Result<()>;
}
