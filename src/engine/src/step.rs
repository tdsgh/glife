//! One generation of B3/S23, computed with bitwise adders over whole words.
//!
//! Each row is first reduced to its horizontal sums: for every cell, the 2-bit
//! count of the cell and its west and east neighbours, kept as a low-bit and a
//! high-bit word. A cell's eight-neighbour count is then the sums of the rows
//! above and below plus its own row's sum minus itself.

use crate::grid::Grid;

/// Writes the next generation of `grid` into `next` and returns its live-cell
/// count. `next` must hold as many words as `grid`.
pub(crate) fn step(grid: &Grid, wrap: bool, next: &mut [u64]) -> u32 {
    let n = grid.words_per_row;
    let height = grid.height;
    let mask = grid.padding_mask();

    // Horizontal sums: low bits in [..n], high bits in [n..]. Rows outside a
    // dead-edged grid stay zero.
    let mut above = vec![0u64; 2 * n];
    let mut middle = vec![0u64; 2 * n];
    let mut below = vec![0u64; 2 * n];
    if wrap {
        row_sums(grid, height - 1, true, &mut above);
    }
    row_sums(grid, 0, wrap, &mut middle);

    let mut live = 0;
    for y in 0..height {
        if y + 1 < height {
            row_sums(grid, y + 1, wrap, &mut below);
        } else if wrap {
            row_sums(grid, 0, true, &mut below);
        } else {
            below.fill(0);
        }

        let row = grid.row(y);
        let out = &mut next[y * n..(y + 1) * n];
        let (a0, a1) = above.split_at(n);
        let (h0, h1) = middle.split_at(n);
        let (b0, b1) = below.split_at(n);
        for k in 0..n {
            let c = row[k];
            // This row's sum without the cell itself.
            let m0 = h0[k] ^ c;
            let m1 = h1[k] & (h0[k] | !c);
            // Total = s0 + 2 * (a1 + m1 + b1 + carry); it is 2 or 3 exactly
            // when one of those four high bits is set.
            let s0 = a0[k] ^ m0 ^ b0[k];
            let carry = (a0[k] & m0) | (b0[k] & (a0[k] ^ m0));
            let p = a1[k] ^ m1;
            let q = a1[k] & m1;
            let r = b1[k] ^ carry;
            let t = b1[k] & carry;
            let two_or_three = (p ^ r) & !(q | t);
            out[k] = two_or_three & (s0 | c);
        }
        out[n - 1] &= mask;
        live += out.iter().map(|word| word.count_ones()).sum::<u32>();

        std::mem::swap(&mut above, &mut middle);
        std::mem::swap(&mut middle, &mut below);
    }
    live
}

/// The horizontal sums of row `y` into `out` (low bits, then high bits).
/// Padding positions may hold garbage; the step masks them out.
fn row_sums(grid: &Grid, y: usize, wrap: bool, out: &mut [u64]) {
    let row = grid.row(y);
    let n = row.len();
    let last_bit = (grid.width - 1) % 64;
    // On a torus, cell 0's west neighbour is the last cell, and the last
    // cell's east neighbour is cell 0.
    let (wrap_west, wrap_east) = if wrap {
        ((row[n - 1] >> last_bit) & 1, (row[0] & 1) << last_bit)
    } else {
        (0, 0)
    };
    let (lo, hi) = out.split_at_mut(n);
    for k in 0..n {
        let c = row[k];
        // Bit x of `west` holds cell x - 1; bit x of `east` holds cell x + 1.
        let from_previous = if k > 0 { row[k - 1] >> 63 } else { wrap_west };
        let from_next = if k + 1 < n {
            row[k + 1] << 63
        } else {
            wrap_east
        };
        let west = (c << 1) | from_previous;
        let east = (c >> 1) | from_next;
        lo[k] = west ^ c ^ east;
        hi[k] = (west & c) | (east & (west ^ c));
    }
}
