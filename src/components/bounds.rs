use num::Integer;
use std::rc::Rc;

use crate::{
    components::transforms::{GeoBandTransform, ViewReadTransform},
    errors::Result,
    try_coord_cast, CrsGeometry,
};
use geo::{AffineOps, Coord, Intersects, MapCoords, Rect};

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
        Ok(GeoBounds(
            self.0
                .intersection(&rhs.0)?
                .bounding_rect()
                .ok_or(BoundsError::NoIntersection)?,
        ))
    }
}

#[derive(Debug, Shrinkwrap)]
pub struct ViewBounds(Rect<usize>);

impl ViewBounds {
    pub fn new(offset: (usize, usize), shape: (usize, usize)) -> Self {
        let offset = Coord::from(offset);
        let max = offset + Coord::from(shape);
        Self(Rect::new(offset, max))
    }

    pub fn from<'a>(
        bounds: &'a GeoBounds,
        transforms: impl Iterator<Item = &'a GeoBandTransform>,
    ) -> Result<Self> {
        let mut read_bounds = transforms
            .into_iter()
            .map(|transform| ReadBounds::new(&bounds, transform))
            .collect::<Result<Vec<ReadBounds>>>()?;
        let (mut view_pixel_x, mut view_pixel_y) = read_bounds.pop().unwrap().shape();
        for read_pixel_shape in read_bounds.into_iter().map(|bounds| bounds.shape()) {
            view_pixel_x = view_pixel_x.lcm(&read_pixel_shape.0);
            view_pixel_y = view_pixel_y.lcm(&read_pixel_shape.1);
        }
        Ok(ViewBounds(Rect::new((0, 0), (view_pixel_x, view_pixel_y))))
    }

    // (Height, Width)
    pub fn shape(&self) -> (usize, usize) {
        (self.0.height(), self.0.width())
    }

    pub fn top_right(&self) -> (usize, usize) {
        self.0.max().x_y()
    }

    pub fn bottom_left(&self) -> (usize, usize) {
        self.0.min().x_y()
    }

    pub fn offset(&self) -> (usize, usize) {
        self.0.min().x_y()
    }

    pub fn num_pixels(&self) -> usize {
        let (height, width) = self.shape();
        height * width
    }

    pub fn intersection(&self, rhs: &Self) -> std::result::Result<Self, BoundsError> {
        if self.0.intersects(&rhs.0) {
            let (self_max_x, self_max_y) = self.0.max().x_y();
            let (rhs_max_x, rhs_max_y) = rhs.0.max().x_y();
            let max = (self_max_x.min(rhs_max_x), self_max_y.min(rhs_max_y));

            let (self_min_x, self_min_y) = self.0.min().x_y();
            let (rhs_min_x, rhs_min_y) = rhs.0.min().x_y();
            let min = (self_min_x.max(rhs_min_x), self_min_y.max(rhs_min_y));

            return Ok(Self(Rect::new(min, max)));
        }
        Err(BoundsError::NoIntersection)
    }

    pub fn to_read_bounds(&self, transform: ViewReadTransform) -> Result<ReadBounds> {
        let bounds: Rect = self.0.try_map_coords(try_coord_cast)?;
        let bounds: Rect<usize> = bounds
            .affine_transform(&transform)
            .try_map_coords(try_coord_cast)?;
        Ok(ReadBounds(bounds))
    }
}

pub struct ReadBounds(Rect<usize>);

impl ReadBounds {
    pub fn new(bounds: &GeoBounds, transform: &GeoBandTransform) -> Result<Self> {
        Ok(Self(
            bounds
                .affine_transform(transform)
                .try_map_coords(try_coord_cast)?,
        ))
    }

    pub fn offset(&self) -> (usize, usize) {
        self.0.min().x_y()
    }

    /// (Height, Width)
    pub fn shape(&self) -> (usize, usize) {
        (self.0.height(), self.0.width())
    }

    pub fn size(&self) -> usize {
        let (hight, width) = self.shape();
        hight * width
    }
}
