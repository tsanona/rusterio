use geo::{Coord, MapCoords};
use rayon::prelude::*;
use std::{collections::HashSet, fmt::Debug, rc::Rc, sync::Arc};
use log::info;

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
    fn init(bounds: ViewBounds, bands: Rc<[ViewBand<T>]>) -> Self {
        let view = Self {bounds, bands};
        info!("new {view:?}");
        view
    }

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
        Ok(Self::init(view_bounds, bands))
    }

    pub fn clip(&self, bounds: ViewBounds) -> Result<Self> {
        let bounds = self.bounds.intersection(&bounds)?;
        let bands = Rc::clone(&self.bands);
        Ok(Self::init(bounds, bands))
    }

    fn par_bands(&self) -> Box<[SendSyncBand<T>]> {
        self.bands
            .iter()
            .map(|view_band| SendSyncBand::from(view_band))
            .collect()
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

    pub fn read(&self) -> Result<Buffer<T, 3>> {
        let mut buff = Buffer::new(self.array_shape());
        buff.as_mut_data()
            .par_chunks_mut(self.bounds.num_pixels())
            .zip(self.bands.into_par_iter())
            .map(|(band_buff, read_band)| {
                let read_bounds = self.bounds.to_read_bounds(read_band.transform)?;
                info!("reading {:?} as {:?}", self.bounds, read_bounds);
                match read_bounds.shape() {
                    (1, 1) => {
                        let mut read_buff = [T::zero()];
                        read_band
                            .reader
                            .read_into_slice(read_bounds, &mut read_buff)?;
                        Ok::<_, RusterioError>(band_buff.fill(read_buff[0]))
                    }
                    read_shape if read_shape.eq(&self.bounds.shape()) => {
                        // TODO: chunk!
                        Ok(read_band.reader.read_into_slice(read_bounds, band_buff)?)
                    }
                    (read_shape_x, read_shape_y) => {
                        let read_buff_len = read_bounds.size();
                        let mut read_buff = unsafe { Box::new_zeroed_slice(read_buff_len).assume_init() };
                        read_band.reader
                        .read_into_slice(read_bounds, &mut read_buff)?;

                        let (ratio_x, ratio_y) = read_band.transform.ratio();
                        
                        let realtive_bounds = self.bounds.map_coords(|Coord { x, y }| Coord {
                            x: x % ratio_x,
                            y: y % ratio_y,
                        });
                        let (left_block_width, bottom_block_hight) =
                            (Coord::from((read_shape_x, read_shape_y)) - realtive_bounds.min())
                                .x_y();
                        let (right_block_width, top_block_hight) = realtive_bounds.max().x_y();
                        let (view_shape_x, _) = self.bounds.shape();

                        for (row_idx, read_row) in read_buff.chunks_exact(read_shape_x).enumerate()
                        {
                            let height = match row_idx {
                                0 => top_block_hight,
                                _ if row_idx != read_shape_y => ratio_y,
                                _ => bottom_block_hight,
                            };
                            let start =
                                view_shape_x * (row_idx * ratio_y + top_block_hight - height);

                            //let length = view_shape_x*height;
                            //band_buff[start..start+length];

                            for (col_idx, read_pixel) in read_row.iter().enumerate() {
                                let width = match col_idx {
                                    0 => left_block_width,
                                    _ if col_idx != read_shape_x => ratio_x,
                                    _ => right_block_width,
                                };
                                band_buff[start..start + width].fill(*read_pixel);
                            }

                            let length = view_shape_x * height;
                            band_buff[start..start + length]
                                .chunks_exact_mut(view_shape_x)
                                .into_iter()
                                .reduce(|lhc, mut _rhc| {
                                    _rhc = lhc;
                                    _rhc
                                });
                        }

                        Ok(())
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
}
