use geo::AffineTransform;
use std::{collections::HashMap, fmt::Debug, path::Path};

use crate::{
    components::{readers::Reader, Band},
    errors::Result,
};

pub trait File: Debug + Sized {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    fn description(&self) -> Result<String>;
    fn size(&self) -> (usize, usize);
    fn crs(&self) -> String;
    fn transform(&self) -> Result<AffineTransform>;
    fn bands(&self) -> Result<Vec<Band>>;
    fn metadata(&self) -> HashMap<String, String>;
    fn reader(&self) -> Result<impl Reader>;
}

pub mod gdal_backend {
    use std::path::PathBuf;

    use super::*;
    use gdal::{
        errors::Result as GdalResult, Dataset as GdalDataset, Metadata as GdalMetadata,
        MetadataEntry as GdalMetadataEntry,
    };

    fn affine_from_gdal(gdal_transform: [f64; 6]) -> AffineTransform {
        AffineTransform::new(
            gdal_transform[1],
            gdal_transform[2],
            gdal_transform[0],
            gdal_transform[4],
            gdal_transform[5],
            gdal_transform[3],
        )
    }

    fn filter_metadata_gdal(metadata: &impl GdalMetadata) -> HashMap<String, String> {
        GdalMetadata::metadata(metadata)
            .filter_map(|GdalMetadataEntry { domain, key, value }| {
                if domain.eq("") {
                    Some((key, value))
                } else {
                    None
                }
            })
            .collect()
    }

    #[derive(Debug)]
    pub struct GdalFile {
        path: PathBuf,
        dataset: GdalDataset,
    }

    impl File for GdalFile {
        fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            Ok(GdalFile {
                path: path.as_ref().to_path_buf(),
                dataset: GdalDataset::open(&path)?,
            })
        }
        fn description(&self) -> Result<String> {
            Ok(GdalMetadata::description(&self.dataset)?)
        }
        fn size(&self) -> (usize, usize) {
            self.dataset.raster_size()
        }
        fn crs(&self) -> String {
            self.dataset.projection()
        }
        fn transform(&self) -> Result<AffineTransform> {
            Ok(affine_from_gdal(self.dataset.geo_transform()?))
        }
        fn bands(&self) -> Result<Vec<Band>> {
            let mut bands = Vec::new();
            for raster_band in self.dataset.rasterbands().collect::<GdalResult<Vec<_>>>()? {
                let metadata = filter_metadata_gdal(&raster_band);
                let name = raster_band.description()?;
                //let metadata = filter_metadata_gdal(&raster_band_result?);
                bands.push(Band::new(name, metadata));
            }
            Ok(bands)
        }
        fn metadata(&self) -> HashMap<String, String> {
            filter_metadata_gdal(&self.dataset)
        }
        fn reader(&self) -> Result<impl Reader> {
            Ok(GdalDataset::open(&self.path)?)
        }
    }
}
