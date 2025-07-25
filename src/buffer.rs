use std::{marker::PhantomData, mem::MaybeUninit};

use crate::components::DataType;

#[derive(Debug)]
pub struct Buffer<T, const ND: usize> {
    // Row-major
    data: Box<[T]>,
    shape: [usize; ND],
    _t: PhantomData<T>,
}

impl<T: DataType, const ND: usize> Buffer<MaybeUninit<T>, ND> {
    pub fn new_uninit(shape: [usize; ND]) -> Self {
        Self {
            data: Box::new_uninit_slice(shape.iter().product()),
            shape,
            _t: PhantomData,
        }
    }

    pub unsafe fn assume_init(self) -> Buffer<T, ND> {
        let data = self.data.assume_init();
        Buffer::<T, ND> {
            data: data,
            shape: self.shape,
            _t: PhantomData,
        }
    }
}

impl<T: DataType, const ND: usize> Buffer<T, ND> {
    pub fn new_zeroed(shape: [usize; ND]) -> Self {
        Self {
            data: unsafe { Box::new_zeroed_slice(shape.iter().product()).assume_init() },
            shape,
            _t: PhantomData,
        }
    }

    pub fn to_owned_parts(self) -> (Box<[T]>, [usize; ND]) {
        (self.data, self.shape)
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
