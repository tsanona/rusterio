use std::{collections::HashMap, fmt::Debug, marker::PhantomData, path::Path, rc::Rc, sync::Arc};

use crate::{
    components::{
        band::{BandInfo, BandReader},
        bounds::{GeoBounds, ReadBounds},
        file::File,
        raster::RasterBand,
        transforms::ReadGeoTransform,
        DataType, Metadata,
    },
    errors::Result,
    try_tuple_cast, Indexes, Raster,
};

/// Implementations for gdal
pub mod gdal_engine {

    use crate::{crs_geo::CrsGeometry, CoordUtils};

    use super::*;
    use gdal::{
        raster::GdalType, Dataset as GdalDataset, Metadata as GdalMetadata,
        MetadataEntry as GdalMetadataEntry,
    };
    use geo::{AffineOps, Point, Rect};
    use geo_traits::RectTrait;
    use log::info;

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

    pub fn open<T: GdalDataType>(path: impl AsRef<Path>) -> Result<Raster<T>> {
        if let Ok(raster) = Raster::new::<GdalFile<T>>(&path, Indexes::all()) {
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
                        .map(|(path, indexes)| Raster::new::<GdalFile<T>>(path, indexes))
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
        fn open(path: impl AsRef<Path>) -> Result<Self> {
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
        fn geo_bounds(&self) -> Result<GeoBounds> {
            let transform = self.transform()?;
            let top_left_geo = geo::Point::new(transform.xoff(), transform.yoff());
            let pixel_shape = Point::<f64>::from(try_tuple_cast(self.dataset.raster_size())?);
            let bottom_right_geo = pixel_shape.affine_transform(&transform);
            let min = (top_left_geo.x(), bottom_right_geo.y());
            let max = (bottom_right_geo.x(), top_left_geo.y());
            let geo_bounds = Rect::new(min, max);
            Ok(GeoBounds::from(CrsGeometry::new(transform.crs, geo_bounds)))
        }

        fn transform(&self) -> Result<ReadGeoTransform> {
            let gdal_transform = self.dataset.geo_transform()?;
            Ok(ReadGeoTransform::new(
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
            let info: Rc<dyn BandInfo> = Rc::new(GdalBandInfo(Rc::clone(&self.dataset), index + 1));
            let reader: Arc<dyn BandReader<T>> =
                Arc::new(GdalBandReader(Arc::clone(&self.path), index + 1));
            Ok(RasterBand { info, reader })
        }
    }

    impl<T: GdalDataType> GdalFile<T> {
        fn crs(&self) -> Rc<Box<str>> {
            Rc::new(Box::from(self.dataset.projection()))
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
        fn read_into_slice(&self, bounds: &ReadBounds, slice: &mut [T]) -> Result<()> {
            let dataset = GdalDataset::open(&self.0)?;
            let rasterband = dataset.rasterband(self.1)?;
            let window_shape = bounds.shape();
            let offset = bounds.min().try_cast()?.x_y();
            //let offset = (offset.0, offset.1);

            //let offset = (offset.0 + window_shape.0 as isize, offset.1);
            /* if T::gdal_ordinal() != rasterband.band_type() as u32 {
                Err(gdal::errors::GdalError::BadArgument(
                    "result array type must match band data types".to_string(),
                ))?
            } */
            info!("reading at offset: {:?}, shape: {:?}", offset, window_shape);
            Ok(rasterband.read_into_slice::<T>(offset, window_shape, window_shape, slice, None)?)
        }
    }
}
