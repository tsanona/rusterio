#![allow(dead_code)]
extern crate geo_booleanop;
use geo::{BooleanOps, Coord, HasDimensions, Translate};

pub mod backends;
pub mod file;
pub mod reader;

pub use file::File;
pub use reader::BandReader;

use geo::{AffineOps, AffineTransform, BoundingRect, Rect};
use ndarray::{parallel::prelude::*, Array3, ArrayView2, Axis};
use num::Num;
use std::{collections::HashMap, fmt::Debug};

use crate::{
    errors::{Result, RusterioError},
    tuple_to, CrsGeometry,
};

type Metadata = HashMap<String, String>;

#[derive(Debug)]
pub struct Band {
    description: String,
    metadata: Metadata,
    //chunk_size: (usize, usize),
    data_type: String,
}

impl Band {
    pub fn new(description: String, metadata: Metadata, data_type: String) -> Self {
        Band {
            description,
            metadata,
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

    pub fn pixel_view<
        'raster: 'viewer,
        'viewer,
        T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync + 'static,
    >(
        &'raster self,
        band_indexes: &'static [usize],
        offset: (usize, usize),
        size: (usize, usize),
    ) -> Result<RasterView<'viewer, T>> {
        let bands: Vec<(usize, &Band, Box<dyn BandReader<T>>)> = band_indexes
            .into_iter()
            .map(|idx| {
                (
                    *idx,
                    &self.bands()[*idx],
                    Box::new(self.file.band_reader(*idx)) as Box<dyn BandReader<T>>,
                )
            })
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
            return Ok(RasterView { bands, bounds });
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

pub struct RasterView<'raster, T>
where
    T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync,
{
    //helps raster views support
    //bands from multiple rasters.
    /// Band references and readers.
    bands: Vec<(usize, &'raster Band, Box<dyn BandReader<T>>)>,
    bounds: Rect,
}

use gdal::raster::GdalType;

impl<'raster, T> RasterView<'raster, T>
where
    T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync + 'static,
{
    pub fn stack(self, view: RasterView<'raster, T>) -> Result<RasterView<'raster, T>> {
        unimplemented!()
    }

    pub fn read<'a>(&'a self, mask: Option<ArrayView2<'a, bool>>) -> Result<Array3<T>> {
        let mut array = Array3::zeros(self.array_size());
        let offset = tuple_to(self.offset());
        let errors: Result<()> = array
            .axis_iter_mut(Axis(0))
            .into_par_iter()
            .zip(&self.bands)
            .map(|(mut band, (_, _, reader))| {
                let read_array = reader.read_window_as_array(offset, band.dim(), mask);
                read_array.map(|read| band.assign(&read))
            })
            .collect();
        errors.map(|_| array)
    }

    /// Lower left corner of view.
    fn offset(&self) -> (f64, f64) {
        self.bounds.min().x_y()
    }

    fn offset_coords(&self) -> Coord {
        self.bounds.min()
    }

    fn array_size(&self) -> (usize, usize, usize) {
        (
            self.bands.len(),
            self.bounds.width() as usize,
            self.bounds.height() as usize,
        )
    }
}
