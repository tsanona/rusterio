#![allow(dead_code)]

use gdal::{
    errors::GdalError, raster::Buffer, Dataset, GeoTransform, GeoTransformEx, Metadata,
    MetadataEntry,
};
use ndarray::{Array2, ShapeError};
use proj::{Proj, ProjCreateError};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use thiserror::Error;
use tokio_stream::StreamExt;

#[derive(Error, Debug)]
pub enum RasterError {
    #[error(transparent)]
    GdalError(#[from] GdalError),
    #[error(transparent)]
    ShapeError(#[from] ShapeError),
    #[error(transparent)]
    ProjError(#[from] ProjCreateError),
    #[error("Band `{0}` not found.")]
    BandNotFound(String),
    #[error("Couldn't find metadata key {key} in dataset {dataset_path}.")]
    MetadataKeyNotFound { dataset_path: String, key: String },
}

#[derive(Debug)]
pub struct Raster {
    path: PathBuf,
    metadata: RasterMetadata,
    bands_info: HashMap<String, BandInfo>,
}

impl Raster {
    fn dataset(&self) -> gdal::errors::Result<Dataset> {
        Dataset::open(&self.path)
    }
}

#[derive(Debug)]
struct BandInfo {
    index: usize,
    metadata: BandMetadata,
    bounds: Bounds,
    projection: Proj,
    geo_transform: GeoTransform,
    path: PathBuf,
}

impl BandInfo {
    fn dataset(&self) -> gdal::errors::Result<Dataset> {
        Dataset::open(&self.path)
    }
    fn resolution(&self) -> usize {
        self.geo_transform[1] as usize
    }
}

#[derive(Debug)]
struct Bounds {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
}

type RasterMetadata = HashMap<String, String>;
type BandMetadata = HashMap<String, String>;
type RasterSubDatasets = Vec<Dataset>;

impl Raster {
    const BANDNAME_KEY: &'static str = "BANDNAME";

    pub fn new<P: AsRef<Path>>(path: P) -> Result<Raster, RasterError> {
        let dataset = Dataset::open(&path)?;
        let (metadata, subdatasets) = Self::parse_dataset(&dataset)?;
        let bands_info = subdatasets
            .into_iter()
            // Don't use tci bands
            .filter(|dataset| !dataset.description().unwrap().contains("TCI"))
            .map(Self::parse_subdataset)
            .filter_map(Result::ok)
            .flatten()
            .collect();
        dataset.close()?;
        Ok(Raster {
            path: path.as_ref().to_path_buf(),
            metadata,
            bands_info,
        })
    }

    fn parse_dataset(
        dataset: &Dataset,
    ) -> Result<(RasterMetadata, RasterSubDatasets), RasterError> {
        let mut raster_metadata = RasterMetadata::new();
        let mut raster_subdataset_paths = RasterSubDatasets::new();
        for MetadataEntry { domain, key, value } in dataset.metadata() {
            match domain.as_str() {
                "" => {
                    raster_metadata.insert(key, value);
                }
                "SUBDATASETS" if key.contains("NAME") => {
                    raster_subdataset_paths.push(Dataset::open(value)?)
                }
                _ => continue,
            }
        }
        Ok((raster_metadata, raster_subdataset_paths))
    }

    fn parse_subdataset(dataset: Dataset) -> Result<HashMap<String, BandInfo>, RasterError> {
        let mut bands_info = HashMap::new();
        for (idx, raster_band) in dataset.rasterbands().filter_map(Result::ok).enumerate() {
            let mut metadata = BandMetadata::new();
            for MetadataEntry { domain, key, value } in raster_band.metadata() {
                match domain.as_str() {
                    "" => {
                        metadata.insert(key, value);
                    }
                    _ => continue,
                }
            }
            let projection = Proj::new(dataset.spatial_ref()?.to_proj4()?.as_str())?;
            let geo_transform = dataset.geo_transform()?;
            let bounds =
                Self::get_extent(raster_band.x_size(), raster_band.y_size(), geo_transform);
            let dataset_path = dataset.description()?;
            bands_info.insert(
                metadata
                    .get(Self::BANDNAME_KEY)
                    .ok_or(RasterError::MetadataKeyNotFound {
                        key: String::from(Self::BANDNAME_KEY),
                        dataset_path,
                    })?
                    .to_string(),
                BandInfo {
                    index: idx + 1,
                    metadata,
                    bounds,
                    projection,
                    geo_transform,
                    path: dataset.description()?.into(),
                },
            );
        }
        dataset.close()?;
        Ok(bands_info)
    }

    fn get_extent(width: usize, height: usize, geo_transform: GeoTransform) -> Bounds {
        let (xmin, ymin) = geo_transform.apply(0.0, 0.0);
        let (xmax, ymax) = geo_transform.apply(width as f64, height as f64);
        Bounds {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }

    // TODO: try using threads for reading bands/blocks (tokio stream?, vanilla spawn?)
    pub async fn get_array_async(
        &self,
        band_names: Vec<&str>,
        offset: (isize, isize),
        size: (usize, usize),
    ) -> Vec<Result<Buffer<u16>, RasterError>> {
        tokio_stream::iter(band_names)
            // TODO: hadle with unsuccessfull band dataset open calls
            .map(|band_name| {
                let band_info = self
                    .bands_info
                    .get(band_name)
                    .ok_or(RasterError::BandNotFound(String::from(band_name)))?;
                band_info
                    .dataset()?
                    .rasterband(band_info.index)?
                    .read_as(offset, size, size, None)
                    .map_err(RasterError::GdalError)
            })
            .collect()
            .await
    }

    pub fn get_array(
        &self,
        band_names: Vec<&str>,
        offset: (isize, isize),
        size: (usize, usize),
    ) -> Vec<Result<Buffer<u16>, RasterError>> {
        band_names
            .into_iter()
            // TODO: hadle with unsuccessfull band dataset open calls
            .map(|band_name| {
                let band_info = self
                    .bands_info
                    .get(band_name)
                    .ok_or(RasterError::BandNotFound(String::from(band_name)))?;
                band_info
                    .dataset()?
                    .rasterband(band_info.index)?
                    .read_as::<u16>(offset, size, size, None)
                    .map_err(RasterError::GdalError)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    const TEST_DATA: &str =
        "data/S2B_MSIL2A_20241206T093309_N0511_R136_T33PTM_20241206T115919.SAFE.zip";

    #[fixture]
    fn test_raster() -> Raster {
        Raster::new(TEST_DATA).unwrap()
    }

    #[rstest]
    fn it_works(test_raster: Raster) {
        let b1 = test_raster.bands_info.get("B2").unwrap();
        //let b2 = raster.bands_info.get("B2").unwrap();
        //let (p1_x, p1_y) = b1.geo_transform.apply(0.0, 0.0);
        //let (p2_x, p2_y) = b1.geo_transform.apply(0.0, 1.0);
        //println!("{:?}", (p1_x - p2_x, p1_y - p2_y));
        //let polygon = geo::Polygon::<f64>::try_from_wkt_str(raster.metadata.get("FOOTPRINT").unwrap().as_str()).unwrap();
        //polygon = polygon.affine_transform(&geo::AffineTransform::from(raster.dataset().unwrap().geo_transform().unwrap()));
        //println!("{:#?}", raster.metadata.get("FOOTPRINT"));
        println!("{:#?}", b1.bounds);
    }

    #[rstest]
    #[tokio::test]
    async fn play_ground(test_raster: Raster) {
        let res = test_raster.get_array_async(vec!["B4", "B3", "B2"], (0, 0), (256, 256));
        println!("{:#?}", res.await)
    }
}
