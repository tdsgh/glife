//! Seeded random fill with SplitMix64.

use crate::grid::Grid;

/// Replaces every cell of `grid`, row by row: a cell is alive when its draw
/// is below `density`. Returns the live-cell count.
pub(crate) fn random_fill(grid: &mut Grid, density: f64, seed: u64) -> u32 {
    // Draws are compared on their top 53 bits, so density 1.0 fills every cell.
    let threshold = (density.clamp(0.0, 1.0) * (1u64 << 53) as f64) as u64;
    let mut state = seed;
    let mut live = 0;
    for y in 0..grid.height {
        for x in 0..grid.width {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            let alive = (z >> 11) < threshold;
            grid.set(x, y, alive);
            live += alive as u32;
        }
    }
    live
}
