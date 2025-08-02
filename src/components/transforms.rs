use geo::{AffineTransform, Coord};
use std::rc::Rc;

use crate::{
    components::bounds::{Bounds, GeoBounds, ViewBounds},
    CoordUtils,
};

/// Affine transform between crs
/// and reading pixel space.
#[derive(Shrinkwrap, Debug)]
pub struct ReadGeoTransform {
    #[shrinkwrap(main_field)]
    transform: AffineTransform,
    pub crs: Rc<Box<str>>,
}

impl ReadGeoTransform {
    pub fn new(a: f64, b: f64, xoff: f64, d: f64, e: f64, yoff: f64, crs: Rc<Box<str>>) -> Self {
        let transform = AffineTransform::new(a, b, xoff, d, e, yoff);
        Self { transform, crs }
    }

    pub fn inverse(&self) -> GeoReadTransform {
        GeoReadTransform {
            transform: self.transform.inverse().unwrap(),
            crs: Rc::clone(&self.crs),
        }
    }
}

#[derive(Shrinkwrap, Debug)]
pub struct GeoReadTransform {
    #[shrinkwrap(main_field)]
    transform: AffineTransform,
    crs: Rc<Box<str>>,
}

impl GeoReadTransform {
    pub fn inverse(&self) -> ReadGeoTransform {
        ReadGeoTransform {
            transform: self.transform.inverse().unwrap(),
            crs: Rc::clone(&self.crs),
        }
    }
}

#[derive(Shrinkwrap, Debug, Clone, Copy)]
pub struct ViewReadTransform(AffineTransform);

impl ViewReadTransform {
    pub fn new(
        view_bounds: &ViewBounds,
        geo_bounds: &GeoBounds,
        geo_read_transform: &GeoReadTransform,
    ) -> Self {
        let view_pixel_shape: (f64, f64) = view_bounds.shape().try_cast().unwrap().x_y();
        let view_geo_transform = AffineTransform::new(
            geo_bounds.width() / view_pixel_shape.0,
            0.,
            geo_bounds.min().x,
            0.,
            -geo_bounds.height() / view_pixel_shape.1,
            geo_bounds.min().y + geo_bounds.height(),
        );
        Self(view_geo_transform.compose(geo_read_transform))
    }

    /// Ratio of View to Read shapes. (Height, Width)
    ///
    /// `ratio = view_shape / read_shape`.
    ///
    /// View bounds are in `View space`, which means that:
    ///
    /// `view_shape = read_shape * N `
    ///
    /// where N is non negative int.
    ///
    /// A.k.a the shape of the chunk of pixels in [ViewBounds] a pixel in [ReadBounds] fills up.
    ///
    pub fn ratio(&self) -> Coord<usize> {
        let inv = self.inverse().unwrap();
        Coord {
            x: inv.a().abs() as usize,
            y: inv.e().abs() as usize,
        }
    }
}
