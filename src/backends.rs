use geo::AffineTransform;
use ndarray::{s, Array2, Array3};
use num::Num;
use std::{collections::HashMap, path::Path};

use crate::{
    components::{Band, Dataset, Reader},
    errors::Result,
    tuple_to,
};

pub fn open_dataset<P: AsRef<Path>>(path: P) -> Result<impl Dataset> {
    //TODO match to other backends.
    gdal_backend::open(path)
}

mod gdal_backend {
    use std::path::PathBuf;

    use super::*;
    use gdal::{
        errors::Result as GdalResult, raster::GdalType,
        Metadata as GdalMetadata, MetadataEntry as GdalMetadataEntry,
        Dataset as GdalDataset
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

    pub fn open<P: AsRef<Path>>(path: P) -> Result<GdalDataset> {
        Ok(GdalDataset::open(path)?)
    }

    impl Dataset for GdalDataset {
        fn description(&self) -> Result<String> {
            Ok(GdalMetadata::description(self)?)
        }
        fn size(&self) -> (usize, usize) {
            self.raster_size()
        }
        fn crs(&self) -> String {
            self.projection()
        }
        fn transform(&self) -> Result<AffineTransform> {
            Ok(affine_from_gdal(self.geo_transform()?))
        }
        fn bands(&self) -> Result<Vec<Band>> {
            let mut bands = Vec::new();
            for raster_band in self.rasterbands().collect::<GdalResult<Vec<_>>>()? {
                let metadata = filter_metadata_gdal(&raster_band);
                let name = raster_band.description()?;
                //let metadata = filter_metadata_gdal(&raster_band_result?);
                bands.push(Band::new(name, metadata));
            }
            Ok(bands)
        }
        fn metadata(&self) -> HashMap<String, String> {
            filter_metadata_gdal(self)
        }
    }

    pub struct GdalReader(PathBuf);

    impl Reader for GdalReader {
        fn read_window<T: Num + Clone + Copy + GdalType>(
            &self,
            band_indexes: &[usize],
            offset: (usize, usize),
            size: (usize, usize),
        ) -> Result<Array3<T>> {
            let dataset = GdalDataset::open(self.0.clone())?;
            let shape = (band_indexes.len(), size.0, size.1);
            let mut array = Array3::zeros(shape);
            for band_index in band_indexes {
                let buf = dataset.rasterband(*band_index)?.read_as::<T>(
                    tuple_to(offset),
                    size,
                    size,
                    None,
                )?;
                let buf_shape = buf.shape();
                array
                    .slice_mut(s![*band_index, .., ..])
                    .assign(&Array2::from_shape_vec(
                        (buf_shape.1, buf_shape.0),
                        buf.data().to_vec(),
                    )?)
            }
            Ok(array)
        }
    }
}
