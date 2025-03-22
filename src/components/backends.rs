use geo::AffineTransform;
use std::{collections::HashMap, fmt::Debug, path::Path};

use crate::{
    components::{Band, File, Reader},
    errors::Result,
};

pub mod gdal_backend {
    use super::*;
    use gdal::{
        errors::Result as GdalResult, raster::GdalType, Dataset as GdalDataset,
        Metadata as GdalMetadata, MetadataEntry as GdalMetadataEntry,
    };
    use ndarray::Array2;
    use num::Num;
    use std::path::PathBuf;

    use crate::errors::RusterioError;

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
                let block_size = raster_band.block_size();
                //let metadata = filter_metadata_gdal(&raster_band_result?);
                bands.push(Band::new(name, metadata, block_size));
            }
            Ok(bands)
        }
        fn metadata(&self) -> HashMap<String, String> {
            filter_metadata_gdal(&self.dataset)
        }
        fn reader(&self) -> impl Reader {
            // For object to exist, this should have been successful.
            GdalReader(self.path.to_path_buf())
        }
    }

    struct GdalReader(PathBuf);

    impl Reader for GdalReader {
        fn read_band_window_as_array<
            T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync,
        >(
            &self,
            band_index: usize,
            offset: (isize, isize),
            size: (usize, usize),
            mask: &Option<Array2<bool>>,
        ) -> Result<Array2<T>> {
            let array;
            if let Some(mask) = mask {
                if mask.mapv(i8::from).sum().eq(&0) {
                    return Ok(Array2::zeros(size));
                } else {
                    array = mask.mapv(T::from)
                }
            } else {
                array = Array2::ones(size);
            }
            let buffer = GdalDataset::open(&self.0)?
                .rasterband(band_index + 1)?
                .read_as::<T>(offset, size, size, None)?;
            Array2::from_shape_vec(size, buffer.data().to_vec())
                .map_err(RusterioError::NdarrayError)
                .map(|read| array * read)
        }

        fn read_band_block_as_array<T: GdalType + Num + From<bool> + Clone + Copy + Send + Sync>(
            &self,
            index: (usize, usize),
            band_index: usize,
            mask: &Option<Array2<bool>>,
        ) -> Result<Array2<T>> {
            let dataset = GdalDataset::open(&self.0)?;
            let rasterband = dataset.rasterband(band_index + 1)?;
            let array;
            if let Some(mask) = mask {
                if mask.mapv(i8::from).sum().eq(&0) {
                    return Ok(Array2::zeros(rasterband.block_size()));
                } else {
                    array = mask.mapv(T::from)
                }
            } else {
                array = Array2::ones(rasterband.block_size());
            }
            let buffer = rasterband.read_block::<T>(index)?;
            let buf_size = buffer.shape();
            Array2::from_shape_vec((buf_size.1, buf_size.0), buffer.data().to_vec())
                .map_err(RusterioError::NdarrayError)
                .map(|read| array * read)
        }
    }
}
