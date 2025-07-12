#![feature(new_zeroed_alloc)]

#[macro_use]
extern crate shrinkwraprs;

mod components;
mod errors;

use std::{collections::HashSet, fmt::Debug, hash::RandomState, rc::Rc};

pub use components::{
    bounds::ViewBounds,
    engines,
    raster::Raster,
    view::{SendSyncView, View},
    DataType,
};
pub use engines::gdal_engine;

extern crate geo_booleanop;
use geo::{
    bool_ops::BoolOpsNum, BooleanOps, BoundingRect, Coord, CoordNum, LineString, MultiPolygon,
    Polygon, Rect,
};
use geo_traits::{CoordTrait, GeometryTrait, GeometryType, RectTrait};
use itertools::Itertools;
use proj::{Proj, Transform};

use errors::{Result, RusterioError};

#[derive(Shrinkwrap, Debug, Clone)]
pub struct CrsGeometry<G: GeometryTrait> {
    crs: Rc<str>,
    #[shrinkwrap(main_field)]
    geometry: G,
}

impl<G: GeometryTrait + Transform<G::T, Output = G>> CrsGeometry<G>
where
    G::T: CoordNum + Debug,
{
    fn with_crs(mut self, crs: &str) -> Result<Self> {
        let proj = Proj::new_known_crs(self.crs.as_ref(), crs, None)?;
        self.crs = Rc::from(crs);
        self.geometry.transform(&proj)?;
        Ok(self)
    }

    fn projected_geometry(&self, crs: &str) -> Result<G> {
        let proj = Proj::new_known_crs(self.crs.as_ref(), crs, None)?;
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
            crs: Rc::clone(&self.crs),
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
            crs: Rc::clone(&self.crs),
            geometry: lhs_polygon.intersection(&rhs_polygon),
        })
    }
}

fn try_cast<T: num::NumCast, U: num::NumCast>(val: T) -> Result<U> {
    num_traits::cast(val).ok_or(RusterioError::Uncastable)
}

fn try_tuple_cast<T: num::NumCast, U: num::NumCast>(tuple: (T, T)) -> Result<(U, U)> {
    Ok((try_cast(tuple.0)?, try_cast(tuple.1)?))
}

fn try_coord_cast<T: CoordNum, U: CoordNum>(coord: Coord<T>) -> Result<Coord<U>> {
    Ok(Coord::from(try_tuple_cast(coord.x_y())?))
}

pub struct Indexes {
    selection: Box<[usize]>,
    drop: bool,
}

impl<const N: usize> From<([usize; N], bool)> for Indexes {
    fn from(value: ([usize; N], bool)) -> Self {
        let selection = Box::from(value.0);
        let drop = value.1;
        Indexes { selection, drop }
    }
}

impl From<(std::ops::Range<usize>, bool)> for Indexes {
    fn from(value: (std::ops::Range<usize>, bool)) -> Self {
        let selection = value.0.collect();
        let drop = value.1;
        Indexes { selection, drop }
    }
}

impl<const N: usize> From<[usize; N]> for Indexes {
    fn from(value: [usize; N]) -> Self {
        let selection = Box::from(value);
        Indexes {
            selection,
            drop: false,
        }
    }
}

impl From<std::ops::Range<usize>> for Indexes {
    fn from(value: std::ops::Range<usize>) -> Self {
        let selection = value.collect();
        Indexes {
            selection,
            drop: false,
        }
    }
}

impl Indexes {
    fn indexes_from(self, collection_len: usize) -> Box<[usize]> {
        let idxs = self.selection;
        if self.drop {
            let drop_idxs: HashSet<usize, RandomState> = HashSet::from_iter(idxs);
            HashSet::from_iter(0..collection_len)
                .difference(&drop_idxs)
                .sorted()
                .map(|idx| *idx)
                .collect()
        } else {
            idxs
        }
    }

    pub fn select_from<T: Clone + Copy>(self, collection: Vec<T>) -> Box<[T]> {
        self.indexes_from(collection.len())
            .iter()
            .map(|idx| collection[*idx])
            .collect()
    }

    pub fn all() -> Self {
        Self {
            selection: Box::from([]),
            drop: true,
        }
    }
}

#[derive(Debug)]
pub struct Buffer<T: DataType, const D: usize> {
    // Row-major ordered
    data: Box<[T]>,
    shape: [usize; D],
}

impl<T: DataType, const D: usize> Buffer<T, D> {
    pub fn new(shape: [usize; D]) -> Self {
        let data = unsafe { Box::new_zeroed_slice(shape.iter().product()).assume_init() };
        Buffer { data, shape }
    }

    pub fn as_mut_data(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn to_owned_parts(self) -> (Box<[T]>, [usize; D]) {
        (self.data, self.shape)
    }

    pub fn shape(&self) -> [usize; D] {
        self.shape
    }
}

#[cfg(test)]
mod tests {

    use crate::components::{bounds::ViewBounds, engines::gdal_engine::GdalFile};

    use super::*;
    use rstest::rstest;

    #[rstest]
    fn base_use() {
        let mut sentinel_rasters = Vec::new();
        let band_indexes = [
            (Indexes::from(([], true))),
            (Indexes::from((0usize..6, false))),
            (Indexes::from(([0usize, 1], false))),
        ];
        for (res, indexes) in [10, 20, 60].into_iter().zip(band_indexes) {
            let raster_path = format!("SENTINEL2_L2A:/vsizip/data/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE.zip/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE/MTD_MSIL2A.xml:{res}:EPSG_32633");
            let raster = Raster::new::<GdalFile<u16>, _>(raster_path, indexes).unwrap();
            println!("{:?}", raster);
            sentinel_rasters.push(raster);
        }

        let sentinel_raster = Raster::stack(sentinel_rasters).unwrap();
        println!("{:?}", sentinel_raster);
        let sentinel_view = sentinel_raster
            .view(None, Indexes::from([0, 4, 10]))
            .unwrap();
        println!("{:?}", sentinel_view);
        let clipped_view = sentinel_view
            .clip(ViewBounds::new((0, 0), (1250, 1250)))
            .unwrap();
        println!("{:?}", clipped_view);
        let buff = clipped_view.read().unwrap();
        println!("{:?}", &buff.data);
        //ndarray_npy::write_npy("dev/test.npy", &arr).unwrap()
    }

    #[rstest]
    fn works_with_full_sentinel2() {
        let sentinel_raster = gdal_engine::open::<u16, _>(
            "data/S2B_MSIL2A_20241206T093309_N0511_R136_T33PTM_20241206T115919.SAFE.zip",
        )
        .unwrap();
        println!("{:#?}", sentinel_raster);
    }

    #[rstest]
    fn works_with_partial_sentinel2() {
        let sentinel_raster = gdal_engine::open::<u16, _>(
           "SENTINEL2_L2A:/vsizip/data/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE.zip/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE/MTD_MSIL2A.xml:10:EPSG_32633",
        )
        .unwrap();
        println!("{:#?}", sentinel_raster);
    }
}
