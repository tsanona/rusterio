#![allow(dead_code)]
extern crate geo_booleanop;
use geo::{BooleanOps, Coord, HasDimensions, Translate};

pub mod backends;
pub mod file;
pub mod reader;

pub use file::File;
pub use reader::Reader;

use geo::{AffineOps, AffineTransform, BoundingRect, Rect};
use ndarray::{parallel::prelude::*, Array3, ArrayView2, Axis};
use num::{Integer, Num};
use std::{collections::HashMap, fmt::Debug, sync::Arc};

use crate::{
    errors::{Result, RusterioError},
    tuple_to, CrsGeometry,
};

type Metadata = HashMap<String, String>;

#[derive(Debug)]
pub struct Band {
    description: String,
    metadata: Metadata,
    chunk_size: (usize, usize),
    data_type: String,
}

impl Band {
    pub fn new(
        description: String,
        metadata: Metadata,
        chunk_size: (usize, usize),
        data_type: String,
    ) -> Self {
        Band {
            description,
            metadata,
            chunk_size,
            data_type,
        }
    }
}

#[derive(Debug)]
pub struct Raster<F: File> {
    description: String,
    /// Bounds in raster crs such that,
    /// when projected to pixel coordinates,
    /// `min @ (0, 0)` and `max @ raster_size`.
    bounds: CrsGeometry<Rect>,
    /// Affine transform from `bounds` to pixel coordinates.
    transform: AffineTransform,
    bands: Vec<Band>,
    metadata: Metadata,
    file: F,
}

impl<F: File> Raster<F> {
    pub fn new(file: F) -> Result<Self> {
        //let file = F::open(path)?;

        let transform = file.transform()?;
        // reflect transform about x/y axis so bounds.max() == size()
        let transform = transform.scaled(
            transform.a().signum(),
            transform.e().signum(),
            Coord::zero(),
        );

        let geometry = Rect::new((0., 0.), tuple_to(file.size())).affine_transform(&transform);
        let crs = file.crs();
        let bounds = CrsGeometry { crs, geometry };

        let description = file.description()?;
        let metadata = file.metadata();
        let bands = file.bands()?;

        Ok(Self {
            description,
            metadata,
            bounds,
            transform: transform.inverse().unwrap(),
            bands,
            file,
        })
    }

    /// Return geospatial bounds `(left, bottom, right, top)`.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let (left, bottom) = self.bounds.geometry.min().x_y();
        let (right, top) = self.bounds.geometry.max().x_y();
        (left, bottom, right, top)
    }

    /// Return affine transform from geospatial to pixel coordinates
    pub fn transform(&self) -> &AffineTransform {
        &self.transform
    }

    /// Return width and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        tuple_to(self.transform.apply(self.bounds.geometry.max()).x_y())
    }

    /// Return bands,
    /// __in order of indexing__.
    pub fn bands(&self) -> &Vec<Band> {
        &self.bands
    }

    // can only be done on view
    /* pub fn clip<'a, T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync>(
        &self,
        band_indexes: &'a [usize],
        geometry: CrsGeometry<Polygon>,
    ) -> Result<Array3<T>> {
        use geo_rasterize::BinaryBuilder;

        let view = self.geometry_view(band_indexes, &geometry)?;
        let mut rasterizer = BinaryBuilder::new()
            .width(view.bounds.width() as usize)
            .height(view.bounds.height() as usize)
            .build()?;
        rasterizer.rasterize(&geometry.geometry.affine_transform(self.transform()))?;
        let mask = rasterizer.finish();
        view.read(Some(mask))
    } */

    pub fn pixel_view<'raster: 'view, 'view, T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync + 'static>(
        &'raster self,
        band_indexes: &'static [usize],
        offset: (usize, usize),
        size: (usize, usize),
    ) -> Result<RasterView<'raster, 'view, T>> {
        let reader: Arc<dyn Reader<'view, T>> = Arc::new(self.file.reader::<T>());
        let bands: Vec<(usize, &Band)> = band_indexes
            .into_iter()
            .map(|idx| (*idx, &self.bands()[*idx]))
            .collect();
        let (x_offset, y_offset) = tuple_to(offset);
        let pixel_bounds = Rect::new((0., 0.), tuple_to(size)).translate(x_offset, y_offset);
        let raster_intersect = pixel_bounds.to_polygon().intersection(
            &self
                .bounds
                .geometry
                .to_polygon()
                .affine_transform(self.transform()),
        );
        if !raster_intersect.is_empty() {
            let bounds = raster_intersect.bounding_rect().unwrap();
            let chunk_size = bands
                .iter()
                .map(|(_, band)| band.chunk_size)
                .reduce(|(x_acc, y_acc), (x, y)| (x_acc.lcm(&x), y_acc.lcm(&y)))
                .unwrap();
            return Ok(RasterView {
                bands,
                bounds,
                chunk_size,
                reader,
            });
        }
        Err(RusterioError::NoIntersection)
    }

    /* fn geometry_view<T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync + 'static>(
        &self,
        band_indexes: &'static [usize],
        geometry: &CrsGeometry<Polygon>,
    ) -> Result<RasterView<T>> {
        let reader: Arc<dyn Reader<T>> = Arc::new(self.file.reader::<T>());
        let bands: Vec<(usize, &Band, Arc<dyn Reader<T>>)> = band_indexes
            .into_iter()
            .map(|idx| (*idx, &self.bands()[*idx], reader.clone()))
            .collect();
        let raster_intersect = geometry
            .projected_geometry(&self.bounds.crs)?
            .intersection(&self.bounds.geometry.to_polygon());
        if !raster_intersect.is_empty() {
            let bounds = raster_intersect
                .affine_transform(self.transform())
                .bounding_rect()
                .unwrap();
            let chunk_size = bands
                .iter()
                .map(|(_, band, _)| band.chunk_size)
                .reduce(|(x_acc, y_acc), (x, y)| (x_acc.lcm(&x), y_acc.lcm(&y)))
                .unwrap();
            return Ok(RasterView {
                bands,
                bounds,
                chunk_size,
            });
        }
        Err(RusterioError::NoIntersection)
    } */
}

pub struct RasterView<'raster: 'reader, 'reader, T>
where
    T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync,
{
    //helps raster views support
    //bands from multiple rasters.
    /// Band references and readers.
    bands: Vec<(usize, &'raster Band)>,
    bounds: Rect,
    chunk_size: (usize, usize),
    reader: Arc<dyn Reader<'reader, T>>,
}

use gdal::raster::GdalType;

impl<'raster, 'reader, T> RasterView<'raster, 'reader, T>
where
    T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync + 'static,
{
    pub fn read<'a>(&'a self, mask: Option<ArrayView2<'a, bool>>) -> Result<Array3<T>> {
        let mut array = Array3::zeros(self.array_size());
        let offset = tuple_to(self.offset());
        for (mut band_chunk, (band_idx, _)) in  array
            .axis_chunks_iter_mut(Axis(0), 1)
            .zip(&self.bands) {
            let dims = band_chunk.dim();
            let read_array = self.reader
                .read_band_window_as_array(
                    *band_idx,
                    offset,
                    tuple_to((dims.1, dims.2)),
                    mask,
                );
            read_array.map(|read| band_chunk.assign(&read))?
            }
        Ok(array)
    }

    pub fn read_async_bands<'a>(&'a self, mask: Option<ArrayView2<'a, bool>>) -> Result<Array3<T>> {
        let mut array = Array3::zeros(self.array_size());
        let offset = tuple_to(self.offset());
        let errors: Result<()> = array
            .axis_chunks_iter_mut(Axis(0), 1)
                .into_par_iter()
                .zip(&self.bands)
                .map(|(mut band_chunk, (band_idx, _))| {
                    let dims = band_chunk.dim();
                    let read_array = self.reader
                        .read_band_window_as_array(
                            *band_idx,
                            offset,
                            tuple_to((dims.1, dims.2)),
                            mask,
                        );
                    read_array.map(|read| band_chunk.assign(&read))
                }).collect();
        errors.map(|_| array)
    }

    /* pub fn read_async_chunks<'a>(&'a self, mask: Option<ArrayView2<'a, bool>>) -> Result<Array3<T>> {
        // do chunking
        let mut array = Array3::zeros(self.array_size());
        let offset = tuple_to(self.offset());
        let errors: Result<()> = array
            .axis_chunks_iter_mut(Axis(0), 1)
            .into_par_iter()
            .zip(&self.bands)
            .map(|(mut band_chunk, (band_idx, band_info))| {
                let chunk_size = band_info.chunk_size;
                // check if view is smaller in area to chunk
                if (band_chunk.len() < chunk_size.0 * chunk_size.1) | true {
                    let dims = band_chunk.dim();
                    let read_array = self.reader
                        .read_band_window_as_array(
                            *band_idx,
                            offset,
                            tuple_to((dims.1, dims.2)),
                            mask,
                        );
                    read_array.map(|read| band_chunk.assign(&read))
                // chunk with mask
                } else if let Some(mask) = mask.as_ref() {
                    let errors: Result<()> = band_chunk
                    .axis_chunks_iter_mut(Axis(1), chunk_size.0)
                    //.zip(mask.axis_chunks_iter(Axis(0), chunk_size.0))
                    .enumerate()
                    .map(move |(width_idx, mut width_chunk)| {
                        let data_chunks = width_chunk
                            .axis_chunks_iter_mut(Axis(2), chunk_size.1)
                            .into_par_iter();
                        //let mask_chunks: AxisChunksIter<'_, bool, _> = mask_width_chunk.axis_chunks_iter(Axis(1), chunk_size.1);
                        let errors: Result<()> = data_chunks
                            //.zip(mask_chunks)
                            .enumerate()
                            .map(|(hight_index, mut array_chunk)| {
                                let dims = array_chunk.dim();
                                let offset = (width_idx * chunk_size.0, hight_index * chunk_size.1);
                                let size = tuple_to((dims.1, dims.2));
                                let chunk_mask = mask.slice(s![offset.0..size.0, offset.1..size.1]);
                                self.reader.read_band_window_as_array(
                                        *band_idx,
                                        offset,
                                        size,
                                        Some(chunk_mask),
                                    ).map(|read| array_chunk.assign(&read))
                            })
                            .collect();
                        errors
                    })
                    .collect();
                    errors
                } else { 
                    let errors: Result<()> = band_chunk
                    .axis_chunks_iter_mut(Axis(1), chunk_size.0)
                    //.zip(mask.axis_chunks_iter(Axis(0), chunk_size.0))
                    .enumerate()
                    .map(move |(width_idx, mut width_chunk)| {
                        let data_chunks = width_chunk
                            .axis_chunks_iter_mut(Axis(2), chunk_size.1)
                            .into_par_iter();
                        //let mask_chunks: AxisChunksIter<'_, bool, _> = mask_width_chunk.axis_chunks_iter(Axis(1), chunk_size.1);
                        let errors: Result<()> = data_chunks
                            //.zip(mask_chunks)
                            .enumerate()
                            .map(|(hight_index, mut array_chunk)| {
                                let dims = array_chunk.dim();
                                let offset = (width_idx * chunk_size.0, hight_index * chunk_size.1);
                                let size = tuple_to((dims.1, dims.2));
                                self.reader.read_band_window_as_array(
                                        *band_idx,
                                        offset,
                                        size,
                                        None,
                                    ).map(|read| array_chunk.assign(&read))
                            })
                            .collect();
                        errors
                    })
                    .collect();
                    errors
                 }
            })
            .collect();
        errors.map(|_| array)
    }
 */
    //pub fn save()

    /// Lower left corner of view.
    fn offset(&self) -> (f64, f64) {
        self.bounds.min().x_y()
    }

    fn array_size(&self) -> (usize, usize, usize) {
        (
            self.bands.len(),
            self.bounds.width() as usize,
            self.bounds.height() as usize,
        )
    }
}
