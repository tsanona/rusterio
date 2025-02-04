use crate::sensors::Sensor;
use super::{
    band::{BandInfo, Bands},
    Result,
};

#[derive(Debug)]
pub struct Raster<S: Sensor> {
    bands: Bands<S::BandMetadata>,
    metadata: S::RasterMetadata
}

impl<S: Sensor> Raster<S> {
    pub fn new(bands: Bands<S::BandMetadata>, metadata: S::RasterMetadata) -> Self {
        Self { bands, metadata }
    }

    fn get_band_info(&self, band_name: &String) -> Result<&BandInfo<S::BandMetadata>> {
        self.bands.get(band_name)
    }

    // todo get intermidiate representation with each array that can be latter conacated
    /* pub fn read_bands(
        &self,
        bands: Vec<&'static str>,
        offset: (isize, isize),
        window: (usize, usize),
    ) -> Result<Array3<u16>> {

        let bands_info= bands.into_iter().map(|band_name| self.get_band(&band_name.to_string()));

        let band_rasters = bands_info
            //.par_bridge()
            .map(|band_info| {
                let transform = band_info?
                    .group
                    .geo_transform
                    .try_inverse()
                    .ok_or(RasterError::BandTransformNotInvertible((*band).into()))?
                    * self.highest_resolution_transform;
                let (corrected_offset, corrected_window) = transform_window(
                    (offset, window),
                    transform,
                    band_info.dataset()?.raster_size(),
                );
                band_info?.reader()
                    .read_as_array::<u16>(corrected_offset, corrected_window)
                    .map(|band_raster| (band_raster, transform))
                    .map_err(RasterError::RastersError)
            })
            .collect::<Result<Vec<(Array2<u16>, PixelTransform)>>>()?;

        Ok(Array3::from_shape_fn(
            (bands.len(), window.0, window.1),
            |(c, x, y)| {
                let (band_raster, transform) = &band_rasters[c];
                let corrected_coords = transform.transform_point(&Point2::new(x as f64, y as f64));
                band_raster[[corrected_coords.x as usize, corrected_coords.y as usize]]
            },
        ))
    } */
}
