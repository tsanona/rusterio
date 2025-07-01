use geo::AffineTransform;
use std::rc::Rc;

use crate::{
    components::bounds::{GeoBounds, ViewBounds},
    errors::Result,
    try_tuple_cast,
};

/// Transform fom [View] `pixel space`
/// to `geo space` of given [GeoBounds].
///
/// The [View]  `pixel space` is given
/// by taking the [lcm](https://en.wikipedia.org/wiki/Least_common_multiple)
/// of all `read bounds` for a given [GeoBounds].
/// This means that, the resolution of `View space`
/// will be equal to the lcm of the resolutions of the bands in [View].
///
/// [View][crate::components::view::View], [GeoBounds][crate::components::bounds::GeoBounds]
#[derive(Shrinkwrap, Debug)]
pub struct ViewGeoTransform(#[shrinkwrap(main_field)] AffineTransform, Rc<str>);

impl ViewGeoTransform {
    pub fn new<'a>(view_bounds: &ViewBounds, geo_bounds: &GeoBounds) -> Result<Self> {
        let view_pixel_shape: (f64, f64) = try_tuple_cast(view_bounds.shape())?;
        let transform = AffineTransform::new(
            geo_bounds.geometry.width() / view_pixel_shape.0,
            0.,
            geo_bounds.geometry.min().x,
            0.,
            geo_bounds.geometry.height() / view_pixel_shape.1,
            geo_bounds.geometry.min().y,
        );
        Ok(Self(transform, Rc::clone(&geo_bounds.crs)))
    }
}

#[derive(Shrinkwrap, Debug)]
pub struct BandGeoTransform(#[shrinkwrap(main_field)] AffineTransform, Rc<str>);

impl BandGeoTransform {
    pub fn new(a: f64, b: f64, xoff: f64, d: f64, e: f64, yoff: f64, crs: Rc<str>) -> Self {
        Self(AffineTransform::new(a, b, xoff, d, e, yoff), crs)
    }

    pub fn inverse(&self) -> GeoBandTransform {
        GeoBandTransform(self.0.inverse().unwrap(), Rc::clone(&self.1))
    }
}

#[derive(Shrinkwrap, Debug)]
pub struct GeoBandTransform(#[shrinkwrap(main_field)] AffineTransform, Rc<str>);

impl GeoBandTransform {
    pub fn inverse(&self) -> BandGeoTransform {
        BandGeoTransform(self.0.inverse().unwrap(), Rc::clone(&self.1))
    }
}

#[derive(Shrinkwrap, Debug, Clone, Copy)]
pub struct ViewReadTransform(AffineTransform);

impl ViewReadTransform {
    pub fn new(
        view_geo_transform: &ViewGeoTransform,
        geo_band_transform: &GeoBandTransform,
    ) -> Self {
        Self(view_geo_transform.compose(geo_band_transform))
    }

    /// Ratio of View to Read shapes. (Height, Width)
    ///
    /// `ratio = view_shape / read_shape`.
    ///
    /// View bounds are in `View space`, which means that:
    ///
    /// `view_shape = read_shape * N `
    ///
    /// where N is non negative.
    ///
    /// A.k.a the shape of the chunk of pixels in [ViewBounds] a pixel in [ReadBounds] fills up.
    ///
    pub fn ratio(&self) -> (usize, usize) {
        (self.a().abs() as usize, self.e().abs() as usize)
    }
}
