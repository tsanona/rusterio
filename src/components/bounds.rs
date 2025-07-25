use geo_traits::{to_geo::ToGeoCoord, RectTrait};
use num::Integer;

use crate::{
    ambassador_remote_traits::{ambassador_impl_GeometryTrait, ambassador_impl_RectTrait},
    components::transforms::{GeoReadTransform, ViewReadTransform},
    errors::Result,
    intersection::Intersection,
    CoordUtils, CrsGeometry,
};
use geo::{AffineOps, Area, Coord, CoordNum, MapCoords, Rect};
use geo_traits::GeometryTrait;

pub trait Bounds: RectTrait
where
    Self::T: CoordNum,
{
    fn shape(&self) -> Coord<<Self as GeometryTrait>::T> {
        self.max().to_coord() - self.min().to_coord()
    }
}

#[derive(ambassador::Delegate, Shrinkwrap, Clone, Debug)]
#[delegate(GeometryTrait)]
#[delegate(RectTrait)]
pub struct GeoBounds(CrsGeometry<Rect>);

impl Bounds for GeoBounds {}

impl Intersection for GeoBounds {
    type Output = GeoBounds;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output> {
        Ok(GeoBounds(self.0.intersection(&rhs.0)?))
    }
}

impl From<CrsGeometry<Rect>> for GeoBounds {
    fn from(value: CrsGeometry<Rect>) -> Self {
        Self(value)
    }
}

/// Pixel bounds of the viewing window.
///
/// Deffined by:
///     - `offset`: Coords of top lef pixel of view,
///         with origin at top left pixel of raster.
///     - `shape`: (H, W) a.ka. row column.
///
/// In underlaying impl `offset` is given by `.min`,
/// and `shape` by `(.hight, .width)`.
#[derive(ambassador::Delegate, Shrinkwrap, Debug)]
#[delegate(GeometryTrait)]
#[delegate(RectTrait)]
pub struct ViewBounds(Rect<usize>);

impl Bounds for ViewBounds {}

impl Intersection for ViewBounds {
    type Output = ViewBounds;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output> {
        Ok(ViewBounds(self.0.intersection(&rhs.0)?))
    }
}

impl ViewBounds {
    pub fn new(offset: (usize, usize), shape: (usize, usize)) -> Self {
        let offset = Coord::from(offset);
        let max = offset + Coord::from(shape);
        Self(Rect::new(offset, max))
    }

    pub fn from<'a>(
        bounds: &'a GeoBounds,
        transforms: impl Iterator<Item = &'a GeoReadTransform>,
    ) -> Result<Self> {
        let mut read_bounds: Vec<ReadBounds> = transforms
            .into_iter()
            .map(|transform| ReadBounds::from((bounds, transform)))
            .collect();
        let (mut view_pixel_x, mut view_pixel_y) = read_bounds.pop().unwrap().shape();
        for read_pixel_shape in read_bounds.into_iter().map(|bounds| bounds.shape()) {
            view_pixel_x = view_pixel_x.lcm(&read_pixel_shape.0);
            view_pixel_y = view_pixel_y.lcm(&read_pixel_shape.1);
        }
        Ok(ViewBounds(Rect::new((0, 0), (view_pixel_x, view_pixel_y))))
    }

    pub fn shape(&self) -> Coord<usize> {
        Coord {
            x: self.0.width(),
            y: self.0.height(),
        }
    }

    /// Coords of the top left pixel of the viewing window.
    pub fn offset(&self) -> Coord<usize> {
        self.0.min()
    }

    /// Pixel area of the viewing window.
    pub fn size(&self) -> usize {
        self.0.unsigned_area()
    }
}

#[derive(ambassador::Delegate, Shrinkwrap, Debug)]
#[delegate(GeometryTrait)]
#[delegate(RectTrait)]
pub struct ReadBounds(Rect<usize>);

impl Bounds for ReadBounds {}

impl Intersection for ReadBounds {
    type Output = ReadBounds;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output> {
        Ok(ReadBounds(self.0.intersection(&rhs.0)?))
    }
}

impl From<(&ViewBounds, &ViewReadTransform)> for ReadBounds {
    fn from(value: (&ViewBounds, &ViewReadTransform)) -> Self {
        let offset = value
            .1
            .apply(value.0.offset().try_cast().unwrap())
            .map_each(f64::ceil)
            .try_cast()
            .unwrap();
        let shape = value
            .1
            .apply(value.0.shape().try_cast().unwrap())
            .map_each(f64::ceil)
            .try_cast()
            .unwrap();
        Self(Rect::new(offset, offset + shape))
    }
}

impl From<(&GeoBounds, &GeoReadTransform)> for ReadBounds {
    fn from(value: (&GeoBounds, &GeoReadTransform)) -> Self {
        let read_bounds = value.0 .0.affine_transform(value.1); // get offset = min and shape and then correct as points so rect doesn't change them
        Self(read_bounds.try_map_coords(Coord::try_cast).unwrap())
    }
}

impl ReadBounds {
    pub fn top_left(&self) -> (usize, usize) {
        (self.0.min() + Coord::from((0, self.0.height()))).x_y()
    }

    /// (width,hight)
    pub fn shape(&self) -> (usize, usize) {
        (self.0.width(), self.0.height())
    }

    pub fn size(&self) -> usize {
        self.0.unsigned_area()
    }
}
