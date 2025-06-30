#![allow(dead_code)]
#![feature(trait_alias)]

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
    bool_ops::BoolOpsNum,
    proj::{Proj, Transform},
    BooleanOps, BoundingRect, Coord, CoordNum, LineString, MultiPolygon, Polygon, Rect,
};
use geo_traits::{CoordTrait, GeometryTrait, GeometryType, RectTrait};

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

fn cast<T: num::NumCast, U: num::NumCast>(val: T) -> Result<U> {
    num_traits::cast(val).ok_or(RusterioError::Uncastable)
}

fn cast_tuple<T: num::NumCast, U: num::NumCast>(tuple: (T, T)) -> Result<(U, U)> {
    Ok((cast(tuple.0)?, cast(tuple.1)?))
}

fn swap<T>(tuple: (T, T)) -> (T, T) {
    (tuple.1, tuple.0)
}

pub struct Indexes {
    selection: Vec<usize>,
    drop: bool,
}

impl<const N: usize> From<([usize; N], bool)> for Indexes {
    fn from(value: ([usize; N], bool)) -> Self {
        let selection = value.0.to_vec();
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
        let selection = value.to_vec();
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
    fn indexes_from(self, collection_len: usize) -> Vec<usize> {
        let idxs = self.selection;
        if self.drop {
            let drop_idxs: HashSet<usize, RandomState> = HashSet::from_iter(idxs.into_iter());
            HashSet::from_iter(0..collection_len)
                .difference(&drop_idxs)
                .map(|idx| *idx)
                .collect()
        } else {
            idxs
        }
    }

    pub fn select_from<I: Clone + Copy>(self, collection: Vec<I>) -> Vec<I> {
        self.indexes_from(collection.len())
            .into_iter()
            .map(|idx| collection[idx])
            .collect()
    }

    pub fn all() -> Self {
        Self {
            selection: Vec::with_capacity(0),
            drop: true,
        }
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
        for (res, indexes) in [10, 20, 60].iter().zip(band_indexes) {
            let raster_path = format!("SENTINEL2_L2A:/vsizip/data/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE.zip/S2B_MSIL2A_20241126T093239_N0511_R136_T33PTM_20241126T120342.SAFE/MTD_MSIL2A.xml:{res}:EPSG_32633");
            let raster = Raster::new::<GdalFile<u16>, _>(raster_path, indexes).unwrap();
            println!("{:?}", raster);
            sentinel_rasters.push(raster);
        }

        let sentinel_raster = Raster::stack(sentinel_rasters).unwrap();
        println!("{:?}", sentinel_raster);
        let sentinel_view = sentinel_raster
            .view(None, Indexes::from([0, 1, 2]))
            .unwrap();
        println!("{:?}", sentinel_view);
        let arr = sentinel_view
            .clip(ViewBounds::new((0, 0), (1250, 1250)))
            .unwrap()
            .as_send_sync()
            .read()
            .unwrap();
        println!("{:?}", &arr);
        ndarray_npy::write_npy("dev/test.npy", &arr).unwrap()
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
