use std::rc::Rc;

use geo::{bool_ops::BoolOpsNum, BoundingRect, CoordNum, Rect};
use geo_traits::{GeometryTrait, RectTrait};
use proj::{Proj, Transform};

use crate::{
    ambassador_remote_traits::{ambassador_impl_GeometryTrait, ambassador_impl_RectTrait},
    errors::Result,
    intersection::Intersection,
};

#[derive(thiserror::Error, Debug)]
pub enum CrsGeometryError {
    #[error(transparent)]
    ProjError(#[from] proj::ProjError),
    #[error(transparent)]
    ProjCreateError(#[from] proj::ProjCreateError),
}

#[derive(ambassador::Delegate, Shrinkwrap, Debug, Clone)]
#[delegate(GeometryTrait, target = "geometry")]
#[delegate(RectTrait, target = "geometry", where = "G: RectTrait")]
pub struct CrsGeometry<G: GeometryTrait> {
    crs: Rc<Box<str>>,
    #[shrinkwrap(main_field)]
    geometry: G,
}

impl<G: GeometryTrait> CrsGeometry<G> {
    pub fn new(crs: Rc<Box<str>>, geometry: G) -> Self {
        Self { crs, geometry }
    }

    pub fn crs(&self) -> &str {
        self.crs.as_ref()
    }
}

impl<G: GeometryTrait + Transform<G::T, Output = G> + Clone> CrsGeometry<G>
where
    G::T: CoordNum,
{
    pub fn with_crs(mut self, crs: &str) -> std::result::Result<Self, CrsGeometryError> {
        if self.crs().ne(crs) {
            let proj = Proj::new_known_crs(self.crs(), crs, None)?;
            self.crs = Rc::new(Box::from(crs));
            self.geometry.transform(&proj)?;
        }
        Ok(self)
    }

    /// Clones if crs is same.
    pub fn projected_geometry(&self, crs: &str) -> std::result::Result<G, CrsGeometryError> {
        if self.crs().ne(crs) {
            let proj = Proj::new_known_crs(self.crs(), crs, None)?;
            Ok(self.geometry.transformed(&proj)?)
        } else {
            Ok(self.geometry.clone())
        }
    }
}

impl<G: GeometryTrait + BoundingRect<G::T>> CrsGeometry<G>
where
    G::T: CoordNum,
{
    pub fn bounding_rect(&self) -> Option<CrsGeometry<Rect<G::T>>> {
        let geometry = self.geometry.bounding_rect().into()?;
        Some(CrsGeometry {
            crs: Rc::clone(&self.crs),
            geometry,
        })
    }
}

/* impl<G: GeometryTrait + Intersection<Output = MultiPolygon<G::T>>> Intersection for CrsGeometry<G>
where G::T: BoolOpsNum
{
    type Output = CrsGeometry<G::Output>;
    fn intersection(&self, rhs: &Self) -> Result<CrsGeometry<MultiPolygon<G::T>>> {
        let geometry  = self.geometry.intersection(&rhs.geometry)?;
        Ok(CrsGeometry::new(Rc::clone(&self.crs), geometry))
    }
} */

impl<G: GeometryTrait + Intersection> Intersection for CrsGeometry<G>
where
    G::T: BoolOpsNum,
{
    type Output = CrsGeometry<G::Output>;
    fn intersection(&self, rhs: &Self) -> Result<Self::Output> {
        let geometry = self.geometry.intersection(&rhs.geometry)?;
        Ok(CrsGeometry::new(Rc::clone(&self.crs), geometry))
    }
}

/* impl<T: CoordNum + Ord> Intersection for CrsGeometry<Rect<T>> {
    type Output = Option<Self>;
    fn intersection(&self, rhs: &Self) -> Self::Output {
        let geometry = self.geometry.intersection(&rhs.geometry)?;
        Some(CrsGeometry::new(Rc::clone(&self.crs), geometry))
    }
} */

/* impl<G, T: CoordNum + Ord + BoolOpsNum> Intersection for CrsGeometry<G>
where G: BooleanOps
{
    type Output = Option<MultiP>;
    fn intersection(&self, rhs: &Self) -> Self::Output {
        let geometry = self.geometry.intersection(&rhs.geometry)?;
        Some(CrsGeometry::new(Rc::clone(&self.crs), geometry))
    }
} */

/* impl<G> CrsGeometry<G>
where
    G: GeometryTrait + Transform<G::T, Output = G> + BooleanOps,
    G::T: CoordNum
{
    pub fn intersection(&self, rhs: &Self) -> Result<CrsGeometry<MultiPolygon<G::Scalar>>>
    where
        G: Clone,
    {
        let rhs_geometry = rhs.projected_geometry(&self.crs)?;
        let geometry = self.geometry.intersection(&rhs_geometry);
        Ok(CrsGeometry {
            crs: Rc::clone(&self.crs),
            geometry,
        })
    }
} */

/* impl<'a, G> CrsGeometry<G>
where
    G: GeometryTrait + Transform<G::T, Output = G> + ToGeoRect<G::T>,
    G::T: CoordNum + Debug + BoolOpsNum + 'a,
{
    fn intersection<H: GeometryTrait<T=G::T> + ToGeoRect<H::T> + Transform<H::T, Output = H>>(&self, rhs: &CrsGeometry<H>) -> Result<CrsGeometry<Rect<G::T>>>
    where
        G: Clone,
    {
        let rhs_geometry = if rhs.crs.ne(&self.crs) {
            &rhs.projected_geometry(&self.crs)?
        } else {
            &rhs.geometry
        };
        let rhs_polygon = Polygon::from(rhs_geometry.to_rect());
        let lhs_polygon = Polygon::from(self.geometry.to_rect());
        let geometry = lhs_polygon.intersection(&rhs_polygon).bounding_rect().ok_or(RusterioError::NoIntersection)?;
        Ok(CrsGeometry {
            crs: Rc::clone(&self.crs),
            geometry,
        })
    }
} */
