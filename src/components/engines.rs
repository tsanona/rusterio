use geo::AffineTransform;
use num::Num;
use std::{collections::HashMap, fmt::Debug, path::Path};

use crate::{
    components::{raster::RasterBand, BandReader, File},
    errors::{Result, RusterioError},
    Indexes, Raster,
};

pub trait DataType: Num + From<bool> + Clone + Copy + Send + Sync + Debug {}
impl DataType for u16 {}

/// Implementations for gdal
pub mod gdal_engine {
    use super::*;
    use gdal::{
        raster::GdalType, Dataset as GdalDataset, Metadata as GdalMetadata,
        MetadataEntry as GdalMetadataEntry,
    };
    use ndarray::{Array2, ArrayView2};
    use std::{marker::PhantomData, path::PathBuf, sync::Arc};

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

    #[derive(thiserror::Error, Debug)]
    pub enum GdalEngineError {
        #[error("Driver {0} can not be used for this path.")]
        WrongDriver(String),
    }

    pub trait GdalDataType: DataType + GdalType {}
    impl GdalDataType for u16 {}

    pub fn open<T: GdalDataType, P: AsRef<Path>>(path: P) -> Result<Raster<T>> {
        let dataset = GdalDataset::open(&path)?;
        if let Ok(raster) = Raster::new::<GdalFile<T>, _>(path, Indexes::from([]), true) {
            return Ok(raster);
        } else {
            match dataset.driver().short_name().as_str() {
                // TODO: Probably there is a better way to do this
                "SENTINEL2" => {
                    let sub_dataset_paths = (1..=3)
                        .into_iter()
                        .map(|sub_dataset_idx| {
                            // Items should exist always
                            dataset
                                .metadata_item(
                                    format!("SUBDATASET_{sub_dataset_idx}_NAME").as_str(),
                                    "SUBDATASETS",
                                )
                                .unwrap()
                        })
                        .zip([
                            (Indexes::from([]), true),
                            (Indexes::from([]), true),
                            (Indexes::from([0usize, 1]), false),
                        ])
                        .map(|(path, (indexes, drop))| {
                            Raster::new::<GdalFile<T>, _>(path, indexes, drop)
                        })
                        .collect::<Result<Vec<_>>>()?;
                    return Raster::stack(sub_dataset_paths);
                }
                _ => unimplemented!(),
            }
        }
    }

    #[derive(Debug)]
    pub struct GdalFile<T: GdalDataType> {
        _t: PhantomData<T>,
        path: PathBuf,
        dataset: GdalDataset,
    }

    impl<T: GdalDataType> File for GdalFile<T> {
        type T = T;
        fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            Ok(GdalFile {
                path: path.as_ref().to_path_buf(),
                dataset: GdalDataset::open(&path)?,
                _t: PhantomData,
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
        fn num_bands(&self) -> usize {
            self.dataset.raster_count()
        }
        fn band(&self, index: usize) -> Result<RasterBand<Self::T>> {
            let raster_band = self.dataset.rasterband(index + 1)?;
            let description = raster_band.description()?;
            let mut metadata = filter_metadata_gdal(&raster_band);
            let name = metadata.remove("BANDNAME").unwrap(); // TODO: this is sentinel2 data specific... generalize!
            Ok(RasterBand {
                description,
                name,
                metadata,
                reader: Arc::new(Box::new(GdalReader(self.path.clone(), index + 1))),
            })
        }
        fn metadata(&self) -> HashMap<String, String> {
            filter_metadata_gdal(&self.dataset)
        }
    }

    #[derive(Debug)]
    struct GdalReader(PathBuf, usize);

    impl<T: GdalDataType> BandReader<T> for GdalReader {
        fn read_window_as_array(
            &self,
            offset: (usize, usize),
            size: (usize, usize),
            mask: Option<ArrayView2<bool>>,
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
            let dataset = GdalDataset::open(&self.0)?;
            let rasterband = dataset.rasterband(self.1)?;
            if T::gdal_ordinal() != rasterband.band_type() as u32 {
                Err(gdal::errors::GdalError::BadArgument(
                    "result array type must match band data types".to_string(),
                ))?
            }
            let buffer = rasterband.read_as::<T>(
                (offset.0 as isize, offset.1 as isize),
                size,
                size,
                None,
            )?;
            Array2::from_shape_vec(size, buffer.data().to_vec())
                .map_err(RusterioError::NdarrayError)
                .map(|read| array * read)
        }

        /* fn read_band_block_as_array(
            &self,
            index: (usize, usize),
            band_index: usize,
            mask: &Option<ArrayView2<'a, bool>>,
        ) -> Result<Array2<T>>  */
    }
}
