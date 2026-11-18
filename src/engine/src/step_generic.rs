//! One generation of any birth/survival rule on any grid, computed with
//! bit-sliced adders over whole words.
//!
//! Each row is first copied five times, shifted so that bit x of copy `d`
//! holds cell `x + d - 2`. A cell's neighbors are then picked, one word for
//! 64 cells at a time, from the copies of its own row and the rows above and
//! below, by grid and by row parity. They are added into a 4-bit count held
//! in four bit planes, and the rule's birth and survival counts are matched
//! against it.

use crate::grid::Grid;
use crate::{Rule, Tiling};

/// Shifted copies kept per row: cells `x - 2` to `x + 2`.
const SHIFTS: usize = 5;

/// Bits of cells whose `x` is even. A cell points up on a triangular grid
/// when `x + y` is even, and a word starts at an even `x`.
const EVEN_X: u64 = 0x5555_5555_5555_5555;

/// Writes the next generation of `grid` into `next` and returns its live-cell
/// count. `next` must hold as many words as `grid`.
pub(crate) fn step(grid: &Grid, tiling: Tiling, rule: Rule, wrap: bool, next: &mut [u64]) -> u32 {
    let n = grid.words_per_row;
    let height = grid.height;
    let mask = grid.padding_mask();
    let counts = Counts::new(rule);

    // Rows outside a dead-edged grid stay zero.
    let mut above = vec![0u64; SHIFTS * n];
    let mut middle = vec![0u64; SHIFTS * n];
    let mut below = vec![0u64; SHIFTS * n];
    if wrap {
        shift_row(grid, height - 1, true, &mut above);
    }
    shift_row(grid, 0, wrap, &mut middle);

    let mut live = 0;
    for y in 0..height {
        if y + 1 < height {
            shift_row(grid, y + 1, wrap, &mut below);
        } else if wrap {
            shift_row(grid, 0, true, &mut below);
        } else {
            below.fill(0);
        }

        let out = &mut next[y * n..(y + 1) * n];
        let a = copies(&above, n);
        let m = copies(&middle, n);
        let b = copies(&below, n);
        let even_row = y % 2 == 0;
        match tiling {
            Tiling::Square => {
                for k in 0..n {
                    let neighbors = [
                        a[1][k], a[2][k], a[3][k], m[1][k], m[3][k], b[1][k], b[2][k], b[3][k],
                    ];
                    out[k] = counts.next(m[2][k], neighbors);
                }
            }
            // Rows above and below: cells x - 1 and x from an even row, x
            // and x + 1 from an odd one.
            Tiling::Hexagonal => {
                let (left, right) = if even_row { (1, 2) } else { (2, 3) };
                for k in 0..n {
                    let neighbors = [
                        a[left][k],
                        a[right][k],
                        m[1][k],
                        m[3][k],
                        b[left][k],
                        b[right][k],
                    ];
                    out[k] = counts.next(m[2][k], neighbors);
                }
            }
            // An up triangle's third edge is below it, a down triangle's
            // above.
            Tiling::Triangular3 => {
                let up = if even_row { EVEN_X } else { !EVEN_X };
                for k in 0..n {
                    let vertical = (b[2][k] & up) | (a[2][k] & !up);
                    out[k] = counts.next(m[2][k], [m[1][k], m[3][k], vertical]);
                }
            }
            // x - 2 to x + 2 in the own row, x - 1 to x + 1 above and below,
            // and x ± 2 below an up triangle or above a down one.
            Tiling::Triangular12 => {
                let up = if even_row { EVEN_X } else { !EVEN_X };
                for k in 0..n {
                    let far_west = (b[0][k] & up) | (a[0][k] & !up);
                    let far_east = (b[4][k] & up) | (a[4][k] & !up);
                    #[rustfmt::skip]
                    let neighbors = [
                        m[0][k], m[1][k], m[3][k], m[4][k],
                        a[1][k], a[2][k], a[3][k],
                        b[1][k], b[2][k], b[3][k],
                        far_west, far_east,
                    ];
                    out[k] = counts.next(m[2][k], neighbors);
                }
            }
        }
        out[n - 1] &= mask;
        live += out.iter().map(|word| word.count_ones()).sum::<u32>();

        std::mem::swap(&mut above, &mut middle);
        std::mem::swap(&mut middle, &mut below);
    }
    live
}

/// The shifted copies of one row held in `buffer`, `x - 2` first.
fn copies(buffer: &[u64], n: usize) -> [&[u64]; SHIFTS] {
    std::array::from_fn(|i| &buffer[i * n..(i + 1) * n])
}

/// Row `y` copied into `out` once per shift: bit x of copy `i` holds cell
/// `x + i - 2`, the cell across the edge on a torus and dead past the edge
/// otherwise. Padding positions may hold garbage; the step masks them out.
fn shift_row(grid: &Grid, y: usize, wrap: bool, out: &mut [u64]) {
    let row = grid.row(y);
    let n = row.len();
    let width = grid.width;
    for (i, copy) in out.chunks_exact_mut(n).enumerate() {
        match i {
            0 | 1 => {
                // West: bit x holds cell x - s.
                let s = 2 - i;
                for k in 0..n {
                    let from_previous = if k > 0 { row[k - 1] >> (64 - s) } else { 0 };
                    copy[k] = (row[k] << s) | from_previous;
                }
                if wrap {
                    for x in 0..s {
                        copy[0] |= (grid.get(width - s + x, y) as u64) << x;
                    }
                }
            }
            2 => copy.copy_from_slice(row),
            _ => {
                // East: bit x holds cell x + s. The row's padding is zero, so
                // the last s cells read dead before wrapping.
                let s = i - 2;
                for k in 0..n {
                    let from_next = if k + 1 < n { row[k + 1] << (64 - s) } else { 0 };
                    copy[k] = (row[k] >> s) | from_next;
                }
                if wrap {
                    for x in 0..s {
                        let at = width - s + x;
                        copy[at / 64] |= (grid.get(x, y) as u64) << (at % 64);
                    }
                }
            }
        }
    }
}

/// The neighbor counts a rule names, as bit patterns to match against the
/// four count planes.
struct Counts {
    terms: Vec<Term>,
}

struct Term {
    /// Plane `p` is all ones where bit `p` of the count is set.
    planes: [u64; 4],
    /// All ones when the count gives birth.
    birth: u64,
    /// All ones when the count lets a live cell survive.
    survival: u64,
}

impl Counts {
    fn new(rule: Rule) -> Counts {
        let all = |mask: u16, n: u32| if mask >> n & 1 == 1 { u64::MAX } else { 0 };
        let terms = (0..16)
            .filter(|&n| (rule.birth | rule.survival) >> n & 1 == 1)
            .map(|n| Term {
                planes: std::array::from_fn(|p| all(n as u16, p as u32)),
                birth: all(rule.birth, n),
                survival: all(rule.survival, n),
            })
            .collect();
        Counts { terms }
    }

    /// The next state of the 64 cells of `cell` from their neighbor words.
    #[inline(always)]
    fn next<const N: usize>(&self, cell: u64, neighbors: [u64; N]) -> u64 {
        // Ripple-add each neighbor word into the planes; no count reaches 16.
        let [mut p0, mut p1, mut p2, mut p3] = [0u64; 4];
        for word in neighbors {
            let c0 = p0 & word;
            p0 ^= word;
            let c1 = p1 & c0;
            p1 ^= c0;
            let c2 = p2 & c1;
            p2 ^= c1;
            p3 ^= c2;
        }
        let (mut birth, mut survival) = (0, 0);
        for t in &self.terms {
            let [q0, q1, q2, q3] = t.planes;
            let equal = !((p0 ^ q0) | (p1 ^ q1) | (p2 ^ q2) | (p3 ^ q3));
            birth |= equal & t.birth;
            survival |= equal & t.survival;
        }
        (birth & !cell) | (survival & cell)
    }
}
