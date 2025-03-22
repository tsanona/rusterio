#![allow(dead_code)]
extern crate geo_booleanop;
use geo::{coord, BooleanOps, Coord, HasDimensions, Translate};

pub mod backends;
pub mod file;
pub mod reader;

pub use file::File;
pub use reader::Reader;

use geo::{AffineOps, AffineTransform, BoundingRect, Polygon, Rect};
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
    block_size: (usize, usize),
}

impl Band {
    pub fn new(description: String, metadata: Metadata, block_size: (usize, usize)) -> Self {
        Band {
            description,
            metadata,
            block_size,
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

    pub fn clip<'a, T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync>(
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
    }

    pub fn pixel_view<'a>(
        &self,
        band_indexes: &'a [usize],
        offset: (usize, usize),
        size: (usize, usize),
    ) -> Result<RasterView<'a, impl Reader + use<'_, F>>> {
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
            return Ok(RasterView {
                band_indexes,
                bounds,
                reader: self.file.reader(),
            });
        }
        Err(RusterioError::NoIntersection)
    }

    fn geometry_view<'a>(
        &self,
        band_indexes: &'a [usize],
        geometry: &CrsGeometry<Polygon>,
    ) -> Result<RasterView<'a, impl Reader + use<'_, F>>> {
        let raster_intersect = geometry
            .projected_geometry(&self.bounds.crs)?
            .intersection(&self.bounds.geometry.to_polygon());
        if !raster_intersect.is_empty() {
            let bounds = raster_intersect
                .affine_transform(self.transform())
                .bounding_rect()
                .unwrap();
            return Ok(RasterView {
                band_indexes,
                bounds,
                reader: self.file.reader(),
            });
        }
        Err(RusterioError::NoIntersection)
    }
}

pub struct RasterView<'a, R: Reader> {
    band_indexes: &'a [usize],
    bounds: Rect,
    reader: R,
}

use gdal::raster::GdalType;
use ndarray::{parallel::prelude::*, Array2, Array3, ArrayViewMut, Axis};
use num::Num;

impl<'a, R: Reader> RasterView<'a, R> {
    pub fn read<T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync>(
        self,
        mask: Option<Array2<bool>>,
    ) -> Result<Array3<T>> {
        // do chunking
        let size = (self.band_indexes.len(), self.size().0, self.size().0);
        let mut array = Array3::zeros(size);
        let fill_errors: Result<Vec<()>> = array
            .axis_chunks_iter_mut(Axis(0), 1)
            .into_par_iter()
            .enumerate()
            .map(|(idx, mut slice): (usize, ArrayViewMut<'_, T, _>)| {
                let band = self.reader.read_band_window_as_array::<T>(
                    idx,
                    tuple_to(self.offset()),
                    tuple_to(self.size()),
                    &mask,
                )?;
                slice.assign(&band);
                Ok(())
            })
            .collect();
        fill_errors.map(|_| array)
    }

    fn offset(&self) -> (f64, f64) {
        (self.bounds.max() - coord! { x: self.bounds.width(), y: 0.}).x_y()
    }

    fn size(&self) -> (usize, usize) {
        (self.bounds.width() as usize, self.bounds.height() as usize)
    }
}
