use std::{collections::HashSet, hash::RandomState, rc::Rc};

use itertools::Itertools;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Indexes {
    selection: Rc<[usize]>,
    drop: bool,
}

impl<const N: usize> From<([usize; N], bool)> for Indexes {
    fn from(value: ([usize; N], bool)) -> Self {
        let selection = Rc::from(value.0);
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
        let selection = Rc::from(value);
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
    pub fn indexes_from(self, collection_len: usize) -> Rc<[usize]> {
        let idxs = self.selection;
        if self.drop {
            let drop_idxs: HashSet<usize, RandomState> = HashSet::from_iter(Box::<[usize]>::from(idxs.as_ref()));
            Rc::from_iter(HashSet::from_iter(0..collection_len)
                .difference(&drop_idxs)
                .sorted()
                .map(|idx| *idx))
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
            selection: Rc::from([]),
            drop: true,
        }
    }
}
