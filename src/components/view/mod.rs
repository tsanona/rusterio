mod band;
mod chunking;

use geo::Coord;
use log::info;
use rayon::prelude::*;
use std::{collections::HashSet, fmt::Debug, rc::Rc, sync::Arc};

use crate::{
    buffer::Buffer,
    components::{
        bounds::{Bounds, GeoBounds, PixelBounds, ViewBounds},
        raster::{band::RasterBand, group::RasterGroupInfo},
        transforms::ViewReadTransform,
        view::{
            band::{ReadBand, ViewBand},
            chunking::ResolutionChunker,
        },
        DataType,
    },
    errors::{Result, RusterioError},
    intersection::Intersection,
};

pub trait Len {
    fn len(&self) -> usize;
}

impl<T> Len for Rc<[T]> {
    fn len(&self) -> usize {
        self.as_ref().len()
    }
}

impl<T> Len for Arc<[T]> {
    fn len(&self) -> usize {
        self.as_ref().len()
    }
}

pub struct View<Ba: Clone + Len> {
    bounds: ViewBounds,
    bands: Ba,
}

pub type InfoView<T> = View<Rc<[ViewBand<T>]>>;
pub type ReadView<T> = View<Arc<[ReadBand<T>]>>;

impl<Ba: Clone + Len> View<Ba> {
    pub fn clip(&self, bounds: ViewBounds) -> Result<Self> {
        let bounds = self.bounds.intersection(&bounds)?;
        let bands = self.bands.clone();
        Ok(Self { bounds, bands })
    }

    pub fn bounds_shape(&self) -> (usize, usize) {
        self.bounds.shape().x_y()
    }

    /// Array shape (C, H, W)
    pub fn array_shape(&self) -> [usize; 3] {
        let (width, height) = self.bounds_shape();
        [self.bands.len(), height, width]
    }
}

impl<T: DataType> Debug for InfoView<T> {
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

impl<T: DataType> InfoView<T> {
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
        Ok(Self {
            bounds: view_bounds,
            bands,
        })
    }

    fn par_bands(&self) -> Box<[ReadBand<T>]> {
        self.bands
            .iter()
            .map(|view_band| ReadBand::from(view_band))
            .collect()
    }

    pub fn to_send_sync(self) -> ReadView<T> {
        let bands = Arc::from_iter(self.par_bands());
        let bounds = self.bounds;
        View { bounds, bands }
    }

    pub fn read(self) -> Result<Buffer<T, 3>> {
        self.to_send_sync().read()
    }
}

impl<T: DataType> ReadView<T> {
    pub fn read(&self) -> Result<Buffer<T, 3>> {
        let mut buff = Buffer::new(self.array_shape());
        let view_bounds = &self.bounds;
        buff.as_mut()
            .par_chunks_mut(view_bounds.size())
            .zip(self.bands.into_par_iter())
            .map(|(band_buff, read_band)| {
                let read_bounds = &view_bounds.as_read_bounds(&read_band.transform);
                info!("reading {:?} as {:?}", view_bounds, read_bounds);
                match read_bounds.shape() {
                    Coord { x: 1, y: 1 } => Ok::<_, RusterioError>(
                        band_buff.fill(read_band.reader.read_pixel(read_bounds.offset())?),
                    ),
                    read_shape if read_shape == view_bounds.shape() => {
                        // TODO: chunk!?
                        Ok(read_band.reader.read_into_slice(read_bounds, band_buff)?)
                    }
                    read_shape => {
                        info!("band has different shape: {:?}", read_shape);
                        let read_buff = read_band.reader.read_to_buffer(read_bounds)?;
                        ResolutionChunker::new(view_bounds, read_bounds)
                            .read_resolution_chucked(read_buff.as_ref(), band_buff)
                    }
                }
            })
            .collect::<Result<Vec<()>>>()?;
        Ok(buff)
    }
}
