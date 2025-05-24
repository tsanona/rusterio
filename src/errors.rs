pub type Result<T> = std::result::Result<T, RusterioError>;

#[derive(thiserror::Error, Debug)]
pub enum RusterioError {
    #[error(transparent)]
    ProjError(#[from] geo::proj::ProjError),
    #[error(transparent)]
    ProjCreateError(#[from] geo::proj::ProjCreateError),
    #[error(transparent)]
    GdalError(#[from] gdal::errors::GdalError),
    #[error(transparent)]
    NdarrayError(#[from] ndarray::ShapeError),
    #[error(transparent)]
    RasterizeError(#[from] geo_rasterize::RasterizeError),
    #[error("Ther is no intersection between geometries")]
    NoIntersection,
    #[error("Value could not be cast")]
    Uncastable,
}
