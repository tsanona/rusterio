use gdal::errors::GdalError;

pub type Result<T> = std::result::Result<T, Sentinel2ArrayError>;

#[derive(thiserror::Error, Debug)]
pub enum Sentinel2ArrayError {
    #[error(transparent)]
    GdalError(#[from] GdalError),
    #[error(transparent)]
    RastersError(#[from] rasters::Error),
    /*#[error(transparent)]
    ProjError(#[from] ProjCreateError),
    #[error(transparent)]
    ShapeError(#[from] ShapeError),
    #[error("Dataset {0} contains bands with different projections.")]
    MultipleProjectionsInDataset(String), */
    #[error("Band `{0}` has a non inverteble geo transform.")]
    BandTransformNotInvertible(String),
    #[error("Band `{0}` not found.")]
    BandNotFound(String),
    #[error("Couldn't find {key} in metadata of {object_desc}.")]
    MetadataKeyNotFound { object_desc: String, key: String },
}
