use super::{band::Bands, raster::Raster, Result, Sentinel2ArrayError};
use crate::sensors::Sensor;
use gdal;
use std::path::Path;

pub trait DatasetReader: Sensor + Sized {
    fn raster_from<P: AsRef<Path>>(path: P) -> Result<Raster<Self>> {
        let dataset = Self::open_dataset(path)?;
        Self::read_dataset(dataset)
            .map(|(bands, raster_metadata)| Raster::new(bands, raster_metadata))
    }

    fn open_dataset<P: AsRef<Path>>(path: P) -> Result<gdal::Dataset> {
        let dataset = gdal::Dataset::open(path)?;
        let dataset_driver = dataset.driver().short_name();
        if dataset_driver.eq_ignore_ascii_case(Self::GDAL_DRIVER_NAME) {
            Ok(dataset)
        } else {
            Err(Sentinel2ArrayError::WrongParser {
                parser: Self::GDAL_DRIVER_NAME.into(),
                dataset: dataset_driver,
            })
        }
    }

    fn read_dataset(dataset: gdal::Dataset)
        -> Result<(Bands<Self::BandMetadata>, Self::RasterMetadata)>;
}
