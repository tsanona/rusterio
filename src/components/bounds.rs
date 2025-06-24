use crate::{
    cast_tuple,
    errors::{Result, RusterioError},
    CrsGeometry,
};
use geo::{AffineOps, AffineTransform, Coord, CoordNum, Intersects, MapCoords, Rect};

/// Bounds in geo space.
#[derive(Shrinkwrap, Clone)]
pub struct GeoBounds(CrsGeometry<Rect>);

impl From<CrsGeometry<Rect>> for GeoBounds {
    fn from(value: CrsGeometry<Rect>) -> Self {
        Self(value)
    }
}

impl From<(String, Rect)> for GeoBounds {
    fn from(value: (String, Rect)) -> Self {
        let (crs, geometry) = value;
        CrsGeometry { crs, geometry }.into()
    }
}

impl GeoBounds {
    pub fn shape(&self) -> (f64, f64) {
        (self.0.geometry.width(), self.0.geometry.width())
    }

    pub fn intersection(&self, rhs: &GeoBounds) -> Result<GeoBounds> {
        Ok(self
            .0
            .intersection(&rhs.0)?
            .bounding_rect()
            .ok_or(RusterioError::NoIntersection)?
            .into())
    }
}

/// Bounds in pixel space.
#[derive(Shrinkwrap, Debug, Clone)]
pub struct PixelBounds(Rect<usize>);

impl<T: CoordNum> TryFrom<Rect<T>> for PixelBounds {
    type Error = RusterioError;
    fn try_from(value: Rect<T>) -> Result<Self> {
        let cast_rect: Rect<usize> = value.try_map_coords(Self::cast_coord)?;
        Ok(Self(cast_rect))
    }
}

impl TryFrom<&PixelBounds> for Rect {
    type Error = RusterioError;
    fn try_from(value: &PixelBounds) -> Result<Rect> {
        let cast_rect: Rect = value.try_map_coords(PixelBounds::cast_coord)?;
        Ok(cast_rect)
    }
}

impl PixelBounds {
    pub fn new<C: Into<Coord<usize>>>(min: C, max: C) -> Self {
        Self(Rect::new(min, max))
    }

    fn cast_coord<T: CoordNum, U: CoordNum>(coord: Coord<T>) -> Result<Coord<U>> {
        Ok(Coord::from(cast_tuple(coord.x_y())?))
    }

    pub fn shape(&self) -> (usize, usize) {
        (self.width(), self.height())
    }

    pub fn affine_transform(&self, transform: &AffineTransform) -> Result<Self> {
        let transformed_bounds = Rect::try_from(self)?.affine_transform(transform);
        let max = transformed_bounds.max().x_y();
        let min = transformed_bounds.min().x_y();
        //let transformed_bounds = transformed_bounds.map_coords(|Coord { x, y }| Coord::from((x.ceil(), y.floor())));
        Self::try_from(Rect::new(
            (min.0.floor(), min.1.floor()),
            (max.0.ceil(), max.1.ceil()),
        ))
    }

    pub fn intersection(&self, rhs: &PixelBounds) -> Result<PixelBounds> {
        if self.intersects(&rhs.0) {
            let (self_max_x, self_max_y) = self.max().x_y();
            let (rhs_max_x, rhs_max_y) = rhs.max().x_y();
            let max = (self_max_x.min(rhs_max_x), self_max_y.min(rhs_max_y));

            let (self_min_x, self_min_y) = self.min().x_y();
            let (rhs_min_x, rhs_min_y) = rhs.min().x_y();
            let min = (self_min_x.max(rhs_min_x), self_min_y.max(rhs_min_y));

            return Ok(PixelBounds(Rect::new(min, max)));
        }
        Err(RusterioError::NoIntersection)
    }
}
