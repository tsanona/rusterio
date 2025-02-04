use std::fmt::Debug;

pub mod sentinel2;

pub trait Sensor: Debug {
    type RasterMetadata: Debug;
    type BandMetadata: Default + Debug;

    const GDAL_DRIVER_NAME: &'static str;
}