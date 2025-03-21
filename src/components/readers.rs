use gdal::raster::GdalType;
use ndarray::{s, Array2, Array3};
use num::Num;

use crate::{errors::Result, tuple_to};

pub trait Reader {
    fn read_window<T: GdalType + Num + Clone + Copy>(
        &self,
        band_indexes: &[usize],
        offset: (usize, usize),
        size: (usize, usize),
    ) -> Result<Array3<T>>;
}

impl Reader for gdal::Dataset {
    fn read_window<T: GdalType + Num + Clone + Copy>(
        &self,
        band_indexes: &[usize],
        offset: (usize, usize),
        size: (usize, usize),
    ) -> Result<Array3<T>> {
        let shape = (band_indexes.len(), size.0, size.1);
        let mut array = Array3::zeros(shape);
        for band_index in band_indexes {
            let buf =
                self.rasterband(*band_index)?
                    .read_as::<T>(tuple_to(offset), size, size, None)?;
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
