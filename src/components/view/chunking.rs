use crate::{
    components::bounds::{Bounds, ReadBounds, ViewBounds},
    errors::Result,
    CoordUtils, DataType,
};
use geo::{Coord, MapCoords};
use num::Zero;
use std::ops::Rem;

pub struct ResolutionChunker {
    ratio: Coord<usize>,
    left_block_width: usize,
    top_block_height: usize,
    view_width: usize,
    read_shape: Coord<usize>,
}

impl ResolutionChunker {
    pub fn new(view_bounds: &ViewBounds, read_bounds: &ReadBounds) -> Self {
        let ratio = view_bounds
            .shape()
            .operate(&read_bounds.shape(), usize::div_ceil); //read_band.transform.ratio();

        let relative_bounds = view_bounds.map_coords(|coord| coord.operate(&ratio, usize::rem));
        let relative_top_height = relative_bounds.max().y;
        let top_block_height = if relative_top_height.is_zero() {
            ratio.y
        } else {
            relative_top_height
        };
        let left_block_width = ratio.x - relative_bounds.min().x;

        let view_width = view_bounds.width();
        let read_shape = read_bounds.shape();
        Self {
            ratio,
            left_block_width,
            top_block_height,
            view_width,
            read_shape,
        }
    }

    pub fn read_resolution_chucked<T: DataType>(
        self,
        read_buff: &[T],
        band_buff: &mut [T],
    ) -> Result<()> {
        for row_idx in 0..self.read_shape.y {
            let block_height = self.read_row_idx_to_block_height(row_idx);
            let row_start =
                (row_idx * self.ratio.y + self.top_block_height - block_height) * self.view_width;
            let read_slice = read_buff.as_ref();
            for col_idx in 0..self.read_shape.x {
                let block_width = self.read_col_idx_to_block_width(col_idx);
                let col_start = col_idx * self.ratio.x + self.left_block_width - block_width;
                let band_write_range = row_start + col_start..row_start + col_start + block_width;
                band_buff[band_write_range].fill(read_slice[self.read_shape.x * row_idx + col_idx]);
            }

            let length = self.view_width * block_height;
            band_buff[row_start..row_start + length]
                .chunks_exact_mut(self.view_width)
                .into_iter()
                .reduce(|lhc, mut _rhc| {
                    _rhc.copy_from_slice(lhc);
                    _rhc
                });
        }
        Ok(())
    }

    fn read_row_idx_to_block_height(&self, row_idx: usize) -> usize {
        if row_idx.is_zero() {
            self.top_block_height
        } else {
            self.ratio.y
        }
    }

    fn read_col_idx_to_block_width(&self, col_idx: usize) -> usize {
        if col_idx.is_zero() {
            self.left_block_width
        } else {
            self.ratio.x
        }
    }
}
