use std::fmt::Debug;

use crate::components::DataType;

#[derive(Debug)]
pub struct Buffer<T: DataType, const D: usize> {
    // Row-major ordered
    data: Box<[T]>,
    shape: [usize; D],
}

impl<T: DataType, const D: usize> Buffer<T, D> {
    pub fn new(shape: [usize; D]) -> Self {
        let data = unsafe { Box::new_zeroed_slice(shape.iter().product()).assume_init() };
        Buffer { data, shape }
    }

    pub fn as_mut_data(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn to_owned_parts(self) -> (Box<[T]>, [usize; D]) {
        (self.data, self.shape)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn shape(&self) -> [usize; D] {
        self.shape
    }
}
