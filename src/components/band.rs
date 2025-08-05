use geo::Coord;

use crate::{
    components::{bounds::ReadBounds, DataType, Metadata},
    errors::Result,
    Buffer,
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
    fn read_to_buffer(&self, bounds: &ReadBounds) -> Result<Buffer<T, 1>>; // TODO: add default impl
    fn read_pixel(&self, offset: Coord<usize>) -> Result<T>;
}
