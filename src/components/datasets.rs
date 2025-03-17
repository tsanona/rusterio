use std::collections::HashMap;

use geo::AffineTransform;

use crate::{components::Band, errors::Result};

pub trait Dataset: {
    fn description(&self) -> Result<String>;
    fn size(&self) -> (usize, usize);
    fn crs(&self) -> String;
    fn transform(&self) -> Result<AffineTransform>;
    fn bands(&self) -> Result<Vec<Band>>;
    fn metadata(&self) -> HashMap<String, String>;
    //fn reader(self) -> impl Reader;
}
