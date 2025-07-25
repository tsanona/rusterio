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
    fn read_into_slice(&self, bounds: &ReadBounds, slice: &mut [T]) -> Result<()>;
}
