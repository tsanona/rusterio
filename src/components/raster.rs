#![allow(dead_code)]

use rasters::{prelude::{transform_window, PixelTransform}, reader::ChunkReader};
use rayon::{iter::{IntoParallelRefIterator, ParallelBridge, ParallelIterator}, prelude::IntoParallelIterator, slice::ParallelSlice};
use ndarray::{Array2, Array3};
use nalgebra::Point2;
use super::{
    band::{BandInfo, Bands},
    Result,
    Sentinel2ArrayError
};
use crate::sensors::Sensor;

#[derive(Debug)]
pub struct Raster<S: Sensor> {
    bands: Bands<S::BandMetadata>,
    pub metadata: S::RasterMetadata,
}

impl<S: Sensor> Raster<S> {
    pub fn new(bands: Bands<S::BandMetadata>, metadata: S::RasterMetadata) -> Self {
        Self { bands, metadata }
    }

    fn get_band_info(&self, band_name: &'static str) -> Result<&BandInfo<S::BandMetadata>> {
        self.bands.get(band_name)
    }

    fn read_band(&self,
        band_info: &BandInfo<S::BandMetadata>,
        off: (isize, isize),
        size: (usize, usize),
    ) -> Result<Array2<u16>> {
       band_info.reader().read_as_array(off, size).map_err(Sentinel2ArrayError::RastersError)
    }

    pub fn read_bands(
        &self,
        band_names: Vec<&'static str>,
        offset: (isize, isize),
        window: (usize, usize),
    ) -> Result<Array3<u16>> {
        let bands_info = band_names.iter().map(|band_name| self.get_band_info(band_name)).collect::<Result<Vec<&BandInfo<S::BandMetadata>>>>()?;
        let highest_resolution_transform = bands_info.iter().map(|band_info| band_info.geo_transform()).reduce(|prev, next| if next.m11 < prev.m11 {next} else {prev}).unwrap();
        let band_rasters = bands_info.into_par_iter().map(
            |band_info| {
                let transform = band_info
                    .geo_transform()
                    .try_inverse().unwrap()
                    * highest_resolution_transform;
                    let (cor_off, cor_size) = transform_window(
                        (offset, window),
                        transform,
                        band_info.raster_size(),
                    );
                   self.read_band(band_info, cor_off, cor_size).map(|array| (array, transform))
            }
        ).collect::<Result<Vec<(Array2<u16>, PixelTransform)>>>()?;

        Ok(Array3::from_shape_fn(
            (band_rasters.len(), window.0, window.1),
            |(c, x, y)| {
                let (band_raster, transform) = &band_rasters[c];
                let corrected_coords = transform.transform_point(&Point2::new(x as f64, y as f64));
                band_raster[[corrected_coords.x as usize, corrected_coords.y as usize]]
            },
        ))
    }
}
