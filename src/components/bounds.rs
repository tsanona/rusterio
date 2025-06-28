use std::rc::Rc;

use crate::{
    cast_tuple,
    components::transforms::{GeoBandTransform, ViewBandTransform},
    errors::Result,
    CrsGeometry,
};
use geo::{AffineOps, Coord, CoordNum, Intersects, MapCoords, Rect};

#[derive(thiserror::Error, Debug)]
pub enum BoundsError {
    #[error("Ther is no intersection between geometries")]
    NoIntersection,
}

/// Bounds in geo space.
#[derive(Shrinkwrap, Clone)]
pub struct GeoBounds(CrsGeometry<Rect>);

impl From<CrsGeometry<Rect>> for GeoBounds {
    fn from(value: CrsGeometry<Rect>) -> Self {
        Self(value)
    }
}

impl From<(Rc<str>, Rect)> for GeoBounds {
    fn from(value: (Rc<str>, Rect)) -> Self {
        let (crs, geometry) = value;
        Self(CrsGeometry { crs, geometry })
    }
}

impl GeoBounds {
    pub fn shape(&self) -> (f64, f64) {
        (self.0.geometry.height(), self.0.geometry.width())
    }

    pub fn intersection(&self, rhs: &GeoBounds) -> Result<GeoBounds> {
        Ok(GeoBounds(self
            .0
            .intersection(&rhs.0)?
            .bounding_rect()
            .ok_or(BoundsError::NoIntersection)?
            ))
    }
}

#[derive(Debug)]
pub struct ViewBounds(Rect<usize>);

impl ViewBounds {
    pub fn new(offset: (usize, usize), shape: (usize, usize)) -> Self {
        Self(Rect::new(offset, shape))
    }
    pub fn shape(&self) -> (usize, usize) {
        (self.0.height(), self.0.width())
    }

    pub fn max(&self) -> Coord<usize> {
        self.0.min() + self.0.max()
    }

    pub fn intersection(&self, rhs: &Self) -> std::result::Result<Self, BoundsError> {
        if self.0.intersects(&rhs.0) {
            let (self_max_x, self_max_y) = self.max().x_y();
            let (rhs_max_x, rhs_max_y) = rhs.max().x_y();
            let max = (self_max_x.min(rhs_max_x), self_max_y.min(rhs_max_y));

            let (self_min_x, self_min_y) = self.0.min().x_y();
            let (rhs_min_x, rhs_min_y) = rhs.0.min().x_y();
            let min = (self_min_x.max(rhs_min_x), self_min_y.max(rhs_min_y));

            return Ok(Self(Rect::new(min, max)));
        }
        Err(BoundsError::NoIntersection)
    }

    pub fn to_read_bounds(&self, transform: ViewBandTransform) -> Result<ReadBounds> {
        let bounds: Rect = self.0.try_map_coords(Self::cast_coord)?;
        let bounds: Rect<usize> = bounds
            .affine_transform(&transform)
            .try_map_coords(Self::cast_coord)?;
        Ok(ReadBounds(bounds))
    }

    fn cast_coord<T: CoordNum, U: CoordNum>(coord: Coord<T>) -> Result<Coord<U>> {
        Ok(Coord::from(cast_tuple(coord.x_y())?))
    }
}

pub struct ReadBounds(Rect<usize>);

impl ReadBounds {
    pub fn new(bounds: &GeoBounds, transform: &GeoBandTransform) -> Result<Self> {
        Ok(Self(
            bounds
                .affine_transform(transform)
                .try_map_coords(Self::cast_coord)?,
        ))
    }

    pub fn offset(&self) -> (usize, usize) {
        self.0.min().x_y()
    }

    pub fn shape(&self) -> (usize, usize) {
        (self.0.height(), self.0.width())
    }
    fn cast_coord<T: CoordNum, U: CoordNum>(coord: Coord<T>) -> Result<Coord<U>> {
        Ok(Coord::from(cast_tuple(coord.x_y())?))
    }
}
