#![allow(dead_code)]
mod components;
mod errors;
mod sensors;

#[cfg(test)]
mod tests {
    use crate::sensors::Sensor;

    use super::*;
    use components::{parser::DatasetParser, raster::Raster};
    use sensors::sentinel2;
    use rstest::{fixture, rstest};

    const TEST_DATA: &str =
        "data/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE.zip";

    #[fixture]
    pub fn test_raster() -> Raster<impl Sensor> {
        sentinel2::Parser::parse_dataset(TEST_DATA).unwrap()
    }

    #[rstest]
    fn play_ground(test_raster: Raster<impl Sensor>) {
        println!("{:#?}", test_raster)
    }

    /* #[rstest]
    fn it_works(test_raster: Raster) {
        print!(
            "{:#?}",
            test_raster
                .read_bands(vec!["B4", "B3", "B2"], (0, 0), (125, 125))
                .unwrap()
        );
    } */

    /* #[rstest]
    fn to_npy(test_raster: Raster) {
        let rgb = ((test_raster
            .read_bands(vec!["B4", "B3", "B2"], (0, 0), (100, 100))
            .unwrap()
            .reversed_axes()
            / 100)
            * 255)
            / 100;

        write_npy("dev/test.npy", &rgb).unwrap();
        //println!("{:?}", rgb);
    } */
}
