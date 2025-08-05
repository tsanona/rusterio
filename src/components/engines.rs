use std::{collections::HashMap, fmt::Debug, marker::PhantomData, path::Path, rc::Rc, sync::Arc};

use crate::{
    components::{
        band::{BandInfo, BandReader},
        bounds::{Bounds, GeoBounds, ReadBounds},
        file::File,
        raster::band::RasterBand,
        transforms::ReadGeoTransform,
        DataType, Metadata,
    },
    errors::Result,
    try_tuple_cast, Indexes, Raster,
};

/// Implementations for gdal
pub mod gdal_engine {

    use crate::{components::bounds::PixelBounds, crs_geo::CrsGeometry, Buffer, CoordUtils};

    use super::*;
    use gdal::{
        raster::{GdalType, RasterBand as GdalRasterBand},
        Dataset as GdalDataset, Metadata as GdalMetadata, MetadataEntry as GdalMetadataEntry,
    };
    use geo::{AffineOps, Coord, Point, Rect};
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

    use self_cell::self_cell;

    self_cell!(
        struct RasterBandCell {
            owner: GdalDataset,

            #[covariant]
            dependent: GdalRasterBand,
        }
    );

    fn build_rasterband_cell(path: impl AsRef<Path>, idx: usize) -> Result<RasterBandCell> {
        // Create owning String on stack.
        let dataset = GdalDataset::open(path)?;

        // Move String into AstCell, then build Ast inplace.
        Ok(RasterBandCell::try_new(dataset, |dataset| {
            dataset.rasterband(idx)
        })?)
    }

    impl GdalBandReader {
        fn raster_band(&self) -> Result<RasterBandCell> {
            build_rasterband_cell(self.0.as_ref(), self.1)
        }
    }

    impl<T: GdalDataType> BandReader<T> for GdalBandReader {
        fn read_into_slice(&self, bounds: &ReadBounds, slice: &mut [T]) -> Result<()> {
            let rasterband = self.raster_band()?;
            let window_shape = bounds.shape().x_y();
            let offset = bounds.min().try_cast()?.x_y();
            info!("reading at offset: {:?}, shape: {:?}", offset, window_shape);
            Ok(rasterband.borrow_dependent().read_into_slice::<T>(
                offset,
                window_shape,
                window_shape,
                slice,
                None,
            )?)
        }
        fn read_to_buffer(&self, bounds: &ReadBounds) -> Result<Buffer<T, 1>> {
            let mut buff = Buffer::new([bounds.size()]);
            self.read_into_slice(bounds, buff.as_mut()).map(|_| buff)
        }
        fn read_pixel(&self, offset: Coord<usize>) -> Result<T> {
            let rasterband = self.raster_band()?;
            let window_shape = (1, 1);
            let offset = offset.try_cast()?.x_y();
            let pixel_buff = &mut [T::zero()];
            info!("reading pixel at offset: {:?}", offset);
            rasterband.borrow_dependent().read_into_slice::<T>(
                offset,
                window_shape,
                window_shape,
                pixel_buff,
                None,
            )?;
            Ok(pixel_buff[0])
        }
    }
}
