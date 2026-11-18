//! A rectangle of alive cells, bit-packed 32 per `u32` word, one row after
//! another, in the pattern-buffer row order. The padding bits past a row's
//! width are always zero.

use std::borrow::Cow;

use crate::{Axis, Tiling, MAX_SIDE};

/// A width by height rectangle of cells on one tiling, 1 to 2048 per side,
/// except that flipping or realigning a hexagonal pattern makes it one column
/// wider. It may hold no live cell.
///
/// The offset says how the pattern lines up with its grid: on the hexagonal
/// grid, whether row 0 is a shifted row; on the triangular grids, whether
/// cell `(0, 0)` points down. A square pattern's offset is always `false`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    width: u32,
    height: u32,
    tiling: Tiling,
    offset: bool,
    words_per_row: usize,
    cells: Vec<u32>,
}

impl Pattern {
    /// An empty pattern.
    ///
    /// # Panics
    ///
    /// If a side is outside `1..=MAX_SIDE`, or if `offset` is set on the
    /// square grid.
    pub fn new(width: u32, height: u32, tiling: Tiling, offset: bool) -> Pattern {
        let sides = 1..=MAX_SIDE;
        assert!(
            sides.contains(&width) && sides.contains(&height),
            "pattern size {width} x {height} is outside 1..={MAX_SIDE} per side"
        );
        Pattern::blank(width, height, tiling, offset)
    }

    /// An empty pattern of any positive size.
    fn blank(width: u32, height: u32, tiling: Tiling, offset: bool) -> Pattern {
        assert!(
            !(tiling == Tiling::Square && offset),
            "a square pattern has offset 0"
        );
        let words_per_row = (width as usize).div_ceil(32);
        Pattern {
            width,
            height,
            tiling,
            offset,
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

    pub fn tiling(&self) -> Tiling {
        self.tiling
    }

    pub fn offset(&self) -> bool {
        self.offset
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

    /// The true mirror image left to right (`Horizontal`) or top to bottom
    /// (`Vertical`). A hexagonal pattern flipped left to right is one column
    /// wider; the offset changes where the mirror needs it.
    pub fn flipped(&self, axis: Axis) -> Pattern {
        let flip = Flip {
            tiling: self.tiling,
            axis,
            offset: self.offset,
            width: self.width,
            height: self.height,
        };
        let (width, height, offset) = flip.shape();
        let mut out = Pattern::blank(width, height, self.tiling, offset);
        for (x, y) in self.live_cells() {
            let (x, y) = flip.cell(x, y);
            out.set(x, y, true);
        }
        out
    }

    /// The same shape with `offset`: on the hexagonal grid, the rows that
    /// were shifted move one cell right; on the triangular grids, an empty
    /// column is added on the left. Either way the pattern is one column
    /// wider. A pattern that already has `offset` is returned unchanged.
    ///
    /// # Panics
    ///
    /// If `offset` is set on a square pattern.
    pub fn aligned(&self, offset: bool) -> Pattern {
        if offset == self.offset {
            return self.clone();
        }
        let mut out = Pattern::blank(self.width + 1, self.height, self.tiling, offset);
        for (x, y) in self.live_cells() {
            out.set(realigned_x(self.tiling, self.offset, x, y), y, true);
        }
        out
    }

    /// The pattern as placing it with its center cell `(⌊w/2⌋, ⌊h/2⌋)` at
    /// `(anchor_x, anchor_y)` stamps it, and the grid cell its top-left cell
    /// lands on. See `Universe::place`.
    pub fn placement(&self, anchor_x: i64, anchor_y: i64) -> (Pattern, i64, i64) {
        let (pattern, x, y) = self.stamped_at(
            anchor_x - (self.width / 2) as i64,
            anchor_y - (self.height / 2) as i64,
        );
        (pattern.into_owned(), x, y)
    }

    /// The pattern lined up with the grid at `(x, y)`, and where its top-left
    /// cell lands. Where the offset does not match the grid there, a
    /// hexagonal pattern is realigned in place and a triangular one moves one
    /// cell right.
    pub(crate) fn stamped_at(&self, x: i64, y: i64) -> (Cow<'_, Pattern>, i64, i64) {
        let offset = grid_offset(self.tiling, x, y);
        match self.tiling {
            _ if offset == self.offset => (Cow::Borrowed(self), x, y),
            Tiling::Hexagonal => (Cow::Owned(self.aligned(offset)), x, y),
            _ => (Cow::Borrowed(self), x + 1, y),
        }
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
        let mut out = Pattern::blank(tw, th, self.tiling, self.offset);
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
    pub(crate) fn live_cells(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.cells.iter().enumerate().flat_map(move |(i, &word)| {
            let (y, k) = (i / self.words_per_row, i % self.words_per_row);
            (0..32)
                .filter(move |b| word >> b & 1 == 1)
                .map(move |b| (32 * k as u32 + b, y as u32))
        })
    }
}

/// The offset a pattern needs to line up with `tiling` with its top-left cell
/// at `(x, y)`.
pub(crate) fn grid_offset(tiling: Tiling, x: i64, y: i64) -> bool {
    match tiling {
        Tiling::Square => false,
        Tiling::Hexagonal => y.rem_euclid(2) == 1,
        Tiling::Triangular12 | Tiling::Triangular3 => (x + y).rem_euclid(2) == 1,
    }
}

/// Where `Pattern::aligned` moves cell `(x, y)` of a pattern whose offset
/// was `from`.
pub(crate) fn realigned_x(tiling: Tiling, from: bool, x: u32, y: u32) -> u32 {
    match tiling {
        Tiling::Hexagonal => x + shifted(from, y) as u32,
        _ => x + 1,
    }
}

/// Whether row `y` of a hexagonal pattern with `offset` is a shifted row.
fn shifted(offset: bool, y: u32) -> bool {
    (y % 2 == 1) != offset
}

/// The mirror `Pattern::flipped` applies to a `width` by `height` pattern
/// with `offset`.
pub(crate) struct Flip {
    pub(crate) tiling: Tiling,
    pub(crate) axis: Axis,
    pub(crate) offset: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Flip {
    /// The flipped pattern's width, height and offset.
    pub(crate) fn shape(&self) -> (u32, u32, bool) {
        let (w, h, offset) = (self.width, self.height, self.offset);
        match (self.tiling, self.axis) {
            (Tiling::Square, _) => (w, h, false),
            (Tiling::Hexagonal, Axis::Horizontal) => (w + 1, h, offset),
            (Tiling::Hexagonal, Axis::Vertical) => (w, h, offset ^ (h % 2 == 0)),
            (_, Axis::Horizontal) => (w, h, offset ^ (w % 2 == 0)),
            (_, Axis::Vertical) => (w, h, offset ^ (h % 2 == 1)),
        }
    }

    /// Where cell `(x, y)` lands in the flipped pattern.
    pub(crate) fn cell(&self, x: u32, y: u32) -> (u32, u32) {
        let (w, h) = (self.width, self.height);
        match (self.tiling, self.axis) {
            (Tiling::Hexagonal, Axis::Horizontal) if !shifted(self.offset, y) => (w - x, y),
            (_, Axis::Horizontal) => (w - 1 - x, y),
            (_, Axis::Vertical) => (x, h - 1 - y),
        }
    }
}
