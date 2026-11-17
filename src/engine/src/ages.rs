//! The age and trail layers, brought up to date after each generation from
//! the alive bits before and after it. Only cells alive in either generation
//! or trailing are visited.

use crate::grid::Grid;

/// Updates `ages`, `trail` and `trailing` for the step from `grid` to `next`.
/// `ages` and `trail` hold one entry per cell, row-major; `trailing` is
/// bit-packed like the grid and marks the cells whose trail is 1 to 254.
pub(crate) fn update(
    grid: &Grid,
    next: &[u64],
    ages: &mut [u16],
    trail: &mut [u8],
    trailing: &mut [u64],
) {
    let n = grid.words_per_row;
    for y in 0..grid.height {
        for k in 0..n {
            let i = y * n + k;
            let (old, new) = (grid.cells[i], next[i]);
            let born = new & !old;
            let died = old & !new;
            // Trailing cells are dead; any of them born now stop trailing.
            let fading = trailing[i] & !new;
            let base = y * grid.width + 64 * k;

            for_each_bit(old & new, |b| {
                ages[base + b] = ages[base + b].saturating_add(1);
            });
            for_each_bit(born, |b| {
                ages[base + b] = 0;
                trail[base + b] = 0;
            });
            for_each_bit(died, |b| trail[base + b] = 1);
            let mut saturated = 0;
            for_each_bit(fading, |b| {
                trail[base + b] += 1;
                if trail[base + b] == 255 {
                    saturated |= 1 << b;
                }
            });
            trailing[i] = (fading & !saturated) | died;
        }
    }
}

/// Calls `f` with the position of each set bit of `word`, lowest first.
#[inline(always)]
fn for_each_bit(mut word: u64, mut f: impl FnMut(usize)) {
    while word != 0 {
        f(word.trailing_zeros() as usize);
        word &= word - 1;
    }
}
