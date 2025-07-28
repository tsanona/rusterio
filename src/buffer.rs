use std::marker::PhantomData;

use crate::components::DataType;

#[derive(Debug)]
pub struct Buffer<T, const ND: usize> {
    // Row-major
    data: Vec<T>,
    shape: [usize; ND],
    _t: PhantomData<T>,
}

impl<T: DataType, const ND: usize> Buffer<T, ND> {
    pub fn new(shape: [usize; ND]) -> Self {
        let data_len = shape.into_iter().product();
        let mut data = Vec::with_capacity(data_len);
        data.resize(data_len, T::zero());
        Self {
            data,
            shape,
            _t: PhantomData,
        }
    }

    pub fn to_owned_parts(self) -> (Box<[T]>, [usize; ND]) {
        (self.data.into_boxed_slice(), self.shape)
    }
}

impl<T, const ND: usize> Buffer<T, ND> {
    pub fn as_mut(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn shape(&self) -> [usize; ND] {
        self.shape
    }
}
