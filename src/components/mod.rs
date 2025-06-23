pub mod band;
pub mod bounds;
pub mod engines;
pub mod file;
pub mod raster;
pub mod view;

pub use band::BandReader;
pub use bounds::{GeoBounds, PixelBounds};
pub use engines::DataType;
pub use file::File;
pub use raster::Raster;

use std::collections::HashMap;
type Metadata = HashMap<String, String>;
