#![allow(dead_code)]

mod datasets;
mod readers;

pub use datasets::Dataset;
pub use readers::Reader;

use geo::{AffineOps, AffineTransform, Coord, Rect};
use std::{collections::HashMap, path::Path};

use crate::{backends, errors::Result, tuple_to, CrsGeometry};

type Metadata = HashMap<String, String>;

#[derive(Debug)]
pub struct Band {
    description: String,
    metadata: Metadata,
}

impl Band {
    pub fn new(description: String, metadata: Metadata) -> Self {
        Band {
            description,
            metadata,
        }
    }
}

#[derive(Debug)]
pub struct Raster {
    description: String,
    /// Bounds in raster crs such that,
    /// when projected to pixel coordinates,
    /// `min @ (0, 0)` and `max @ raster_size`.
    bounds: CrsGeometry<Rect>,
    /// Affine transform from `bounds` to pixel coordinates.
    transform: AffineTransform,
    bands: Vec<Band>,
    metadata: Metadata,
}

impl Raster {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let dataset = backends::open_dataset(path)?;

        let transform = dataset.transform()?;
        // reflect transform about x/y axis so bounds.max() == size()
        let transform = transform.scaled(
            transform.a().signum(),
            transform.e().signum(),
            Coord::zero(),
        );
        let size = dataset.size();
        let geometry = Rect::new((0., 0.), tuple_to(size)).affine_transform(&transform);
        let crs = dataset.crs();
        let bounds = CrsGeometry { crs, geometry };
        Ok(Self {
            description: dataset.description()?,
            metadata: dataset.metadata(),
            bounds,
            transform: transform.inverse().unwrap(),
            bands: dataset.bands()?,
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

    /// Return total number of bands.
    pub fn num_bands(&self) -> usize {
        self.bands.len()
    }
}

struct RasterStack {
    rasters: Vec<Raster>
}
