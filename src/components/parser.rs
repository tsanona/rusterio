use gdal;
use crate::sensors::Sensor;
use super::{raster::Raster, Result};
use std::path::Path;

pub trait DatasetParser<S: Sensor> {
    fn using_correct_parser(&self, dataset: &gdal::Dataset) {
        assert!(dataset.driver().short_name().eq_ignore_ascii_case(S::GDAL_DRIVER_NAME));
    }

    fn parse_dataset<P: AsRef<Path>>(path: P) -> Result<Raster<S>>;
}
