mod components;
mod errors;
mod sensors;

pub use components::reader::DatasetReader;
pub use sensors::Sentinel2;

#[cfg(test)]
mod tests {
    use super::*;
    use components::{reader::DatasetReader, raster::Raster};
    use rstest::{fixture, rstest};
    use sensors::Sentinel2;

    const TEST_DATA: &str =
        "data/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE.zip";

    #[fixture]
    pub fn test_raster() -> Raster<Sentinel2> {
        Sentinel2::raster_from(TEST_DATA).unwrap()
    }

    #[rstest]
    fn play_ground(test_raster: Raster<Sentinel2>) {
        println!("{:#?}", test_raster.read_bands(vec!["B4", "B2", "B3"], (0, 0), (125, 125)).unwrap().dim())
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
