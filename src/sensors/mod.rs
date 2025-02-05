use std::fmt::Debug;

mod sentinel2;
pub use sentinel2::Sentinel2;

pub trait Sensor: Debug {
    type RasterMetadata: Debug;
    type BandMetadata: Default + Debug;

    const GDAL_DRIVER_NAME: &'static str;
}
