pub mod bounds;
pub mod engines;
pub mod file;
pub mod raster;
pub mod reader;
pub mod view;

pub use bounds::{GeoBounds, PixelBounds};
pub use engines::DataType;
pub use file::File;
pub use raster::Raster;
pub use reader::BandReader;

use std::collections::HashMap;
type Metadata = HashMap<String, String>;
