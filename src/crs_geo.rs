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
