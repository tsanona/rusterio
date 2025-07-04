pub mod band;
pub mod bounds;
pub mod engines;
pub mod file;
pub mod raster;
pub mod transforms;
pub mod view;

type Metadata = std::collections::HashMap<String, String>;

pub trait DataType: num::Num + From<bool> + Clone + Copy + Send + Sync + std::fmt::Debug {}
impl DataType for u16 {}
