#![allow(dead_code)]

mod components;
mod errors;

pub use components::{Raster, files};

use geo::{proj::Proj, Transform};
use geo_traits::GeometryTrait;
use num::traits::AsPrimitive;

use errors::{Result, RusterioError};

#[derive(Debug)]
pub struct CrsGeometry<G: GeometryTrait + Transform<G::T, Output = G>> {
    crs: String,
    geometry: G,
}

impl<G: GeometryTrait + Transform<G::T, Output = G>> CrsGeometry<G> {
    fn with_crs(mut self, crs: String) -> Result<Self> {
        let proj = Proj::new_known_crs(self.crs.as_str(), crs.as_str(), None)?;
        self.crs = crs;
        self.geometry.transform(&proj)?;
        Ok(self)
    }

    fn projected_geometry(&self, crs: String) -> Result<G> {
        let proj = Proj::new_known_crs(self.crs.as_str(), crs.as_str(), None)?;
        self.geometry
            .transformed(&proj)
            .map_err(RusterioError::ProjError)
    }
}

fn tuple_to<TO: Copy + 'static, TI: AsPrimitive<TO>>(tuple: (TI, TI)) -> (TO, TO) {
    (tuple.0.as_(), tuple.1.as_())
}

#[cfg(test)]
mod tests {
    use super::*;
    use components::{
        files::{gdal_backend::GdalFile, File},
        Raster,
    };
    use rstest::rstest;

    #[rstest]
    fn play_ground() {
        let raster_path = "SENTINEL2_L2A:/vsizip/data/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE.zip/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE/MTD_MSIL2A.xml:10:EPSG_32633";
        let file = GdalFile::open(raster_path).unwrap();
        let raster = Raster::new(file).unwrap();
        println!("{:#?}", raster);
        //println!("{:#?}", raster.read_pixel_window(&[0, 1, 2, 3], (0, 0), (125, 125)).unwrap().shape())
        //assert_eq!(raster.size(), tuple_to(dataset.raster_size()))
    }
}
