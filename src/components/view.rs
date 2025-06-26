use ndarray::{parallel::prelude::*, s, Array2, Array3, Axis};
use num::Integer;
use std::{collections::HashSet, fmt::Debug, rc::Rc, sync::Arc};

use crate::{
    components::{
        band::{BandInfo, BandReader},
        bounds::{GeoBounds, ReadBounds, ViewBounds},
        raster::{RasterBand, RasterGroupInfo},
        transforms::{ViewBandTransform, ViewGeoTransform},
        DataType,
    },
    errors::Result,
};

#[derive(Debug, Clone)]
pub struct ViewBand<T: DataType> {
    /// Transform from [RasterView] pixel space to band pixel space.
    transform: ViewBandTransform,
    info: Rc<Box<dyn BandInfo>>,
    reader: Arc<Box<dyn BandReader<T>>>,
}

impl<T: DataType> From<(ViewBandTransform, &RasterBand<T>)> for ViewBand<T> {
    fn from(value: (ViewBandTransform, &RasterBand<T>)) -> Self {
        let (transform, RasterBand { info, reader }) = value;
        ViewBand {
            transform,
            info: Rc::clone(info),
            reader: Arc::clone(reader),
        }
    }
}

pub struct ParBand<T: DataType> {
    transform: ViewBandTransform,
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
    fn read(&self, bounds: ReadBounds) -> Result<Array2<T>> {
        self.reader
            .read_window_as_array(bounds.offset(), bounds.shape(), None)
    }
}

//#[derive(Clone)]
pub struct View<T: DataType> {
    /// Shape of array when read.
    bounds: ViewBounds,
    bands: Vec<ViewBand<T>>,
}

impl<T: DataType> Debug for View<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("View");
        let bands: Vec<String> = self
            .bands
            .iter()
            .map(|view_band| view_band.info.name())
            .collect();
        f.field("pixel_shape", &self.bounds.shape())
            .field("bands", &bands)
            .finish()
    }
}

impl<T> View<T>
where
    T: DataType,
{
    pub fn new(
        bounds: GeoBounds,
        selected_bands: Vec<(&RasterGroupInfo, &RasterBand<T>)>,
    ) -> Result<Self> {
        let view_group_infos: HashSet<&RasterGroupInfo> = selected_bands
            .iter()
            .map(|(group_idx, _)| *group_idx)
            .collect();
        let view_transforms = view_group_infos
            .into_iter()
            .map(|group_info| &group_info.transform);

        let mut band_bounds = view_transforms
            .into_iter()
            .map(|transform| ReadBounds::new(&bounds, transform))
            .collect::<Result<Vec<ReadBounds>>>()?;
        let mut view_pixel_shape = band_bounds.pop().unwrap().shape();
        for band_pixel_shape in band_bounds.into_iter().map(|bounds| bounds.shape()) {
            view_pixel_shape = (
                view_pixel_shape.0.lcm(&band_pixel_shape.0),
                view_pixel_shape.1.lcm(&band_pixel_shape.1),
            )
        }
        let view_geo_transform = ViewGeoTransform::new(&bounds, view_pixel_shape);
        let view_bounds = ViewBounds::new((0, 0), view_pixel_shape);

        let bands = selected_bands
            .into_iter()
            .map(|(group_info, raster_band)| {
                let transform = ViewBandTransform::new(&view_geo_transform, &group_info.transform);
                ViewBand::from((transform, raster_band))
            })
            .collect();
        Ok(Self {
            bounds: view_bounds,
            bands,
        })
    }

    pub fn clip(mut self, bounds: ViewBounds) -> Result<Self> {
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
        let (bounds_y, bounds_x) = bounds;
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
        let view_bounds = &self.bounds;
        let read_bands = self.par_bands();
        let errors: Result<()> = array
            .axis_iter_mut(Axis(0))
            .into_par_iter()
            .zip(read_bands)
            .map(|(mut arr_band, read_band)| {
                let read_bounds = view_bounds.to_read_bounds(read_band.transform)?;
                match read_bounds.shape() {
                    (1, 1) => Ok(arr_band.fill(read_band.read(read_bounds)?[[0, 0]])),
                    shape if shape.eq(&read_shape) => {
                        Ok(arr_band.assign(&read_band.read(read_bounds)?))
                    }
                    shape => {
                        let ratio = read_band.transform.ratio();
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

    /// Array shape (C, H, W)
    pub fn shape(&self) -> (usize, usize, usize) {
        let (width, hieght) = self.bounds.shape();
        (self.bands.len(), hieght, width)
    }
}
