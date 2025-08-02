use std::{fmt::Debug, hash::Hash};

use crate::components::{
    raster::band::RasterBand, transforms::GeoReadTransform, DataType, Metadata,
};

/// Info for [RasterGroup].
#[derive(Debug)]
pub struct RasterGroupInfo {
    pub description: String,
    pub transform: GeoReadTransform,
    pub metadata: Metadata,
}

impl Hash for &RasterGroupInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let ptr: *const RasterGroupInfo = *self;
        ptr.hash(state);
    }
}

impl PartialEq for &RasterGroupInfo {
    fn eq(&self, other: &Self) -> bool {
        let lh_ptr: *const RasterGroupInfo = *self;
        let rh_ptr: *const RasterGroupInfo = *other;
        lh_ptr.eq(&rh_ptr)
    }
}

impl Eq for &RasterGroupInfo {}

impl RasterGroupInfo {
    pub fn resolution(&self) -> (f64, f64) {
        (self.transform.a(), self.transform.b())
    }
}

/// Collection or [RasterBand] that share the same [RasterGroupInfo]
pub struct RasterGroup<T: DataType> {
    pub info: RasterGroupInfo,
    pub bands: Box<[RasterBand<T>]>,
}
