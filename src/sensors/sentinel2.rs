use gdal::Metadata as GdalMetadata;
use itertools::Itertools;
use std::sync::Arc;

use crate::{
    components::{
        band::{BandGroup, BandInfo, Bands},
        metadata::Metadata,
        reader::DatasetReader,
    },
    errors::{Result, Sentinel2ArrayError},
};

use super::Sensor;

#[derive(Debug)]
pub struct Sentinel2;

impl Sensor for Sentinel2 {
    type RasterMetadata = RasterMetadata;
    type BandMetadata = BandMetadata;

    const GDAL_DRIVER_NAME: &'static str = "Sentinel2";
}

impl DatasetReader for Sentinel2 {
    fn read_dataset(dataset: gdal::Dataset) -> Result<(Bands<BandMetadata>, RasterMetadata)> {
        let (metadata, bandgroup_datasets) = Self::parse_raster_metadata(&dataset)?;
        let bands = bandgroup_datasets
            .iter()
            .map(Self::read_bandgroup_dataset)
            .process_results(|iter| Bands::from_iter(iter.flatten()))?;
        Ok((bands, metadata))
    }
}

impl Sentinel2 {
    fn parse_raster_metadata(
        raster_dataset: &gdal::Dataset,
    ) -> Result<(RasterMetadata, Vec<gdal::Dataset>)> {
        let mut raster_metadata = RasterMetadata::new(raster_dataset.description()?);
        let mut subdatasets = Vec::new();
        for gdal::MetadataEntry { domain, key, value } in raster_dataset.metadata() {
            match domain.as_str() {
                "" => raster_metadata.0.insert(key, value),
                "SUBDATASETS" if key.contains("NAME") => {
                    subdatasets.push(gdal::Dataset::open(value)?)
                }
                _ => (),
            };
        }
        Ok((raster_metadata, subdatasets))
    }

    fn read_bandgroup_dataset<'a>(
        bandgroup_dataset: &'a gdal::Dataset,
    ) -> Result<Vec<(String, BandInfo<BandMetadata>)>> {
        let band_group = Arc::new(BandGroup::new(&bandgroup_dataset)?);
        bandgroup_dataset
            .rasterbands()
            .enumerate()
            .map(|(index, raster_band)| {
                let (band_name, metadata) = Self::parse_rasterband_metadata(raster_band?)?;
                Ok((
                    band_name,
                    BandInfo::new(Arc::clone(&band_group), index + 1, metadata),
                ))
            })
            .collect()
    }

    fn parse_rasterband_metadata(
        raster_band: gdal::raster::RasterBand,
    ) -> Result<(String, BandMetadata)> {
        let mut band_name = String::new();
        let mut  metadata = BandMetadata::new(raster_band.description()?);
        for gdal::MetadataEntry { domain, key, value } in raster_band.metadata() {
            if matches!(domain.as_str(), "") {
                // Should only exist one.
                if matches!(key.as_str(), "BANDNAME") {
                    band_name.push_str(&value);
                } else {
                    metadata.0.insert(key, value);
                }
            }
        }
        Ok((band_name, metadata))
    }
}

#[derive(Debug)]
pub struct RasterMetadata(Metadata);
impl RasterMetadata {
    pub fn new(description: String) -> Self {
        Self(Metadata::new(description))
    }

    pub fn footprint(&self) -> Result<GeometryWCRS> {
        gdal::vector::Geometry::from_wkt(self.0.get("FOOTPRINT")?)?
            .to_geo()
            .map(|geometry| GeometryWCRS {
                geometry,
                crs: "EPSG:4326".into(),
            })
            .map_err(Sentinel2ArrayError::GdalError)
    }
}

#[derive(Debug)]
pub struct GeometryWCRS {
    pub geometry: geo::Geometry,
    pub crs: String,
}

#[derive(Debug, Default)]
pub struct BandMetadata(Metadata);

impl BandMetadata {
    pub fn new(description: String) -> Self {
        Self(Metadata::new(description))
    }
}
