//! The Game of Life engine: a B3/S23 universe with wrapping or dead edges.
//! Pure and deterministic: no I/O, no rendering, no clock.

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

/// A grid of cells, its edge mode and its generation count.
pub struct Universe {
    grid: Grid,
    /// Scratch buffer the next generation is written into.
    next: Vec<u64>,
    edges: Edges,
    generation: u64,
    live_count: u32,
}

impl Clone for Universe {
    /// Copies the cells but not the scratch buffer, so snapshots stay cheap.
    fn clone(&self) -> Universe {
        Universe {
            grid: self.grid.clone(),
            next: Vec::new(),
            edges: self.edges,
            generation: self.generation,
            live_count: self.live_count,
        }
    }
}

impl Universe {
    /// An empty universe at generation 0.
    pub fn new(width: u32, height: u32, edges: Edges) -> Result<Universe, SizeError> {
        let sides = MIN_SIDE..=MAX_SIDE;
        if !sides.contains(&width) || !sides.contains(&height) {
            return Err(SizeError { width, height });
        }
        Ok(Universe {
            grid: Grid::new(width as usize, height as usize),
            next: Vec::new(),
            edges,
            generation: 0,
            live_count: 0,
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

    /// Takes effect from the next generation.
    pub fn set_edges(&mut self, edges: Edges) {
        self.edges = edges;
    }

    /// Sets one cell. Positions outside the grid are ignored.
    pub fn set(&mut self, x: u32, y: u32, alive: bool) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        if self.grid.set(x as usize, y as usize, alive) {
            if alive {
                self.live_count += 1;
            } else {
                self.live_count -= 1;
            }
        }
    }

    /// Kills every cell and resets the generation to 0.
    pub fn clear(&mut self) {
        self.grid.cells.fill(0);
        self.live_count = 0;
        self.generation = 0;
    }

    /// Replaces every cell: each is alive with probability `density` (0.0 to
    /// 1.0), drawn from `seed`. Resets the generation to 0.
    pub fn random_fill(&mut self, density: f64, seed: u64) {
        self.live_count = rng::random_fill(&mut self.grid, density, seed);
        self.generation = 0;
    }

    /// Advances one generation.
    pub fn step(&mut self) {
        self.next.resize(self.grid.cells.len(), 0);
        self.live_count = step::step(&self.grid, self.edges == Edges::Wrap, &mut self.next);
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
        let (x0, y0, w, h) = (x0 as usize, y0 as usize, w as usize, h as usize);
        assert!(
            x0 % 32 == 0 && w % 32 == 0,
            "region columns must be aligned to 32"
        );
        assert!(
            x0 + w <= self.grid.width.next_multiple_of(32) && y0 + h <= self.grid.height,
            "region must lie inside the grid"
        );
        out.reserve(h * w / 32);
        for y in y0..y0 + h {
            let row = self.grid.row(y);
            for x in (x0..x0 + w).step_by(32) {
                out.push((row[x / 64] >> (x % 64)) as u32);
            }
        }
    }
}
