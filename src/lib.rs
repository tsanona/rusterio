#![allow(dead_code)]
#![feature(trait_alias)]

mod components;
mod errors;

use std::fmt::Debug;

pub use backends::gdal_backend;
pub use components::{backends, BandReader, File, PixelBounds, Raster};

extern crate geo_booleanop;
use geo::{
    bool_ops::BoolOpsNum,
    proj::{Proj, Transform},
    BooleanOps, BoundingRect, Coord, CoordNum, LineString, MultiPolygon, Polygon, Rect,
};
use geo_traits::{CoordTrait, GeometryTrait, GeometryType, RectTrait};
use itertools::Itertools;

use errors::{Result, RusterioError};

#[derive(Debug, Clone)]
pub struct CrsGeometry<G: GeometryTrait> {
    crs: String,
    geometry: G,
}

impl<G: GeometryTrait + Transform<G::T, Output = G>> CrsGeometry<G>
where
    G::T: CoordNum + Debug,
{
    fn with_crs(mut self, crs: String) -> Result<Self> {
        let proj = Proj::new_known_crs(self.crs.as_str(), crs.as_str(), None)?;
        self.crs = crs;
        self.geometry.transform(&proj)?;
        Ok(self)
    }

    fn projected_geometry(&self, crs: &str) -> Result<G> {
        let proj = Proj::new_known_crs(self.crs.as_str(), crs, None)?;
        self.geometry
            .transformed(&proj)
            .map_err(RusterioError::ProjError)
    }
}

impl<G: GeometryTrait + BoundingRect<G::T>> CrsGeometry<G>
where
    G::T: CoordNum + Debug,
{
    fn bounding_rect(&self) -> Option<CrsGeometry<Rect<G::T>>> {
        let geometry = self.geometry.bounding_rect().into()?;
        Some(CrsGeometry {
            crs: self.crs.clone(),
            geometry,
        })
    }
}

impl<'a, G: GeometryTrait + Transform<G::T, Output = G>> CrsGeometry<G>
where
    G::T: CoordNum + Debug + BoolOpsNum + 'a,
{
    fn intersection(&self, rhs: &Self) -> Result<CrsGeometry<MultiPolygon<G::T>>>
    where
        G: Clone,
    {
        let rhs_geometry = if rhs.crs.ne(&self.crs) {
            &rhs.projected_geometry(&self.crs)?
        } else {
            &rhs.geometry
        };
        let rhs_polygon: Polygon<G::T> = match rhs_geometry.as_type() {
            GeometryType::Rect(rect) => {
                let rect: Rect<G::T> = Rect::new(rect.min().x_y(), rect.max().x_y());
                let rect_coord: Vec<Coord<G::T>> =
                    rect.to_lines().into_iter().map(|line| line.start).collect();
                Polygon::new(LineString::from(rect_coord), vec![])
            }
            _ => unimplemented!(),
        };
        let lhs_polygon: Polygon<G::T> = match self.geometry.as_type() {
            GeometryType::Rect(rect) => {
                let rect: Rect<G::T> = Rect::new(rect.min().x_y(), rect.max().x_y());
                let rect_coord: Vec<Coord<G::T>> =
                    rect.to_lines().into_iter().map(|line| line.start).collect();
                Polygon::new(LineString::from(rect_coord), vec![])
            }
            _ => unimplemented!(),
        };
        Ok(CrsGeometry {
            crs: self.crs.clone(),
            geometry: lhs_polygon.intersection(&rhs_polygon),
        })
    }
}

fn cast<T: num::NumCast, U: num::NumCast>(val: T) -> Result<U> {
    num_traits::cast(val).ok_or(RusterioError::Uncastable)
}

fn cast_tuple<T: num::NumCast, U: num::NumCast>(tuple: (T, T)) -> Result<(U, U)> {
    Ok((cast(tuple.0)?, cast(tuple.1)?))
}

#[macro_use]
extern crate shrinkwraprs;

#[derive(Shrinkwrap)]
pub struct Indexes(Vec<usize>);

/* impl Indexes {
    fn select_in<I: Index<usize>>(self, indexable: I, size: usize, drop: bool) -> impl Iterator<Item = I::Output>
    where I::Output: Sized + 'static
    {
         let mut idxs = self.0.into_iter();
        if drop {
            let mut non_dropped_indxs = Vec::from_iter(0..size);
            let sorted_indexes = idxs.sorted().enumerate();
            for (shift, idx) in sorted_indexes {
                non_dropped_indxs.remove(idx - shift);
            }
            idxs = non_dropped_indxs.into_iter()
        }
        idxs.map(|idx| indexable[idx])
    }
} */

/* impl From<&'static [usize]> for Indexes {
    fn from(value: &'static [usize]) -> Self {
        Indexes(value.to_vec())
    }
} */

impl<const N: usize> From<[usize; N]> for Indexes {
    fn from(value: [usize; N]) -> Self {
        Indexes(value.to_vec())
    }
}

impl From<std::ops::Range<usize>> for Indexes {
    fn from(value: std::ops::Range<usize>) -> Self {
        Indexes(value.into_iter().collect_vec())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use components::backends::gdal_backend::GdalFile;
    use rstest::rstest;

    #[rstest]
    fn play_ground() {
        let mut sentinel_rasters = Vec::new();
        let band_indexes = [
            (Indexes::from([]), true),
            (Indexes::from(0usize..6), false),
            (Indexes::from([0usize, 1]), false),
        ];
        for (res, (indexes, drop)) in [10, 20, 60].iter().zip(band_indexes) {
            let raster_path = format!("SENTINEL2_L2A:/vsizip/data/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE.zip/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE/MTD_MSIL2A.xml:{res}:EPSG_32633");
            let file = GdalFile::open(raster_path).unwrap();
            let raster = Raster::<u16>::new(file, indexes, drop).unwrap();
            println!("{:?}", raster);
            sentinel_rasters.push(raster);
        }

        let sentinel_raster = Raster::stack(sentinel_rasters).unwrap();
        println!("{:?}", sentinel_raster);
        let sentinel_view = sentinel_raster
            .view(None, Indexes::from([0, 1, 2]), false)
            .unwrap();
        println!("{:?}", sentinel_view);
        let arr = sentinel_view
            .clip(PixelBounds::new((0, 0), (1250, 1250)))
            .unwrap()
            .read()
            .unwrap();
        println!("{:?}", &arr);
        ndarray_npy::write_npy("dev/test.npy", &arr.t()).unwrap()
    }
}
