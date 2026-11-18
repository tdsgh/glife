//! Rectangles of cells: extracting, placing, killing, flipping, and the
//! compact captures undo is built from.

use std::fmt;

use crate::{Edges, Pattern, SizeError, Universe};

/// A rectangle of grid cells: `w` by `h` with its top-left cell at `(x, y)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    /// The pattern's live cells are added; cells already alive keep their ages.
    Or,
    /// The pattern's whole rectangle replaces what lies beneath it.
    Copy,
}

/// A pattern wider or taller than the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceError {
    pub pattern: (u32, u32),
    pub grid: (u32, u32),
}

impl fmt::Display for PlaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pattern of {} x {} does not fit a grid of {} x {}",
            self.pattern.0, self.pattern.1, self.grid.0, self.grid.1
        )
    }
}

impl std::error::Error for PlaceError {}

/// The parts handed to `Universe::from_parts` do not describe one universe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartsError {
    Size(SizeError),
    /// The pattern's size is not the universe's.
    PatternSize,
    /// There is not one age per cell.
    AgesLength,
}

impl fmt::Display for PartsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartsError::Size(e) => e.fmt(f),
            PartsError::PatternSize => f.write_str("pattern size differs from the universe's"),
            PartsError::AgesLength => f.write_str("ages do not hold one entry per cell"),
        }
    }
}

impl std::error::Error for PartsError {}

/// One cell's alive state and age, from `Universe::capture_cell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellState {
    x: u32,
    y: u32,
    alive: bool,
    age: u16,
}

/// The cells of some rectangles: the alive bits packed row-major, and the
/// ages of the live cells only, run-length encoded.
#[derive(Debug, Clone)]
pub struct Capture {
    parts: Vec<CapturePart>,
}

#[derive(Debug, Clone)]
struct CapturePart {
    rect: Rect,
    /// The rectangle's cells row-major, one bit each, 64 per word.
    bits: Vec<u64>,
    /// `(age, count)` runs over the live cells in the same order.
    ages: Vec<(u16, u32)>,
}

impl Capture {
    /// Memory the capture holds, in bytes.
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Capture>()
            + self.parts.capacity() * std::mem::size_of::<CapturePart>()
            + self
                .parts
                .iter()
                .map(|p| {
                    p.bits.capacity() * std::mem::size_of::<u64>()
                        + p.ages.capacity() * std::mem::size_of::<(u16, u32)>()
                })
                .sum::<usize>()
    }
}

impl Universe {
    /// The cells of `rect` as a pattern.
    ///
    /// # Panics
    ///
    /// If `rect` is empty or reaches outside the grid.
    pub fn extract(&self, rect: Rect) -> Pattern {
        self.check_rect(rect);
        let mut out = Pattern::new(rect.w, rect.h);
        for dy in 0..rect.h {
            for dx in 0..rect.w {
                if self.get(rect.x + dx, rect.y + dy) {
                    out.set(dx, dy, true);
                }
            }
        }
        out
    }

    /// The grid rectangles a `w` by `h` placement with its top-left cell at
    /// `(x, y)` touches: up to four on a wrapping grid, and the clipped one
    /// (none when it lies wholly outside) on a dead-edged grid.
    pub fn footprint(&self, w: u32, h: u32, x: i64, y: i64) -> Vec<Rect> {
        self.segments(w, h, x, y).into_iter().map(|(r, _, _)| r).collect()
    }

    /// Stamps `pattern` with its top-left cell at `(x, y)`, which may lie
    /// outside the grid. Cells past an edge wrap to the opposite side on a
    /// wrapping grid and are dropped on a dead-edged one. A cell set alive
    /// from dead gets age 0; one killed leaves no trail.
    pub fn place(
        &mut self,
        pattern: &Pattern,
        x: i64,
        y: i64,
        mode: PasteMode,
    ) -> Result<(), PlaceError> {
        if pattern.width() > self.width() || pattern.height() > self.height() {
            return Err(PlaceError {
                pattern: (pattern.width(), pattern.height()),
                grid: (self.width(), self.height()),
            });
        }
        for (rect, px, py) in self.segments(pattern.width(), pattern.height(), x, y) {
            for dy in 0..rect.h {
                for dx in 0..rect.w {
                    let (x, y) = (rect.x + dx, rect.y + dy);
                    if pattern.get(px + dx, py + dy) {
                        self.set(x, y, true);
                    } else if mode == PasteMode::Copy {
                        self.write_cell(x as usize, y as usize, false, self.age(x, y));
                    }
                }
            }
        }
        Ok(())
    }

    /// Kills every cell in `rect` and clears every trail in it, so the
    /// rectangle is blank. Ages stay.
    ///
    /// # Panics
    ///
    /// As `extract`.
    pub fn kill_rect(&mut self, rect: Rect) {
        self.check_rect(rect);
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                self.write_cell(x as usize, y as usize, false, self.age(x, y));
            }
        }
    }

    /// Mirrors the cells of `rect` in place; each keeps its age and trail.
    ///
    /// # Panics
    ///
    /// As `extract`.
    pub fn flip_rect(&mut self, rect: Rect, axis: Axis) {
        self.check_rect(rect);
        let Rect { x, y, w, h } = rect;
        match axis {
            Axis::Horizontal => {
                for row in y..y + h {
                    for i in 0..w / 2 {
                        self.swap_cells((x + i, row), (x + w - 1 - i, row));
                    }
                }
            }
            Axis::Vertical => {
                for i in 0..h / 2 {
                    for col in x..x + w {
                        self.swap_cells((col, y + i), (col, y + h - 1 - i));
                    }
                }
            }
        }
    }

    /// Records the cells of `rects`, which must not overlap.
    ///
    /// # Panics
    ///
    /// If a rectangle is empty or reaches outside the grid.
    pub fn capture(&self, rects: &[Rect]) -> Capture {
        let parts = rects
            .iter()
            .map(|&rect| {
                self.check_rect(rect);
                let mut bits = vec![0u64; (rect.w as usize * rect.h as usize).div_ceil(64)];
                let mut ages: Vec<(u16, u32)> = Vec::new();
                let mut n = 0;
                for y in rect.y..rect.y + rect.h {
                    for x in rect.x..rect.x + rect.w {
                        if self.get(x, y) {
                            bits[n / 64] |= 1 << (n % 64);
                            let age = self.age(x, y);
                            match ages.last_mut() {
                                Some((last, count)) if *last == age => *count += 1,
                                _ => ages.push((age, 1)),
                            }
                        }
                        n += 1;
                    }
                }
                CapturePart { rect, bits, ages }
            })
            .collect();
        Capture { parts }
    }

    /// Puts back the cells and the ages of the live cells of `capture`, and
    /// sets the trail to 0 inside its rectangles. Dead cells get age 0.
    ///
    /// # Panics
    ///
    /// If a rectangle of `capture` reaches outside the grid.
    pub fn restore(&mut self, capture: &Capture) {
        for part in &capture.parts {
            let rect = part.rect;
            self.check_rect(rect);
            let mut runs = part.ages.iter().flat_map(|&(age, n)| (0..n).map(move |_| age));
            let mut n = 0;
            for y in rect.y..rect.y + rect.h {
                for x in rect.x..rect.x + rect.w {
                    let alive = part.bits[n / 64] >> (n % 64) & 1 == 1;
                    let age = if alive { runs.next().unwrap_or(0) } else { 0 };
                    self.write_cell(x as usize, y as usize, alive, age);
                    n += 1;
                }
            }
        }
    }

    /// Records one cell. Positions outside the grid are dead with age 0.
    pub fn capture_cell(&self, x: u32, y: u32) -> CellState {
        CellState {
            x,
            y,
            alive: self.get(x, y),
            age: self.age(x, y),
        }
    }

    /// Puts back a recorded cell and sets its trail to 0. Positions outside
    /// the grid are ignored.
    pub fn restore_cell(&mut self, cell: CellState) {
        if cell.x < self.width() && cell.y < self.height() {
            self.write_cell(cell.x as usize, cell.y as usize, cell.alive, cell.age);
        }
    }

    /// Sets the generation count.
    pub fn set_generation(&mut self, generation: u64) {
        self.generation = generation;
    }

    /// A universe from an opened file: the alive cells of `cells`, the
    /// `width * height` row-major `ages`, no trail.
    pub fn from_parts(
        width: u32,
        height: u32,
        edges: Edges,
        generation: u64,
        cells: &Pattern,
        ages: &[u16],
    ) -> Result<Universe, PartsError> {
        let mut u = Universe::new(width, height, edges).map_err(PartsError::Size)?;
        if (cells.width(), cells.height()) != (width, height) {
            return Err(PartsError::PatternSize);
        }
        if ages.len() != width as usize * height as usize {
            return Err(PartsError::AgesLength);
        }
        for y in 0..height {
            for x in 0..width {
                if cells.get(x, y) {
                    u.set(x, y, true);
                }
            }
        }
        u.ages.copy_from_slice(ages);
        u.generation = generation;
        Ok(u)
    }

    /// The grid rectangles of a placement with the pattern offset of each
    /// one's top-left cell.
    fn segments(&self, w: u32, h: u32, x: i64, y: i64) -> Vec<(Rect, u32, u32)> {
        let spans = |start: i64, len: u32, grid: u32| -> Vec<(u32, u32, u32)> {
            // (grid start, length, pattern offset)
            match self.edges {
                Edges::Wrap => {
                    let at = start.rem_euclid(grid as i64) as u32;
                    let first = len.min(grid - at);
                    let mut v = vec![(at, first, 0)];
                    if first < len {
                        v.push((0, len - first, first));
                    }
                    v
                }
                Edges::Dead => {
                    let lo = start.max(0);
                    let hi = (start + len as i64).min(grid as i64);
                    if lo < hi {
                        vec![(lo as u32, (hi - lo) as u32, (lo - start) as u32)]
                    } else {
                        vec![]
                    }
                }
            }
        };
        let columns = spans(x, w, self.width());
        let mut out = Vec::new();
        for &(gy, rh, py) in &spans(y, h, self.height()) {
            for &(gx, rw, px) in &columns {
                out.push((Rect { x: gx, y: gy, w: rw, h: rh }, px, py));
            }
        }
        out
    }

    fn check_rect(&self, rect: Rect) {
        assert!(
            rect.w > 0
                && rect.h > 0
                && rect.x as u64 + rect.w as u64 <= self.width() as u64
                && rect.y as u64 + rect.h as u64 <= self.height() as u64,
            "rectangle must be non-empty and lie inside the grid"
        );
    }

    /// Writes a cell's alive state and age, and zeroes its trail.
    fn write_cell(&mut self, x: usize, y: usize, alive: bool, age: u16) {
        if self.grid.set(x, y, alive) {
            if alive {
                self.live_count += 1;
            } else {
                self.live_count -= 1;
            }
        }
        let i = y * self.grid.width + x;
        self.ages[i] = age;
        self.trail[i] = 0;
        self.trailing[y * self.grid.words_per_row + x / 64] &= !(1 << (x % 64));
    }

    /// Swaps two cells' alive state, age and trail.
    fn swap_cells(&mut self, (ax, ay): (u32, u32), (bx, by): (u32, u32)) {
        let (a, b) = (self.cell_parts(ax, ay), self.cell_parts(bx, by));
        self.put_cell_parts(ax, ay, b);
        self.put_cell_parts(bx, by, a);
    }

    fn cell_parts(&self, x: u32, y: u32) -> (bool, u16, u8) {
        (self.get(x, y), self.age(x, y), self.trail(x, y))
    }

    fn put_cell_parts(&mut self, x: u32, y: u32, (alive, age, trail): (bool, u16, u8)) {
        let (x, y) = (x as usize, y as usize);
        self.grid.set(x, y, alive);
        let i = y * self.grid.width + x;
        self.ages[i] = age;
        self.trail[i] = trail;
        let bit = 1u64 << (x % 64);
        let word = &mut self.trailing[y * self.grid.words_per_row + x / 64];
        if (1..=254).contains(&trail) {
            *word |= bit;
        } else {
            *word &= !bit;
        }
    }
}
