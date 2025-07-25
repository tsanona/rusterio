use geo::{bool_ops::BoolOpsNum, BooleanOps, CoordNum, MultiPolygon, Polygon, Rect};
use geo_traits::GeometryTrait;

use crate::{
    errors::{Result, RusterioError},
    CoordUtils,
};

#[derive(thiserror::Error, Debug)]
pub enum IntersectionError {
    #[error("Ther is no intersection between geometries")]
    NoIntersection,
}

pub trait Intersection {
    type Output: GeometryTrait;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output>;
}

impl<T: CoordNum> Intersection for Rect<T> {
    type Output = Rect<T>;
    fn intersection(&self, rhs: &Self) -> Result<Rect<T>> {
        let lhs_max = self.max();
        let rhs_min = rhs.min();
        if (lhs_max.x < rhs_min.x) | (lhs_max.y < rhs_min.y) {
            return Err(IntersectionError::NoIntersection).map_err(RusterioError::NoIntersection);
        }

        let lhs_min = self.min();
        let rhs_max = rhs.max();
        if (lhs_min.x > rhs_max.x) | (lhs_min.y > rhs_max.y) {
            return Err(IntersectionError::NoIntersection).map_err(RusterioError::NoIntersection);
        }

        let min = lhs_min.operate(&rhs_min, |x, y| if x > y { x } else { y });
        let max = lhs_max.operate(&rhs_max, |x, y| if x < y { x } else { y });

        Ok(Self::new(min, max))
    }
}

impl<T: CoordNum + BoolOpsNum + Ord> Intersection for Polygon<T> {
    type Output = MultiPolygon<T>;
    fn intersection(&self, rhs: &Self) -> Result<MultiPolygon<T>> {
        Ok(<Self as BooleanOps>::intersection(&self, rhs))
    }
}
