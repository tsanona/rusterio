use geo::{Coord, MapCoords};
use log::info;
use num::Zero;
use rayon::prelude::*;
use std::{collections::HashSet, fmt::Debug, ops::Rem, rc::Rc, sync::Arc};

use crate::{
    buffer::Buffer,
    components::{
        band::{BandInfo, BandReader},
        bounds::{Bounds, GeoBounds, PixelBounds, ViewBounds},
        raster::{band::RasterBand, group::RasterGroupInfo},
        transforms::ViewReadTransform,
        DataType,
    },
    errors::{Result, RusterioError},
    intersection::Intersection,
    CoordUtils,
};

#[derive(Debug, Clone)]
pub struct ViewBand<T: DataType> {
    info: Rc<dyn BandInfo>,
    /// Transform from [RasterView] pixel space to band pixel space.
    transform: ViewReadTransform,
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

pub struct ReadBand<T: DataType> {
    transform: ViewReadTransform,
    reader: Arc<dyn BandReader<T>>,
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
        let view = Self { bounds, bands };
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

        let view_bounds = bounds.build_raster_view_bounds(view_transforms)?;

        let bands = Rc::from_iter(selected_bands.iter().map(|(group_info, raster_band)| {
            let transform = ViewReadTransform::new(&view_bounds, &bounds, &group_info.transform);
            ViewBand::from((transform, *raster_band))
        }));
        Ok(Self::init(view_bounds, bands))
    }

    pub fn clip(&self, bounds: ViewBounds) -> Result<Self> {
        let bounds: ViewBounds = self.bounds.intersection(&bounds)?;
        let bands = Rc::clone(&self.bands);
        Ok(Self::init(bounds, bands))
    }

    fn par_bands(&self) -> Box<[ReadBand<T>]> {
        self.bands
            .iter()
            .map(|view_band| ReadBand::from(view_band))
            .collect()
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
    bands: Arc<[ReadBand<T>]>,
}

impl<T: DataType> SendSyncView<T> {
    pub fn clip(&self, bounds: ViewBounds) -> Result<Self> {
        let bounds = self.bounds.intersection(&bounds)?;
        let bands = Arc::clone(&self.bands);
        Ok(Self { bounds, bands })
    }

    pub fn read(&self) -> Result<Buffer<T, 3>> {
        let mut buff = Buffer::new(self.array_shape());
        let view_bounds = &self.bounds;
        buff.as_mut()
            .par_chunks_mut(view_bounds.size())
            .zip(self.bands.into_par_iter())
            .map(|(mut band_buff, read_band)| {
                let read_bounds = view_bounds.as_read_bounds(&read_band.transform);
                info!("reading {:?} as {:?}", view_bounds, read_bounds);
                match read_bounds.shape() {
                    Coord { x: 1, y: 1 } => {
                        let mut read_buff = [T::zero()];
                        read_band
                            .reader
                            .read_into_slice(&read_bounds, &mut read_buff)?;
                        let _ = band_buff.fill(read_buff[0]);
                        Ok::<_, RusterioError>(())
                    }
                    read_shape if read_shape.eq(&view_bounds.shape()) => {
                        // TODO: chunk!?
                        Ok(read_band.reader.read_into_slice(&read_bounds, band_buff)?)
                    }
                    read_shape => {
                        info!("band has different shape: {:?}", read_shape);
                        let read_buff_len = read_bounds.size();
                        let mut read_buff = Buffer::new([read_buff_len]);
                        read_band
                            .reader
                            .read_into_slice(&read_bounds, read_buff.as_mut())?;

                        let ratio = read_band.transform.ratio();

                        info!(
                            "read shape: {:?}, ratio: {:?}, view shape: {:?}",
                            read_shape,
                            ratio,
                            view_bounds.shape()
                        );
                        let relative_bounds =
                            view_bounds.map_coords(|coord| coord.operate(&ratio, usize::rem));

                        let left_block_width = if relative_bounds.min().x.is_zero() {
                            ratio.x
                        } else {
                            ratio.x - relative_bounds.min().x
                        };
                        let top_block_hight = if relative_bounds.max().y.is_zero() {
                            ratio.y
                        } else {
                            relative_bounds.max().y
                        };
                        let view_shape = view_bounds.shape();

                        for (row_idx, read_row) in
                            read_buff.as_mut().chunks_exact(read_shape.x).enumerate()
                        {
                            let block_hight = if row_idx.is_zero() {
                                top_block_hight
                            } else {
                                ratio.y
                            };

                            let block_buff: &mut [T];
                            (block_buff, band_buff) =
                                band_buff.split_at_mut(view_shape.x * block_hight);

                            for (col_idx, read_pixel) in read_row.iter().enumerate() {
                                let block_width = if col_idx.is_zero() {
                                    left_block_width
                                } else {
                                    ratio.x
                                };
                                let col_start = col_idx * ratio.x + left_block_width - block_width;
                                let band_write_range = col_start..col_start + block_width;
                                block_buff[band_write_range].fill(*read_pixel);
                            }

                            block_buff
                                .chunks_exact_mut(view_shape.x)
                                .into_iter()
                                //.par_chunks_exact(view_shape_x)
                                .reduce(|lhc, mut _rhc| {
                                    _rhc.copy_from_slice(lhc);
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
        self.bounds.shape().x_y()
    }

    /// Array shape (C, H, W)
    pub fn array_shape(&self) -> [usize; 3] {
        let (width, hieght) = self.bounds_shape();
        [self.bands.len(), hieght, width]
    }
}
