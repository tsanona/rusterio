use geo::AffineTransform;
use ndarray::{parallel::prelude::*, s, Array2, Array3, Axis};
use std::fmt::Debug;

use crate::{
    cast_tuple,
    components::{raster::RasterBand, DataType, PixelBounds},
    errors::Result,
};

#[derive(Debug, Clone)]
pub struct ViewBand<'a, T: DataType> {
    /// Transform from [RasterView] bounds pixel space to band pixel space
    transform: AffineTransform,
    raster_band: &'a RasterBand<T>,
}

impl<'a, T: DataType> From<(AffineTransform, &'a RasterBand<T>)> for ViewBand<'a, T> {
    fn from(value: (AffineTransform, &'a RasterBand<T>)) -> Self {
        let (transform, raster_band) = value;
        ViewBand {
            transform,
            raster_band,
        }
    }
}

impl<'a, T> ViewBand<'a, T>
where
    T: DataType,
{
    fn read(&self, bounds: PixelBounds) -> Result<Array2<T>> {
        self.raster_band.reader.read_window_as_array(
            cast_tuple(bounds.min().x_y())?,
            cast_tuple((bounds.width(), bounds.height()))?,
            None,
        )
    }
}

#[derive(Clone)]
pub struct RasterView<'a, T: DataType> {
    /// Shape of array when read.
    bounds: PixelBounds,
    bands: Vec<ViewBand<'a, T>>,
}

impl<'a, T: DataType> Debug for RasterView<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("RasterView");
        let bands: Vec<&String> = self
            .bands
            .iter()
            .map(|view_band| &view_band.raster_band.name)
            .collect();
        f.field("pixel_shape", &(self.bounds.width(), self.bounds.height()))
            .field("bands", &bands)
            .finish()
    }
}

impl<'a, T> RasterView<'a, T>
where
    T: DataType,
{
    pub fn new(bounds: PixelBounds, bands: Vec<ViewBand<'a, T>>) -> Self {
        Self { bounds, bands }
    }

    pub fn clip(mut self, bounds: PixelBounds) -> Result<Self> {
        let original_bounds = self.bounds;
        self.bounds = original_bounds.intersection(&bounds)?;
        Ok(self)
    }

    // TODO: add masking
    pub fn read(&self /* , mask: Option<ArrayView2<'a, bool>> */) -> Result<Array3<T>> {
        let mut array = Array3::zeros(self.shape());
        let errors: Result<()> = array
            .axis_iter_mut(Axis(0))
            .into_par_iter()
            .zip(&self.bands)
            .map(|(mut arr_band, band)| {
                let band_bounds = self.bounds.affine_transform(&band.transform)?;
                match band_bounds.shape() {
                    (1, 1) => Ok(arr_band.fill(band.read(band_bounds)?[[0, 0]])),
                    shape if shape.eq(&self.bounds.shape()) => Ok(arr_band.assign(&band.read(band_bounds)?)),
                    (band_x, band_y) => {
                        let inv_transform = band.transform.inverse().unwrap();
                        let (band_ratio_x, band_ratio_y) = (
                            inv_transform.a().abs() as usize,
                            inv_transform.e().abs() as usize,
                        );
                        let (arr_band_x, arr_band_y) = arr_band.dim();
                        Ok(band.read(band_bounds)?.into_iter().enumerate().map(|(idx, val)| {
                            let (x, y) = (band_x - idx % band_x, band_y - idx / band_y);
                            let x_slice = ((x - 1) * band_ratio_x)..(x * band_ratio_x).min(arr_band_x);
                            let y_slice = ((y - 1) * band_ratio_y)..(y * band_ratio_y).min(arr_band_y);
                            arr_band.slice_mut(s![x_slice, y_slice]).fill(val);
                        }).collect())
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
