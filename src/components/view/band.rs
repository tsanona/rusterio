use std::{fmt::Debug, rc::Rc, sync::Arc};

use crate::{
    components::{
        band::{BandInfo, BandReader},
        raster::band::RasterBand,
        transforms::ViewReadTransform,
        DataType,
    }
};

#[derive(Debug, Clone)]
pub struct ViewBand<T: DataType> {
    pub info: Rc<dyn BandInfo>,
    /// Transform from [RasterView] pixel space to band pixel space.
    pub transform: ViewReadTransform,
    pub reader: Arc<dyn BandReader<T>>,
}

impl<T: DataType> From<(ViewReadTransform, &RasterBand<T>)> for ViewBand<T> {
    fn from(value: (ViewReadTransform, &RasterBand<T>)) -> Self {
        let (transform, RasterBand { info, reader }) = value;
        ViewBand {
            transform,
            info: Rc::clone(info),
            reader: Arc::clone(reader),
        }
    }
}

pub struct ReadBand<T: DataType> {
    pub transform: ViewReadTransform,
    pub reader: Arc<dyn BandReader<T>>,
}

impl<T: DataType> From<&ViewBand<T>> for ReadBand<T> {
    fn from(value: &ViewBand<T>) -> Self {
        let ViewBand {
            transform, reader, ..
        } = value;
        ReadBand {
            transform: *transform,
            reader: Arc::clone(reader),
        }
    }
}