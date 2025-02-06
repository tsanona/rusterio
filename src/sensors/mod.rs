#![allow(unused_imports)]

use std::fmt::Debug;

mod sentinel2;
pub use sentinel2::Sentinel2;

pub trait Sensor {
    type RasterMetadata: Debug;
    type BandMetadata: Debug + Default;

    const GDAL_DRIVER_NAME: &'static str;
}
