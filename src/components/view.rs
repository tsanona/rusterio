use rayon::prelude::*;
use std::{collections::HashSet, fmt::Debug, rc::Rc, sync::Arc};

use crate::{
    components::{
        band::{BandInfo, BandReader},
        bounds::{GeoBounds, ViewBounds},
        raster::{RasterBand, RasterGroupInfo},
        transforms::{ViewGeoTransform, ViewReadTransform},
        DataType,
    },
    errors::{Result, RusterioError},
    Buffer,
};

#[derive(Debug, Clone)]
pub struct ViewBand<T: DataType> {
    /// Transform from [RasterView] pixel space to band pixel space.
    transform: ViewReadTransform,
    info: Rc<dyn BandInfo>,
    reader: Arc<dyn BandReader<T>>,
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

pub struct SendSyncBand<T: DataType> {
    transform: ViewReadTransform,
    reader: Arc<dyn BandReader<T>>,
}

impl<T: DataType> From<&ViewBand<T>> for SendSyncBand<T> {
    fn from(value: &ViewBand<T>) -> Self {
        let ViewBand {
            transform, reader, ..
        } = value;
        SendSyncBand {
            transform: *transform,
            reader: Arc::clone(reader),
        }
    }
}

//#[derive(Clone)]
pub struct View<T: DataType> {
    /// Shape of array when read.
    bounds: ViewBounds,
    bands: Rc<[ViewBand<T>]>,
}

impl<T: DataType> Debug for View<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let f = &mut f.debug_struct("View");
        let bands: Vec<String> = self
            .bands
            .iter()
            .map(|view_band| view_band.info.name())
            .collect();
        f.field("bounds", &self.bounds)
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
        selected_bands: Box<[(&RasterGroupInfo, &RasterBand<T>)]>,
    ) -> Result<Self> {
        let view_group_infos: HashSet<&RasterGroupInfo> = selected_bands
            .iter()
            .map(|(group_idx, _)| *group_idx)
            .collect();
        let view_transforms = view_group_infos
            .into_iter()
            .map(|group_info| &group_info.transform);

        let view_bounds = ViewBounds::from(&bounds, view_transforms)?;
        let view_geo_transform = ViewGeoTransform::new(&view_bounds, &bounds)?;

        let bands = Rc::from_iter(selected_bands.iter().map(|(group_info, raster_band)| {
            let transform = ViewReadTransform::new(&view_geo_transform, &group_info.transform);
            ViewBand::from((transform, *raster_band))
        }));
        Ok(Self {
            bounds: view_bounds,
            bands,
        })
    }

    pub fn clip(&self, bounds: ViewBounds) -> Result<Self> {
        let bounds = self.bounds.intersection(&bounds)?;
        let bands = Rc::clone(&self.bands);
        Ok(Self { bounds, bands })
    }

    fn par_bands(&self) -> Box<[SendSyncBand<T>]> {
        self.bands
            .iter()
            .map(|view_band| SendSyncBand::from(view_band))
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

    /// Array shape (C, H, W)
    pub fn shape(&self) -> (usize, usize, usize) {
        let (width, hieght) = self.bounds.shape();
        (self.bands.len(), hieght, width)
    }

    pub fn read(self) -> Result<Buffer<T, 3>> {
        self.to_send_sync().read()
    }

    pub fn to_send_sync(self) -> SendSyncView<T> {
        let bands = Arc::from_iter(self.par_bands());
        let bounds = self.bounds;
        SendSyncView { bounds, bands }
    }
}

pub struct SendSyncView<T: DataType> {
    /// Shape of array when read.
    bounds: ViewBounds,
    bands: Arc<[SendSyncBand<T>]>,
}

impl<T: DataType> SendSyncView<T> {
    pub fn clip(&self, bounds: ViewBounds) -> Result<Self> {
        let bounds = self.bounds.intersection(&bounds)?;
        let bands = Arc::clone(&self.bands);
        Ok(Self { bounds, bands })
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

    pub fn read(&self) -> Result<Buffer<T, 3>> {
        let mut buff = Buffer::new(self.array_shape());
        buff.as_mut_data()
            .par_chunks_mut(self.bounds.num_pixels())
            .zip(self.bands.into_par_iter())
            .map(|(band_buff, read_band)| {
                let read_bounds = self.bounds.to_read_bounds(read_band.transform)?;
                match read_bounds.shape() {
                    (1, 1) => {
                        let mut read_buff = [T::zero()];
                        read_band
                            .reader
                            .read_into_slice(read_bounds, &mut read_buff)?;
                        Ok::<_, RusterioError>(band_buff.fill(read_buff[0]))
                    }
                    shape if shape.eq(&self.bounds.shape()) => {
                        Ok(read_band.reader.read_into_slice(read_bounds, band_buff)?)
                    }
                    shape => {
                        let (view_offset_x, view_offset_y) = self.bounds.offset();
                        let (ratio_x, ratio_y) = read_band.transform.ratio();
                        let view_relative_band_x = ratio_x - (view_offset_x % ratio_x);
                        let view_relative_band_y = ratio_y - (view_offset_y % ratio_y);

                        unimplemented!()
                        /* let ratio = read_band.transform.ratio();
                        let read_max: (usize, usize) = arr_band.dim();
                        Ok(read_band
                            .reader.read_as_array(read_bounds, None)?
                            .into_iter()
                            .enumerate()
                            .map(|(idx, val)| {
                                let (x_slice, y_slice) =
                                    Self::index_slice(idx, shape, ratio, read_max);
                                arr_band.slice_mut(s![x_slice, y_slice]).fill(val);
                            })
                            .collect()) */
                    }
                }
            })
            .collect::<Result<Vec<()>>>()?;
        Ok(buff)
    }

    pub fn bounds_shape(&self) -> (usize, usize) {
        self.bounds.shape()
    }

    /// Array shape (C, H, W)
    pub fn array_shape(&self) -> [usize; 3] {
        let (width, hieght) = self.bounds.shape();
        [self.bands.len(), hieght, width]
    }

    fn num_pixels(&self) -> usize {
        self.array_shape().into_iter().product()
    }
}
