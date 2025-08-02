#[macro_use]
extern crate shrinkwraprs;
extern crate geo_booleanop;

mod ambassador_remote_traits;
mod buffer;
mod components;
mod crs_geo;
mod errors;
mod indexes;
mod intersection;

use geo::{Coord, CoordNum, Line, MapCoords};
use geo_traits::{CoordTrait, LineTrait};

pub use buffer::Buffer;
pub use components::{
    bounds::{Bounds, ViewBounds},
    engines::gdal_engine,
    raster::Raster,
    view::{SendSyncView, View},
    DataType,
};
pub use crs_geo::CrsGeometry;
use errors::{Result, RusterioError};
pub use indexes::Indexes;

trait CoordUtils: CoordTrait + Sized {
    /* fn map<NT: CoordNum>(self, func: impl Fn(Coord<Self::T>) -> Coord<NT>) -> Coord<NT>
    where Self::T: CoordNum
    {
        func(Coord::from((self.x(), self.y())))
    }

    fn affine_transform(self, transform: AffineTransform<Self::T>) -> Coord<Self::T>
    where
        Self::T: CoordNum
    {
        self.map(|coord| transform.apply(coord))
    } */

    fn map_each<NT: CoordNum>(&self, func: impl Fn(Self::T) -> NT) -> Coord<NT>
    where
        Self::T: CoordNum,
    {
        Coord {
            x: func(self.x()),
            y: func(self.y()),
        }
    }

    fn try_cast<NT: CoordNum + num::NumCast>(self) -> Result<Coord<NT>>
    where
        Self::T: num::NumCast,
    {
        Ok(Coord {
            x: try_cast(self.x())?,
            y: try_cast(self.y())?,
        })
    }

    fn operate<NT: CoordNum>(&self, rhs: &Self, func: impl Fn(Self::T, Self::T) -> NT) -> Coord<NT>
    where
        Self::T: CoordNum,
    {
        Coord {
            x: func(self.x(), rhs.x()),
            y: func(self.y(), rhs.y()),
        }
    }
}

impl<T: CoordNum> CoordUtils for Coord<T> {}

trait LineUtils<T: CoordNum, NT: CoordNum + num::NumCast>:
    LineTrait + Sized + MapCoords<T, NT>
{
    fn try_cast(self) -> Result<Self::Output>
    where
        Self::T: num::NumCast,
    {
        self.try_map_coords(Coord::try_cast)
    }
}

impl<T: CoordNum, NT: CoordNum + num::NumCast> LineUtils<T, NT> for Line<T> {}

fn try_cast<T: num::NumCast, U: num::NumCast>(val: T) -> Result<U> {
    num_traits::cast(val).ok_or(RusterioError::Uncastable)
}

fn try_tuple_cast<T: num::NumCast, U: num::NumCast>(tuple: (T, T)) -> Result<(U, U)> {
    Ok((try_cast(tuple.0)?, try_cast(tuple.1)?))
}

#[cfg(test)]
mod tests {

    use crate::components::{bounds::ViewBounds, engines::gdal_engine::GdalFile};

    use super::*;
    use log::info;
    use ndarray::Axis;
    use rstest::rstest;

    const SENTINEL2_FILE_NAME: &str =
        "S2B_MSIL2A_20241206T093309_N0511_R136_T33PTM_20241206T115919";
    const SENTINEL2_FILE_PATH: fn() -> String = || format!("data/{SENTINEL2_FILE_NAME}.SAFE.zip");
    const SENTINEL2_RESOLUTION_GROUP_PATH: fn(i32) -> String = |resolution| {
        format!("SENTINEL2_L2A:/vsizip/data/{SENTINEL2_FILE_NAME}.SAFE.zip/{SENTINEL2_FILE_NAME}.SAFE/MTD_MSIL2A.xml:{resolution}:EPSG_32633")
    };

    #[rstest]
    #[test_log::test]
    fn base_use() {
        let mut sentinel_rasters = Vec::new();
        let band_indexes = [
            (Indexes::from(([], true))),
            (Indexes::from((0usize..6, false))),
            (Indexes::from(([0usize, 1], false))),
        ];
        for (res, indexes) in [10, 20, 60].into_iter().zip(band_indexes) {
            let raster_path = SENTINEL2_RESOLUTION_GROUP_PATH(res);
            let raster = Raster::new::<GdalFile<u16>>(raster_path, indexes).unwrap();
            sentinel_rasters.push(raster);
        }

        let sentinel_raster = Raster::stack(sentinel_rasters).unwrap();
        let sentinel_view = sentinel_raster
            .view(None, Indexes::from([0, 4, 10])) // all different resolutions
            .unwrap();
        let clipped_view = sentinel_view
            .clip(ViewBounds::new((0, 0), (10, 10)))
            .unwrap();
        let buff = clipped_view.read().unwrap();
        info!(
            "data len: {:?}\ndata shape: {:?}\nmatches {:}",
            &buff.len(),
            &buff.shape(),
            &buff.shape().iter().product::<usize>() == &buff.len()
        );
        let (buff_data, _) = buff.to_owned_parts();
        let buff_vec = Vec::from(buff_data);
        info!("as vector: {:?}", buff_vec.len())
        //ndarray_npy::write_npy("dev/test.npy", &arr).unwrap()
    }

    #[rstest]
    #[test_log::test]
    fn works_with_full_sentinel2() {
        let sentinel_raster = gdal_engine::open::<u16>(SENTINEL2_FILE_PATH()).unwrap();
        info!("{:#?}", sentinel_raster);
    }

    #[rstest]
    #[test_log::test]
    fn works_with_partial_sentinel2() {
        let sentinel_raster =
            gdal_engine::open::<u16>(SENTINEL2_RESOLUTION_GROUP_PATH(10)).unwrap();
        info!("{:#?}", sentinel_raster);
    }

    #[rstest]
    #[test_log::test]
    fn convert_to_ndarray() {
        use ndarray;

        let sentinel_raster =
            gdal_engine::open::<u16>(SENTINEL2_RESOLUTION_GROUP_PATH(10)).unwrap();

        let (data, shape) = sentinel_raster
            .view(None, Indexes::all())
            .unwrap()
            .clip(ViewBounds::new((0, 0), (125, 250)))
            .unwrap()
            .read()
            .unwrap()
            .to_owned_parts();

        let arr = ndarray::Array3::from_shape_vec(shape, data.to_vec()).unwrap();
        info!("as ndarray: {:?}", arr)
    }

    #[rstest]
    #[test_log::test]
    fn as_rgb_image() {
        use image;
        //use ndarray::s;

        let sentinel_raster = gdal_engine::open::<u16>(
            // SENTINEL2_RESOLUTION_GROUP_PATH(10)
            SENTINEL2_FILE_PATH(),
        )
        .unwrap();

        let view = sentinel_raster
            .view(None, Indexes::from([0, 4, 15]))
            .unwrap()
            .clip(ViewBounds::new((0, 0), (1000, 1000)))
            .unwrap();

        let (data, shape) = view.read().unwrap().to_owned_parts();
        let shape = [shape[0], shape[2], shape[1]];
        let arr = ndarray::Array3::from_shape_vec(shape, data.to_vec()).unwrap();
        let arr_dim = arr.dim();
        info!("as ndarray: {:?}", arr_dim);
        //info!("as ndarray: {:?}", arr); //.slice(s![0.., ..10, (arr_dim.2 - 10)..]));

        let arr = arr.permuted_axes([1, 2, 0]); // rearrange axes to (W, H, C)
        let arr = arr.mapv(u32::from);
        let arr_max = arr
            .map_axis(Axis(0), |axis| *axis.iter().max().unwrap())
            .map_axis(Axis(0), |axis| *axis.iter().max().unwrap());
        let broadcasted_arr_max = arr_max.broadcast(arr.dim()).unwrap();

        let arr = ((arr * 255) / broadcasted_arr_max).mapv(|val| val as u8);
        info!("as ndarray: {:?}", arr.dim());
        let _ = image::RgbImage::from_raw(
            arr.dim().0 as u32,
            arr.dim().1 as u32,
            arr.into_iter().collect(),
        )
        .unwrap()
        .save(format!("data/{SENTINEL2_FILE_NAME}.png"))
        .unwrap();
    }
}
