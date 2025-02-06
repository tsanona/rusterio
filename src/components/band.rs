use gdal::Metadata as GdalMetadata;
use rasters::prelude::{transform_from_gdal, PixelTransform, RasterPathReader};
use std::{
    collections::{hash_map::Entry, HashMap},
    path::PathBuf,
    rc::Rc,
};

use super::{Result, Sentinel2ArrayError};

#[derive(Debug)]
pub struct BandGroup {
    gdal_dataset_path: PathBuf,
    crs: String,
    geo_transform: PixelTransform,
    raster_size: (usize, usize)
}

impl BandGroup {
    pub fn new(dataset: &gdal::Dataset) -> Result<Self> {
        let gdal_dataset_path = dataset.description()?.into();
        let crs = dataset.projection();
        dataset
            .geo_transform()
            .map(|geo_transform| Self {
                gdal_dataset_path,
                crs,
                geo_transform: transform_from_gdal(&geo_transform),
                raster_size: dataset.raster_size()
            })
            .map_err(Sentinel2ArrayError::GdalError)
    }

    fn band_reader(&self, band_index: usize) -> RasterPathReader<PathBuf> {
        RasterPathReader(&self.gdal_dataset_path, band_index)
    }
}

#[derive(Debug)]
pub struct BandInfo<BM> {
    index: usize,
    group: Rc<BandGroup>,
    metadata: BM,
}

impl<BM> BandInfo<BM> {
    pub fn new(group: Rc<BandGroup>, index: usize, metadata: BM) -> Self {
        Self {
            index,
            group,
            metadata,
        }
    }

    pub fn geo_transform(&self) -> PixelTransform {
        self.group.geo_transform
    }

    pub fn raster_size(&self) -> (usize, usize) {
        self.group.raster_size
    }

    pub fn resolution(&self) -> u8 {
        self.geo_transform().m11 as u8
    }

    pub fn reader(&self) -> RasterPathReader<PathBuf> {
        self.group.band_reader(self.index)
    }
}

#[derive(Debug, Default)]
pub struct Bands<BM>(HashMap<String, BandInfo<BM>>);

impl<BM> Bands<BM> {
    pub fn get(&self, band_name: &'static str) -> Result<&BandInfo<BM>> {
        self.0
            .get(band_name)
            .ok_or(Sentinel2ArrayError::BandNotFound(band_name.into()))
    }
    
    pub fn names(&self) -> Vec<&String> {
        let mut names = self.0.keys().collect::<Vec<&String>>();
        names.sort();
        names
    }

    fn insert(mut self, band_name: String, band_info: BandInfo<BM>) -> Self {
        match self.0.entry(band_name) {
            Entry::Occupied(entry) if entry.get().resolution() < band_info.resolution() => entry,
            entry => entry.insert_entry(band_info),
        };
        self
    }
}

impl<BM: Default> FromIterator<(String, BandInfo<BM>)> for Bands<BM> {
    fn from_iter<T: IntoIterator<Item = (String, BandInfo<BM>)>>(iter: T) -> Self {
        iter.into_iter()
            .fold(Bands::default(), |bands, (band_name, band_info)| {
                bands.insert(band_name, band_info)
            })
    }
}
