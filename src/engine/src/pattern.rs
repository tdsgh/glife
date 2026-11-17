//! A rectangle of alive cells, bit-packed 32 per `u32` word, one row after
//! another, in the pattern-buffer row order. The padding bits past a row's
//! width are always zero.

use crate::{Axis, MAX_SIDE};

/// A width by height rectangle of cells, 1 to 2048 per side. It may hold no
/// live cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    width: u32,
    height: u32,
    words_per_row: usize,
    cells: Vec<u32>,
}

impl Pattern {
    /// An empty pattern.
    ///
    /// # Panics
    ///
    /// If a side is outside `1..=MAX_SIDE`.
    pub fn new(width: u32, height: u32) -> Pattern {
        let sides = 1..=MAX_SIDE;
        assert!(
            sides.contains(&width) && sides.contains(&height),
            "pattern size {width} x {height} is outside 1..={MAX_SIDE} per side"
        );
        let words_per_row = (width as usize).div_ceil(32);
        Pattern {
            width,
            height,
            words_per_row,
            cells: vec![0; words_per_row * height as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Whether a cell is alive. Positions outside the pattern are dead.
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.cells[self.word(x, y)] >> (x % 32) & 1 == 1
    }

    /// Sets one cell. Positions outside the pattern are ignored.
    pub fn set(&mut self, x: u32, y: u32, alive: bool) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = self.word(x, y);
        if alive {
            self.cells[i] |= 1 << (x % 32);
        } else {
            self.cells[i] &= !(1 << (x % 32));
        }
    }

    pub fn live_count(&self) -> u32 {
        self.cells.iter().map(|w| w.count_ones()).sum()
    }

    /// The pattern mirrored left to right (`Horizontal`) or top to bottom
    /// (`Vertical`).
    pub fn flipped(&self, axis: Axis) -> Pattern {
        let mut out = Pattern::new(self.width, self.height);
        for (x, y) in self.live_cells() {
            match axis {
                Axis::Horizontal => out.set(self.width - 1 - x, y, true),
                Axis::Vertical => out.set(x, self.height - 1 - y, true),
            }
        }
        out
    }

    /// The pattern scaled down to fit `max_side` per side, keeping the aspect
    /// ratio: each cell of the result is alive when any cell it covers is. A
    /// pattern that already fits is returned unchanged.
    ///
    /// # Panics
    ///
    /// If `max_side` is 0.
    pub fn thumbnail(&self, max_side: u32) -> Pattern {
        assert!(max_side > 0, "thumbnail needs a positive side");
        let longest = self.width.max(self.height);
        if longest <= max_side {
            return self.clone();
        }
        let scaled = |side: u32| (side as u64 * max_side as u64).div_ceil(longest as u64) as u32;
        let (tw, th) = (scaled(self.width), scaled(self.height));
        let mut out = Pattern::new(tw, th);
        for (x, y) in self.live_cells() {
            let tx = (x as u64 * tw as u64 / self.width as u64) as u32;
            let ty = (y as u64 * th as u64 / self.height as u64) as u32;
            out.set(tx, ty, true);
        }
        out
    }

    /// Appends the pattern buffer to `out`: `[width, height]`, then `height`
    /// rows of `ceil(width / 32)` words, bit `i` of word `k` in row `r` being
    /// cell `(32k + i, r)`.
    pub fn to_words(&self, out: &mut Vec<u32>) {
        out.reserve(2 + self.cells.len());
        out.push(self.width);
        out.push(self.height);
        out.extend_from_slice(&self.cells);
    }

    fn word(&self, x: u32, y: u32) -> usize {
        y as usize * self.words_per_row + x as usize / 32
    }

    /// The live cells, row by row.
    fn live_cells(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.cells.iter().enumerate().flat_map(move |(i, &word)| {
            let (y, k) = (i / self.words_per_row, i % self.words_per_row);
            (0..32)
                .filter(move |b| word >> b & 1 == 1)
                .map(move |b| (32 * k as u32 + b, y as u32))
        })
    }
}
