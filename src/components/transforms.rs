use geo::AffineTransform;

use crate::components::bounds::GeoBounds;

#[derive(Shrinkwrap, Debug)]
pub struct ViewGeoTransform(#[shrinkwrap(main_field)] AffineTransform, String);

impl ViewGeoTransform {
    pub fn new(bounds: &GeoBounds, view_shape: (usize, usize)) -> Self {
        let transform = AffineTransform::new(
            bounds.geometry.width() / (view_shape.0 as f64),
            0.,
            bounds.geometry.min().x,
            0.,
            bounds.geometry.height() / (view_shape.1 as f64),
            bounds.geometry.min().y,
        );
        Self(transform, bounds.crs.clone())
    }
}

#[derive(Shrinkwrap, Debug)]
pub struct BandGeoTransform(#[shrinkwrap(main_field)] AffineTransform, String);

impl BandGeoTransform {
    pub fn new(a: f64, b: f64, xoff: f64, d: f64, e: f64, yoff: f64, crs: String) -> Self {
        Self(AffineTransform::new(a, b, xoff, d, e, yoff), crs)
    }

    pub fn inverse(&self) -> GeoBandTransform {
        GeoBandTransform(self.0.inverse().unwrap(), self.1.clone())
    }
}

#[derive(Shrinkwrap, Debug)]
pub struct GeoBandTransform(#[shrinkwrap(main_field)] AffineTransform, String);

impl GeoBandTransform {
    pub fn inverse(&self) -> BandGeoTransform {
        BandGeoTransform(self.0.inverse().unwrap(), self.1.clone())
    }
}

#[derive(Shrinkwrap, Debug, Clone, Copy)]
pub struct ViewBandTransform(AffineTransform);

impl ViewBandTransform {
    pub fn new(
        view_geo_transform: &ViewGeoTransform,
        geo_band_transform: &GeoBandTransform,
    ) -> Self {
        Self(view_geo_transform.compose(geo_band_transform))
    }

    pub fn ratio(&self) -> (usize, usize) {
        (self.a().abs() as usize, self.e().abs() as usize)
    }
}
