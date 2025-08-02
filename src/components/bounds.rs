use geo_traits::{to_geo::ToGeoCoord, RectTrait};
use num::Integer;

use crate::{
    ambassador_remote_traits::{
        ambassador_impl_Area, ambassador_impl_GeometryTrait, ambassador_impl_MapCoords,
        ambassador_impl_RectTrait,
    },
    components::transforms::{GeoReadTransform, ViewReadTransform},
    errors::Result,
    intersection::Intersection,
    CoordUtils, CrsGeometry, LineUtils,
};
use geo::{AffineOps, Area, BoundingRect, Coord, CoordNum, Line, MapCoords, Rect};
use geo_traits::GeometryTrait;

/// Trait for shared Bound implementations.
///
/// Bounds are defined by `offset` or `origin`
/// and `shape` [Coord]s
pub trait Bounds: RectTrait + Intersection
where
    Self::T: CoordNum,
{
    /// (Width, Height)
    fn shape(&self) -> Coord<<Self as GeometryTrait>::T> {
        self.max().to_coord() - self.min().to_coord()
    }
}

/// Trait for shared PixelBound implementations.
/// Like [Bounds] but with integer side lengths.
pub trait PixelBounds: Bounds + Area<Self::T>
where
    Self::T: CoordNum + Integer,
{
    /* /// Offset coords for pixel bounds.
    ///
    ///  - [ViewBounds]: Top left pixel of the viewing window.
    ///  - [ReadBounds]: Depends of [GeoReadTransform].
    fn offset<'a>(&'a self) -> Self::CoordType<'a> {
        self.min()
    } */

    /// Number of pixels within bounds.
    fn size(&self) -> Self::T {
        self.unsigned_area()
    }
}

/// Geospatial bounds of the raster.
///
/// Deffined by:
///     - `origin`: Coords of top left pixel of raster.
///     - `shape`: Width x Height in crs unit.
///
/// In underlaying impl `offset` is given by `.min`,
/// and `shape` by `(.width, .hight) or .max - .min`.
#[derive(ambassador::Delegate, Shrinkwrap, Clone, Debug)]
#[delegate(GeometryTrait)]
#[delegate(RectTrait)]
pub struct GeoBounds(CrsGeometry<Rect>);

impl Intersection for GeoBounds {
    type Output = GeoBounds;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output> {
        Ok(GeoBounds(self.0.intersection(&rhs.0)?))
    }
}

impl Bounds for GeoBounds {}

impl From<CrsGeometry<Rect>> for GeoBounds {
    fn from(value: CrsGeometry<Rect>) -> Self {
        Self(value)
    }
}

impl From<&GeoBounds> for Line {
    fn from(value: &GeoBounds) -> Self {
        Line::new(value.min(), value.max())
    }
}

impl GeoBounds {
    pub fn origin(&self) -> Coord {
        self.min()
    }

    /// Build [ViewBounds] (or pixel bounds) of a raster.
    ///
    /// Transforms [GeoBounds] to [ViewBounds].
    /// It's pixel resolution is given by
    /// the `least common multiple` resolution
    /// of each raster band.
    /// A raster band resolution is encoded in [GeoReadTransform].
    pub fn build_raster_view_bounds<'a>(
        &'a self,
        transforms: impl Iterator<Item = &'a GeoReadTransform>,
    ) -> Result<ViewBounds> {
        let mut read_bounds: Vec<ReadBounds> = transforms
            .into_iter()
            .map(|transform| self.as_read_bounds(transform))
            .collect();
        let mut view_pixel_shape = read_bounds.pop().unwrap().shape();
        for read_pixel_shape in read_bounds.into_iter().map(|bounds| bounds.shape()) {
            view_pixel_shape = view_pixel_shape.operate(&read_pixel_shape, num::integer::lcm);
        }
        Ok(ViewBounds(Rect::new(Coord::zero(), view_pixel_shape)))
    }

    pub fn as_read_bounds(&self, transform: &GeoReadTransform) -> ReadBounds {
        let offset_shape_line = Line::from(self)
            .affine_transform(transform)
            .try_cast()
            .unwrap();
        ReadBounds(offset_shape_line.bounding_rect())
    }
}

/// Pixel bounds of the viewing window.
///
/// Deffined by:
///     - `offset`: Coords of top left pixel of view,
///         with origin at top left pixel of raster.
///     - `shape`: Width x Height in pixels.
///
/// In underlaying impl `offset` is given by `.min`,
/// and `shape` by `(.width, .hight) or .max - .min`.
#[derive(ambassador::Delegate, Debug)]
#[delegate(GeometryTrait)]
#[delegate(RectTrait)]
#[delegate(Area<T>, generics="T", where="T: CoordNum")]
#[delegate(MapCoords<T, NT>, generics="T, NT")]
pub struct ViewBounds(Rect<usize>);

impl Intersection for ViewBounds {
    type Output = ViewBounds;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output> {
        Ok(ViewBounds(self.0.intersection(&rhs.0)?))
    }
}

impl Bounds for ViewBounds {}
impl PixelBounds for ViewBounds {}

impl From<&ViewBounds> for Line<usize> {
    fn from(value: &ViewBounds) -> Self {
        Line::new(value.min(), value.max())
    }
}

impl ViewBounds {
    pub fn new(offset: (usize, usize), shape: (usize, usize)) -> Self {
        let offset = Coord::from(offset);
        let max = offset + Coord::from(shape);
        Self(Rect::new(offset, max))
    }

    pub fn as_read_bounds(&self, transform: &ViewReadTransform) -> ReadBounds {
        let offset_shape_line = Line::from(self)
            .try_cast()
            .unwrap()
            .affine_transform(transform)
            .map_coords(|coord| coord.map_each(f64::ceil))
            .try_cast()
            .unwrap();
        ReadBounds(offset_shape_line.bounding_rect())
    }
}

/// Pixel bounds of the reading window.
///
/// Deffined by:
///     - `offset`: Coords of offset pixel of view,
///         with origin determined by [GeoReadTransform].
///     - `shape`: Width x Height in pixels.
///
/// In underlaying impl `offset` is given by `.min`,
/// and `shape` by `(.width, .hight) or .max - .min`.
#[derive(ambassador::Delegate, Debug)]
#[delegate(GeometryTrait)]
#[delegate(RectTrait)]
#[delegate(Area<T>, generics="T", where="T: CoordNum")]
pub struct ReadBounds(Rect<usize>);

impl Intersection for ReadBounds {
    type Output = ReadBounds;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output> {
        Ok(ReadBounds(self.0.intersection(&rhs.0)?))
    }
}

impl Bounds for ReadBounds {}
impl PixelBounds for ReadBounds {}
