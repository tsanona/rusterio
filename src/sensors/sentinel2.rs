use gdal::Metadata as GdalMetadata;
use itertools::Itertools;
use std::{path::Path, rc::Rc};

use crate::{
    components::{
        band::{BandGroup, BandInfo},
        metadata::Metadata,
        parser::DatasetParser,
        raster::Raster,
    },
    errors::{Sentinel2ArrayError, Result},
};

use super::Sensor;

#[derive(Debug)]
pub struct Sentinel2;
impl Sensor for Sentinel2 {
    type RasterMetadata = RasterMetadata;
    type BandMetadata = BandMetadata;

    const GDAL_DRIVER_NAME: &'static str = "Sentinel2";
}

pub struct Parser;

impl DatasetParser<Sentinel2> for Parser {
    fn parse_dataset<P: AsRef<Path>>(path: P) -> Result<Raster<Sentinel2>> {
        let dataset = gdal::Dataset::open(path)?;
        let (metadata, bandgroup_datasets) = Self::parse_raster_metadata(&dataset)?;
        let bands = bandgroup_datasets
            .iter()
            .map(Self::parse_bandgroup_dataset)
            .process_results(|iter| iter.flatten().collect())?;
        Ok(Raster::new(bands, metadata))
    }
}

impl Parser {
    fn parse_raster_metadata(
        raster_dataset: &gdal::Dataset,
    ) -> Result<(RasterMetadata, Vec<gdal::Dataset>)> {
        raster_dataset
            .description()
            .map(|description| {
                raster_dataset.metadata().fold(
                    (RasterMetadata::new(description), Vec::new()),
                    |mut folded, gdal::MetadataEntry { domain, key, value }| {
                        match domain.as_str() {
                            "" => folded.0.0.insert(key, value),
                            // Subdataset paths are presumed to not fail
                            "SUBDATASETS" if key.contains("NAME") => {
                                folded.1.push(gdal::Dataset::open(value).unwrap())
                            }
                            _ => (),
                        };
                        folded
                    },
                )
            })
            .map_err(Sentinel2ArrayError::GdalError)
    }

    fn parse_bandgroup_dataset<'a>(
        bandgroup_dataset: &'a gdal::Dataset,
    ) -> Result<Vec<(String, BandInfo<BandMetadata>)>> {
        let band_group = Rc::new(BandGroup::new(&bandgroup_dataset)?);
        bandgroup_dataset
            .rasterbands()
            .enumerate()
            .map(|(index, raster_band)| {
                Self::parse_rasterband_metadata(raster_band?).map(|(band_name, metadata)| {
                    (
                        band_name,
                        BandInfo::new(Rc::clone(&band_group), index, metadata),
                    )
                })
            })
            .collect()
    }

    fn parse_rasterband_metadata(
        raster_band: gdal::raster::RasterBand,
    ) -> Result<(String, BandMetadata)> {
        raster_band
            .description()
            .map(|description| {
                raster_band
                    .metadata()
                    .filter_map(|gdal::MetadataEntry { domain, key, value }| {
                        matches!(domain.as_str(), "").then(|| (key, value))
                    })
                    .fold(
                        (String::new(), BandMetadata::new(description)),
                        |mut acc, (key, value)| {
                            if matches!(key.as_str(), "BANDNAME") {
                                acc.0.push_str(&value);
                            } else {
                                acc.1.0.insert(key, value);
                            }
                            acc
                        },
                    )
            })
            .map_err(Sentinel2ArrayError::GdalError)
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