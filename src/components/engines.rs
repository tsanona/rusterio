use std::{collections::HashMap, fmt::Debug, path::Path, rc::Rc};

use crate::{
    components::{
        band::{BandInfo, BandReader},
        file::File,
        raster::RasterBand,
        transforms::BandGeoTransform,
        DataType, Metadata,
    },
    errors::{Result, RusterioError},
    Indexes, Raster,
};

/// Implementations for gdal
pub mod gdal_engine {
    use super::*;
    use gdal::{
        raster::GdalType, Dataset as GdalDataset, Metadata as GdalMetadata,
        MetadataEntry as GdalMetadataEntry,
    };
    use ndarray::{Array2, ArrayView2};
    use std::{marker::PhantomData, sync::Arc};

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

    trait GdalDriver: Debug {
        fn band_name(raster_band: gdal::raster::RasterBand) -> String;
        fn open<P: AsRef<Path>>(path: P) -> Result<Raster<impl GdalDataType>>;
    }

    pub fn open<T: GdalDataType, P: AsRef<Path>>(path: P) -> Result<Raster<T>> {
        if let Ok(raster) = Raster::new::<GdalFile<T>, _>(&path, Indexes::all()) {
            return Ok(raster);
        } else {
            let dataset = GdalDataset::open(&path)?;
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
                            (Indexes::all()),
                            (Indexes::all()),
                            (Indexes::from([0usize, 1])),
                        ])
                        .map(|(path, indexes)| Raster::new::<GdalFile<T>, _>(path, indexes))
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
        path: Arc<Path>,
        dataset: Rc<GdalDataset>,
    }

    impl<T: GdalDataType> File<T> for GdalFile<T> {
        fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            let dataset = Rc::new(GdalDataset::open(&path)?);
            Ok(GdalFile {
                path: Arc::from(path.as_ref()),
                dataset,
                _t: PhantomData,
            })
        }
        fn description(&self) -> Result<String> {
            Ok(self.dataset.description()?)
        }
        fn size(&self) -> (usize, usize) {
            self.dataset.raster_size()
        }
        fn crs(&self) -> Rc<str> {
            Rc::from(self.dataset.projection())
        }
        fn transform(&self) -> Result<BandGeoTransform> {
            let gdal_transform = self.dataset.geo_transform()?;
            Ok(BandGeoTransform::new(
                gdal_transform[1],
                gdal_transform[2],
                gdal_transform[0],
                gdal_transform[4],
                gdal_transform[5],
                gdal_transform[3],
                self.crs(),
            ))
        }
        fn num_bands(&self) -> usize {
            self.dataset.raster_count()
        }
        fn metadata(&self) -> HashMap<String, String> {
            filter_metadata_gdal(self.dataset.as_ref())
        }
        fn band(&self, index: usize) -> Result<RasterBand<T>> {
            let info: Rc<dyn BandInfo> =
                Rc::new(GdalBandInfo(Rc::clone(&self.dataset), index + 1));
            let reader: Arc<dyn BandReader<T>> =
                Arc::new(GdalBandReader(Arc::clone(&self.path), index + 1));
            Ok(RasterBand { info, reader })
        }
    }

    #[derive(Debug)]
    struct GdalBandInfo(Rc<gdal::Dataset>, usize);

    impl<'a> BandInfo for GdalBandInfo {
        fn description(&self) -> Result<String> {
            Ok(self.0.rasterband(self.1)?.description()?)
        }

        fn name(&self) -> String {
            match self.0.driver().short_name().as_str() {
                "SENTINEL2" => return self.metadata().unwrap().remove("BANDNAME").unwrap(),
                _ => unimplemented!(),
            };
        }

        fn metadata(&self) -> Result<Metadata> {
            Ok(filter_metadata_gdal(&self.0.rasterband(self.1)?))
        }
    }

    #[derive(Debug)]
    struct GdalBandReader(Arc<Path>, usize);

    impl<T: GdalDataType> BandReader<T> for GdalBandReader {
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
                array = Array2::ones((size.1, size.0));
            }
            let dataset = GdalDataset::open(&self.0)?;
            let rasterband = dataset.rasterband(self.1)?;
            if T::gdal_ordinal() != rasterband.band_type() as u32 {
                Err(gdal::errors::GdalError::BadArgument(
                    "result array type must match band data types".to_string(),
                ))?
            }
            let buffer = rasterband.read_as::<T>(
                (offset.1 as isize, offset.0 as isize),
                size,
                size,
                None,
            )?;
            Array2::from_shape_vec(size, buffer.data().to_vec())
                .map_err(RusterioError::NdarrayError)
                // Array2 shape is (rows, cols) and Buffer shape is (cols in x-axis, rows in y-axis)
                // thus needs T to correct orientation
                .map(|read| array * read.t())
        }

        /* fn read_band_block_as_array(
            &self,
            index: (usize, usize),
            band_index: usize,
            mask: &Option<ArrayView2<'a, bool>>,
        ) -> Result<Array2<T>>  */
    }
}
