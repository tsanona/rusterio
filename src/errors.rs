pub type Result<T> = std::result::Result<T, RusterioError>;

#[derive(thiserror::Error, Debug)]
pub enum RusterioError {
    /// lib errors
    #[error(transparent)]
    GdalError(#[from] gdal::errors::GdalError),
    #[error(transparent)]
    /// crate mod errors
    CrsGeometryError(#[from] crate::crs_geo::CrsGeometryError),
    #[error(transparent)]
    NoIntersection(#[from] crate::intersection::IntersectionError),
    #[error(transparent)]
    GdalEngineError(#[from] crate::components::engines::gdal_engine::GdalEngineError),
    /// crate lib errors
    #[error("Value could not be cast")]
    Uncastable,
    #[error("Coundn't find area of use in file")]
    NoAreaOfUse,
}
