//! Alive cells, bit-packed 64 per `u64` word, one row after another.
//! Bit `i` of word `k` in a row is cell `64k + i`. The padding bits past the
//! grid's width are always zero.

#[derive(Clone)]
pub(crate) struct Grid {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) words_per_row: usize,
    pub(crate) cells: Vec<u64>,
}

impl Grid {
    pub(crate) fn new(width: usize, height: usize) -> Grid {
        let words_per_row = width.div_ceil(64);
        Grid {
            width,
            height,
            words_per_row,
            cells: vec![0; words_per_row * height],
        }
    }

    /// The bits of a row's last word that lie inside the grid.
    pub(crate) fn padding_mask(&self) -> u64 {
        match self.width % 64 {
            0 => u64::MAX,
            used => (1 << used) - 1,
        }
    }

    pub(crate) fn row(&self, y: usize) -> &[u64] {
        &self.cells[y * self.words_per_row..(y + 1) * self.words_per_row]
    }

    pub(crate) fn get(&self, x: usize, y: usize) -> bool {
        (self.cells[y * self.words_per_row + x / 64] >> (x % 64)) & 1 == 1
    }

    /// Sets one cell and returns whether its state changed.
    pub(crate) fn set(&mut self, x: usize, y: usize, alive: bool) -> bool {
        let word = &mut self.cells[y * self.words_per_row + x / 64];
        let bit = 1u64 << (x % 64);
        let was_alive = *word & bit != 0;
        if alive {
            *word |= bit;
        } else {
            *word &= !bit;
        }
        was_alive != alive
    }
}
