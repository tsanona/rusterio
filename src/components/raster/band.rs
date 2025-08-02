use std::{fmt::Debug, rc::Rc, sync::Arc};

use crate::components::{
    band::{BandInfo, BandReader},
    raster::group::{RasterGroup, RasterGroupInfo},
    DataType,
};

/// Raster representation of a band.
///
/// Contains [BandInfo] and [BandReader].
#[derive(Debug)]
pub struct RasterBand<T: DataType> {
    pub info: Rc<dyn BandInfo>,
    pub reader: Arc<dyn BandReader<T>>,
}

pub struct RasterBands<T: DataType>(Vec<RasterGroup<T>>);

impl<T: DataType> From<RasterGroup<T>> for RasterBands<T> {
    fn from(value: RasterGroup<T>) -> Self {
        Self(vec![value])
    }
}

impl<T: DataType> RasterBands<T> {
    pub fn iter(&self) -> impl Iterator<Item = &RasterBand<T>> {
        self.0.iter().flat_map(|group| group.bands.iter())
    }

    pub fn append(&mut self, other: &mut RasterBands<T>) {
        self.0.append(other.0.as_mut())
    }

    pub fn groups(&self) -> impl Iterator<Item = &RasterGroup<T>> {
        self.0.iter()
    }

    pub fn group_bands(&self) -> Vec<(&RasterGroupInfo, &RasterBand<T>)> {
        self.groups()
            .flat_map(|group| group.bands.iter().map(move |band| (&group.info, band)))
            .collect()
    }
}
