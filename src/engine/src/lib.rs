//! The Game of Life engine: a B3/S23 universe with wrapping or dead edges.
//! Pure and deterministic: no I/O, no rendering, no clock.

mod ages;
mod grid;
mod rng;
mod step;
#[cfg(test)]
mod tests;

use std::fmt;

use grid::Grid;

/// The shortest side a universe may have.
pub const MIN_SIDE: u32 = 3;
/// The longest side a universe may have.
pub const MAX_SIDE: u32 = 2048;

/// What lies past the grid's boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edges {
    /// The grid is a torus.
    Wrap,
    /// Every position outside the grid counts as a dead cell.
    Dead,
}

/// A width or height outside `MIN_SIDE..=MAX_SIDE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeError {
    width: u32,
    height: u32,
}

impl fmt::Display for SizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "universe size {} x {} is outside {MIN_SIDE}..={MAX_SIDE} per side",
            self.width, self.height
        )
    }
}

impl std::error::Error for SizeError {}

/// A grid of cells, its edge mode, its generation count, and each cell's age
/// and trail.
pub struct Universe {
    grid: Grid,
    /// Scratch buffer the next generation is written into.
    next: Vec<u64>,
    edges: Edges,
    generation: u64,
    live_count: u32,
    /// Generations each cell has stayed alive, row-major, saturating. A dead
    /// cell keeps the age it died at.
    ages: Vec<u16>,
    /// Generations since the rule killed each cell, row-major, saturating at
    /// 255. 0 means no trail; a live cell always has 0.
    trail: Vec<u8>,
    /// Bit-packed like the grid: the cells whose trail is 1 to 254.
    trailing: Vec<u64>,
}

impl Clone for Universe {
    /// Copies the cells and layers but not the scratch buffer, so snapshots
    /// stay cheap.
    fn clone(&self) -> Universe {
        Universe {
            grid: self.grid.clone(),
            next: Vec::new(),
            edges: self.edges,
            generation: self.generation,
            live_count: self.live_count,
            ages: self.ages.clone(),
            trail: self.trail.clone(),
            trailing: self.trailing.clone(),
        }
    }

    /// Copies into the buffers `self` already holds, allocating only when
    /// `source` is larger.
    fn clone_from(&mut self, source: &Universe) {
        self.grid.width = source.grid.width;
        self.grid.height = source.grid.height;
        self.grid.words_per_row = source.grid.words_per_row;
        self.grid.cells.clone_from(&source.grid.cells);
        self.edges = source.edges;
        self.generation = source.generation;
        self.live_count = source.live_count;
        self.ages.clone_from(&source.ages);
        self.trail.clone_from(&source.trail);
        self.trailing.clone_from(&source.trailing);
    }
}

impl Universe {
    /// An empty universe at generation 0.
    pub fn new(width: u32, height: u32, edges: Edges) -> Result<Universe, SizeError> {
        let sides = MIN_SIDE..=MAX_SIDE;
        if !sides.contains(&width) || !sides.contains(&height) {
            return Err(SizeError { width, height });
        }
        let grid = Grid::new(width as usize, height as usize);
        let cells = grid.width * grid.height;
        Ok(Universe {
            next: Vec::new(),
            edges,
            generation: 0,
            live_count: 0,
            ages: vec![0; cells],
            trail: vec![0; cells],
            trailing: vec![0; grid.cells.len()],
            grid,
        })
    }

    pub fn width(&self) -> u32 {
        self.grid.width as u32
    }

    pub fn height(&self) -> u32 {
        self.grid.height as u32
    }

    pub fn edges(&self) -> Edges {
        self.edges
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn live_count(&self) -> u32 {
        self.live_count
    }

    /// Whether a cell is alive. Positions outside the grid are dead.
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width() && y < self.height() && self.grid.get(x as usize, y as usize)
    }

    /// Generations a live cell has stayed alive, or the age a dead cell died
    /// at. Positions outside the grid have age 0.
    pub fn age(&self, x: u32, y: u32) -> u16 {
        if x >= self.width() || y >= self.height() {
            return 0;
        }
        self.ages[y as usize * self.grid.width + x as usize]
    }

    /// Generations since the rule killed a cell, saturating at 255; 0 means
    /// no trail. Positions outside the grid have trail 0.
    pub fn trail(&self, x: u32, y: u32) -> u8 {
        if x >= self.width() || y >= self.height() {
            return 0;
        }
        self.trail[y as usize * self.grid.width + x as usize]
    }

    /// Takes effect from the next generation.
    pub fn set_edges(&mut self, edges: Edges) {
        self.edges = edges;
    }

    /// Sets one cell. Positions outside the grid are ignored. A cell set alive
    /// from dead gets age 0 and no trail; a cell set dead keeps its age and
    /// leaves no trail.
    pub fn set(&mut self, x: u32, y: u32, alive: bool) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if self.grid.set(x, y, alive) {
            if alive {
                self.live_count += 1;
                let i = y * self.grid.width + x;
                self.ages[i] = 0;
                self.trail[i] = 0;
                self.trailing[y * self.grid.words_per_row + x / 64] &= !(1 << (x % 64));
            } else {
                // A live cell has no trail, so there is none to clear.
                self.live_count -= 1;
            }
        }
    }

    /// Kills every cell, zeroes every age and trail, and resets the
    /// generation to 0.
    pub fn clear(&mut self) {
        self.grid.cells.fill(0);
        self.ages.fill(0);
        self.trail.fill(0);
        self.trailing.fill(0);
        self.live_count = 0;
        self.generation = 0;
    }

    /// Replaces every cell: each is alive with probability `density` (0.0 to
    /// 1.0), drawn from `seed`. Zeroes every age and trail and resets the
    /// generation to 0.
    pub fn random_fill(&mut self, density: f64, seed: u64) {
        self.live_count = rng::random_fill(&mut self.grid, density, seed);
        self.ages.fill(0);
        self.trail.fill(0);
        self.trailing.fill(0);
        self.generation = 0;
    }

    /// Advances one generation.
    pub fn step(&mut self) {
        self.next.resize(self.grid.cells.len(), 0);
        self.live_count = step::step(&self.grid, self.edges == Edges::Wrap, &mut self.next);
        ages::update(
            &self.grid,
            &self.next,
            &mut self.ages,
            &mut self.trail,
            &mut self.trailing,
        );
        std::mem::swap(&mut self.grid.cells, &mut self.next);
        self.generation += 1;
    }

    /// Appends to `out` the alive bits of the region `x0..x0 + w`,
    /// `y0..y0 + h` in frame layout v1 order: `h` rows of `w / 32` words, bit
    /// `i` of word `k` in row `r` being cell `(x0 + 32k + i, y0 + r)`. Bits
    /// past the grid's width are zero.
    ///
    /// # Panics
    ///
    /// If `x0` or `w` is not a multiple of 32, if the region ends past the
    /// grid's width rounded up to 32, or if it ends past the grid's height.
    pub fn copy_region(&self, x0: u32, y0: u32, w: u32, h: u32, out: &mut Vec<u32>) {
        let (x0, y0, w, h) = self.check_region(x0, y0, w, h);
        out.reserve(h * w / 32);
        for y in y0..y0 + h {
            let row = self.grid.row(y);
            for x in (x0..x0 + w).step_by(32) {
                out.push((row[x / 64] >> (x % 64)) as u32);
            }
        }
    }

    /// Appends to `out` the ages of the region `x0..x0 + w`, `y0..y0 + h` in
    /// frame layout v1 order: `h` rows of `w` bytes, each `min(age, 255)`,
    /// cell `x0 + 4j + b` in byte `b` of word `j`. Bytes past the grid's width
    /// are zero.
    ///
    /// # Panics
    ///
    /// As `copy_region`.
    pub fn copy_ages(&self, x0: u32, y0: u32, w: u32, h: u32, out: &mut Vec<u32>) {
        let region = self.check_region(x0, y0, w, h);
        copy_byte_layer(
            &self.ages,
            self.grid.width,
            region,
            |age| age.min(255) as u8,
            out,
        );
    }

    /// Appends to `out` the trail of the region `x0..x0 + w`, `y0..y0 + h` in
    /// the same layout as `copy_ages`.
    ///
    /// # Panics
    ///
    /// As `copy_region`.
    pub fn copy_trail(&self, x0: u32, y0: u32, w: u32, h: u32, out: &mut Vec<u32>) {
        let region = self.check_region(x0, y0, w, h);
        copy_byte_layer(&self.trail, self.grid.width, region, |trail| trail, out);
    }

    /// The region as `usize`s, after the checks every `copy_*` makes.
    fn check_region(&self, x0: u32, y0: u32, w: u32, h: u32) -> (usize, usize, usize, usize) {
        let (x0, y0, w, h) = (x0 as usize, y0 as usize, w as usize, h as usize);
        assert!(
            x0 % 32 == 0 && w % 32 == 0,
            "region columns must be aligned to 32"
        );
        assert!(
            x0 + w <= self.grid.width.next_multiple_of(32) && y0 + h <= self.grid.height,
            "region must lie inside the grid"
        );
        (x0, y0, w, h)
    }
}

/// Appends a row-major layer's region in frame layout v1 byte order, one
/// `to_byte` per cell, zero past the row's `width`.
fn copy_byte_layer<T: Copy>(
    layer: &[T],
    width: usize,
    (x0, y0, w, h): (usize, usize, usize, usize),
    to_byte: impl Fn(T) -> u8,
    out: &mut Vec<u32>,
) {
    out.reserve(h * w / 4);
    for y in y0..y0 + h {
        let row = &layer[y * width..(y + 1) * width];
        let inside = &row[x0.min(width)..(x0 + w).min(width)];
        let (quads, rest) = inside.as_chunks::<4>();
        out.extend(quads.iter().map(|&[a, b, c, d]| {
            u32::from_le_bytes([to_byte(a), to_byte(b), to_byte(c), to_byte(d)])
        }));
        let mut written = quads.len();
        if !rest.is_empty() {
            let mut bytes = [0; 4];
            for (byte, &value) in bytes.iter_mut().zip(rest) {
                *byte = to_byte(value);
            }
            out.push(u32::from_le_bytes(bytes));
            written += 1;
        }
        out.resize(out.len() + w / 4 - written, 0);
    }
}
