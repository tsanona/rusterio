use geo::AffineTransform;
use ndarray::{parallel::prelude::*, s, Array2, Array3, Axis};
use std::{fmt::Debug, rc::Rc, sync::Arc};

use crate::{
    cast_tuple,
    components::{band::BandInfo, raster::RasterBand, DataType, PixelBounds},
    errors::Result,
    BandReader,
};

#[derive(Debug, Clone)]
pub struct ViewBand<T: DataType> {
    /// Transform from [RasterView] bounds pixel space to band pixel space.
    transform: AffineTransform,
    info: Rc<Box<dyn BandInfo>>,
    reader: Arc<Box<dyn BandReader<T>>>,
}

impl<T: DataType> From<(AffineTransform, &RasterBand<T>)> for ViewBand<T> {
    fn from(value: (AffineTransform, &RasterBand<T>)) -> Self {
        let (transform, RasterBand { info, reader }) = value;
        ViewBand {
            transform,
            info: Rc::clone(info),
            reader: Arc::clone(reader),
        }
    }
}

pub struct ParBand<T: DataType> {
    transform: AffineTransform,
    reader: Arc<Box<dyn BandReader<T>>>,
}

impl<T: DataType> From<&ViewBand<T>> for ParBand<T> {
    fn from(value: &ViewBand<T>) -> Self {
        let ViewBand {
            transform, reader, ..
        } = value;
        ParBand {
            transform: *transform,
            reader: Arc::clone(reader),
        }
    }
}

impl<T> ParBand<T>
where
    T: DataType,
{
    fn read(&self, bounds: PixelBounds) -> Result<Array2<T>> {
        self.reader.read_window_as_array(
            cast_tuple(bounds.min().x_y())?,
            cast_tuple((bounds.width(), bounds.height()))?,
            None,
        )
    }

    fn ratio(&self) -> (usize, usize) {
        let inv_transform = self.transform.inverse().unwrap();
        (
            inv_transform.a().abs() as usize,
            inv_transform.e().abs() as usize,
        )
    }
}

//#[derive(Clone)]
pub struct RasterView<T: DataType> {
    /// Shape of array when read.
    bounds: PixelBounds,
    bands: Vec<ViewBand<T>>,
}

impl<T: DataType> Debug for RasterView<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("RasterView");
        let bands: Vec<String> = self
            .bands
            .iter()
            .map(|view_band| view_band.info.name())
            .collect();
        f.field("pixel_shape", &(self.bounds.width(), self.bounds.height()))
            .field("bands", &bands)
            .finish()
    }
}

impl<T> RasterView<T>
where
    T: DataType,
{
    pub fn new(bounds: PixelBounds, bands: Vec<ViewBand<T>>) -> Self {
        Self { bounds, bands }
    }

    pub fn clip(mut self, bounds: PixelBounds) -> Result<Self> {
        let original_bounds = self.bounds;
        self.bounds = original_bounds.intersection(&bounds)?;
        Ok(self)
    }

    fn par_bands(&self) -> Vec<ParBand<T>> {
        self.bands
            .iter()
            .map(|view_band| ParBand::from(view_band))
            .collect()
    }

    fn index_slice(
        index: usize,
        bounds: (usize, usize),
        ratio: (usize, usize),
        read_max: (usize, usize),
    ) -> (core::ops::Range<usize>, core::ops::Range<usize>) {
        let (bounds_x, bounds_y) = bounds;
        let (ratio_x, ratio_y) = ratio;
        let (x, y) = (bounds_x - index % bounds_x, bounds_y - index / bounds_y);
        let x_slice = ((x - 1) * ratio_x)..(x * ratio_x).min(read_max.0);
        let y_slice = ((y - 1) * ratio_y)..(y * ratio_y).min(read_max.1);
        (x_slice, y_slice)
    }

    // TODO: add masking
    pub fn read(&self /* , mask: Option<ArrayView2<bool>> */) -> Result<Array3<T>> {
        let mut array = Array3::zeros(self.shape());
        let read_shape = self.bounds.shape();
        let read_bounds = &self.bounds;
        let read_bands = self.par_bands();
        let errors: Result<()> = array
            .axis_iter_mut(Axis(0))
            .into_par_iter()
            .zip(read_bands)
            .map(|(mut arr_band, read_band)| {
                let read_bounds = read_bounds.affine_transform(&read_band.transform)?;
                match read_bounds.shape() {
                    (1, 1) => Ok(arr_band.fill(read_band.read(read_bounds)?[[0, 0]])),
                    shape if shape.eq(&read_shape) => {
                        Ok(arr_band.assign(&read_band.read(read_bounds)?))
                    }
                    shape => {
                        let ratio = read_band.ratio();
                        let read_max: (usize, usize) = arr_band.dim();
                        Ok(read_band
                            .read(read_bounds)?
                            .into_iter()
                            .enumerate()
                            .map(|(idx, val)| {
                                let (x_slice, y_slice) =
                                    Self::index_slice(idx, shape, ratio, read_max);
                                arr_band.slice_mut(s![x_slice, y_slice]).fill(val);
                            })
                            .collect())
                    }
                }
            })
            .collect();
        errors.map(|_| array)
    }

    pub fn shape(&self) -> (usize, usize, usize) {
        let (width, hight) = self.bounds.shape();
        (self.bands.len(), width, hight)
    }
}
