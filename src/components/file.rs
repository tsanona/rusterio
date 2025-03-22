use geo::AffineTransform;
use std::{collections::HashMap, fmt::Debug, path::Path};

use crate::{
    components::{Band, Reader},
    errors::Result,
};

pub trait File: Debug + Sized {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    fn description(&self) -> Result<String>;
    fn size(&self) -> (usize, usize);
    fn crs(&self) -> String;
    fn transform(&self) -> Result<AffineTransform>;
    fn bands(&self) -> Result<Vec<Band>>;
    fn metadata(&self) -> HashMap<String, String>;
    fn reader(&self) -> impl Reader;
}
