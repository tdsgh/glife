use std::collections::BTreeSet;

use crate::{Edges, Universe, MAX_SIDE, MIN_SIDE};

type Cells = BTreeSet<(u32, u32)>;

const GLIDER: [(u32, u32); 5] = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];

fn universe(width: u32, height: u32, edges: Edges, cells: &[(u32, u32)]) -> Universe {
    let mut u = Universe::new(width, height, edges).unwrap();
    for &(x, y) in cells {
        u.set(x, y, true);
    }
    u
}

fn alive(u: &Universe) -> Cells {
    let mut cells = Cells::new();
    for y in 0..u.height() {
        for x in 0..u.width() {
            if u.get(x, y) {
                cells.insert((x, y));
            }
        }
    }
    cells
}

fn shifted(cells: &[(u32, u32)], dx: u32, dy: u32) -> Cells {
    cells.iter().map(|&(x, y)| (x + dx, y + dy)).collect()
}

fn assert_storage_consistent(u: &Universe) {
    let mask = u.grid.padding_mask();
    for y in 0..u.grid.height {
        let last = u.grid.row(y)[u.grid.words_per_row - 1];
        assert_eq!(last & !mask, 0, "padding bit set in row {y}");
    }
    let popcount: u32 = u.grid.cells.iter().map(|w| w.count_ones()).sum();
    assert_eq!(u.live_count(), popcount);
}

/// Cell-by-cell B3/S23, the reference for `step()`.
fn naive_step(cells: &[Vec<bool>], wrap: bool) -> Vec<Vec<bool>> {
    let height = cells.len() as i64;
    let width = cells[0].len() as i64;
    let alive_at = |x: i64, y: i64| -> bool {
        if wrap {
            cells[y.rem_euclid(height) as usize][x.rem_euclid(width) as usize]
        } else {
            (0..width).contains(&x) && (0..height).contains(&y) && cells[y as usize][x as usize]
        }
    };
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| {
                    let mut neighbours = 0;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            if (dx, dy) != (0, 0) && alive_at(x + dx, y + dy) {
                                neighbours += 1;
                            }
                        }
                    }
                    neighbours == 3 || (neighbours == 2 && alive_at(x, y))
                })
                .collect()
        })
        .collect()
}

fn to_rows(u: &Universe) -> Vec<Vec<bool>> {
    (0..u.height())
        .map(|y| (0..u.width()).map(|x| u.get(x, y)).collect())
        .collect()
}

// Rules

#[test]
fn underpopulation_kills_cells_with_fewer_than_two_neighbours() {
    // A lone cell and a pair: each has 0 or 1 neighbours.
    let mut u = universe(9, 9, Edges::Dead, &[(1, 1), (5, 5), (6, 5)]);
    u.step();
    assert!(alive(&u).is_empty());
}

#[test]
fn survival_with_two_neighbours() {
    let mut u = universe(7, 7, Edges::Dead, &[(1, 1), (2, 2), (3, 3)]);
    u.step();
    assert!(u.get(2, 2));
}

#[test]
fn survival_with_three_neighbours() {
    let mut u = universe(7, 7, Edges::Dead, &[(2, 2), (1, 1), (3, 1), (2, 3)]);
    u.step();
    assert!(u.get(2, 2));
}

#[test]
fn overpopulation_kills_cells_with_more_than_three_neighbours() {
    let mut u = universe(7, 7, Edges::Dead, &[(2, 2), (1, 1), (3, 1), (1, 3), (3, 3)]);
    u.step();
    assert!(!u.get(2, 2));
}

#[test]
fn birth_with_exactly_three_neighbours() {
    let mut u = universe(7, 7, Edges::Dead, &[(1, 1), (3, 1), (2, 3)]);
    u.step();
    assert!(u.get(2, 2));
}

// Patterns

#[test]
fn blinker_has_period_two() {
    let horizontal: Cells = [(1, 2), (2, 2), (3, 2)].into();
    let vertical: Cells = [(2, 1), (2, 2), (2, 3)].into();
    for edges in [Edges::Wrap, Edges::Dead] {
        let mut u = universe(5, 5, edges, &[(1, 2), (2, 2), (3, 2)]);
        u.step();
        assert_eq!(alive(&u), vertical);
        u.step();
        assert_eq!(alive(&u), horizontal);
    }
}

#[test]
fn block_stays_still() {
    let block = [(3, 3), (4, 3), (3, 4), (4, 4)];
    let mut u = universe(8, 8, Edges::Dead, &block);
    for _ in 0..5 {
        u.step();
        assert_eq!(alive(&u), block.into());
    }
}

#[test]
fn glider_moves_one_cell_diagonally_every_four_generations() {
    let mut u = universe(16, 16, Edges::Dead, &GLIDER);
    for moved in 1..=4 {
        for _ in 0..4 {
            u.step();
        }
        assert_eq!(alive(&u), shifted(&GLIDER, moved, moved));
    }
    assert_eq!(u.generation(), 16);
}

#[test]
fn glider_returns_to_start_on_wrapping_square_grids() {
    for n in [8, 64, 65] {
        let mut u = universe(n, n, Edges::Wrap, &GLIDER);
        for _ in 0..4 * n {
            u.step();
            assert_eq!(u.live_count(), 5, "glider broke on {n} x {n}");
        }
        assert_eq!(alive(&u), GLIDER.into(), "{n} x {n}");
    }
}

// Dead edges

#[test]
fn glider_driven_into_dead_corner_stays_inside_the_grid() {
    for width in [10, 63, 64, 65, 130] {
        let height = 10;
        let start = shifted(&GLIDER, width - 6, height - 6);
        let start: Vec<_> = start.into_iter().collect();
        let mut u = universe(width, height, Edges::Dead, &start);
        for _ in 0..60 {
            u.step();
            assert_storage_consistent(&u);
        }
    }
}

// Reference check

#[test]
fn step_matches_naive_stepper_on_random_grids() {
    let mut seed = 1;
    for width in [3, 5, 31, 63, 64, 65, 127, 128, 129, 200] {
        for height in [3, 7, 64] {
            for edges in [Edges::Wrap, Edges::Dead] {
                seed += 1;
                let mut u = Universe::new(width, height, edges).unwrap();
                u.random_fill(0.35, seed);
                let mut expected = to_rows(&u);
                for generation in 1..=20 {
                    u.step();
                    expected = naive_step(&expected, edges == Edges::Wrap);
                    assert!(
                        to_rows(&u) == expected,
                        "{width} x {height} {edges:?}, generation {generation}"
                    );
                    assert_storage_consistent(&u);
                }
            }
        }
    }
}

// Editing

#[test]
fn set_keeps_live_count_exact() {
    let mut u = universe(5, 5, Edges::Wrap, &[]);
    u.set(1, 1, true);
    u.set(1, 1, true);
    u.set(2, 1, true);
    assert_eq!(u.live_count(), 2);
    u.set(1, 1, false);
    u.set(1, 1, false);
    u.set(3, 3, false);
    assert_eq!(u.live_count(), 1);
    u.set(5, 0, true);
    u.set(0, 5, true);
    assert_eq!(u.live_count(), 1);
    assert!(!u.get(5, 0));
}

#[test]
fn clear_kills_every_cell_and_resets_the_generation() {
    let mut u = universe(8, 8, Edges::Wrap, &GLIDER);
    u.step();
    u.clear();
    assert!(alive(&u).is_empty());
    assert_eq!(u.live_count(), 0);
    assert_eq!(u.generation(), 0);
}

#[test]
fn random_fill_is_deterministic_per_seed_and_resets_the_generation() {
    let mut a = Universe::new(100, 50, Edges::Wrap).unwrap();
    let mut b = Universe::new(100, 50, Edges::Wrap).unwrap();
    a.step();
    a.random_fill(0.3, 42);
    b.random_fill(0.3, 42);
    assert_eq!(a.generation(), 0);
    assert_eq!(a.grid.cells, b.grid.cells);
    assert_storage_consistent(&a);
    b.random_fill(0.3, 43);
    assert_ne!(a.grid.cells, b.grid.cells);
}

#[test]
fn random_fill_density_quarter_on_largest_grid() {
    let mut u = Universe::new(MAX_SIDE, MAX_SIDE, Edges::Wrap).unwrap();
    u.random_fill(0.25, 7);
    let share = u.live_count() as f64 / (MAX_SIDE as f64 * MAX_SIDE as f64);
    assert!((0.24..=0.26).contains(&share), "share {share}");
    assert_storage_consistent(&u);
}

#[test]
fn random_fill_extreme_densities() {
    let mut u = Universe::new(70, 3, Edges::Dead).unwrap();
    u.random_fill(1.0, 1);
    assert_eq!(u.live_count(), 70 * 3);
    assert_storage_consistent(&u);
    u.random_fill(0.0, 1);
    assert_eq!(u.live_count(), 0);
}

// copy_region

#[test]
fn copy_region_partial_last_word() {
    let mut u = Universe::new(40, 3, Edges::Wrap).unwrap();
    u.random_fill(1.0, 0);
    let mut out = Vec::new();
    u.copy_region(0, 1, 64, 2, &mut out);
    assert_eq!(out, [u32::MAX, 0xff, u32::MAX, 0xff]);
}

#[test]
fn copy_region_at_the_right_edge() {
    let cells = [(160, 1), (191, 1), (192, 1), (199, 1), (130, 2)];
    let u = universe(200, 3, Edges::Wrap, &cells);
    let mut out = Vec::new();
    u.copy_region(160, 0, 64, 3, &mut out);
    assert_eq!(out, [0, 0, 1 | 1 << 31, 1 | 1 << 7, 0, 0]);
}

#[test]
fn copy_region_single_row_appends() {
    let u = universe(
        70,
        10,
        Edges::Wrap,
        &[(0, 5), (33, 5), (64, 5), (69, 5), (1, 4)],
    );
    let mut out = vec![99];
    u.copy_region(0, 5, 96, 1, &mut out);
    assert_eq!(out, [99, 1, 1 << 1, 1 | 1 << 5]);
}

#[test]
#[should_panic(expected = "aligned to 32")]
fn copy_region_rejects_unaligned_columns() {
    let u = universe(70, 10, Edges::Wrap, &[]);
    u.copy_region(16, 0, 32, 1, &mut Vec::new());
}

// new

#[test]
fn new_accepts_sides_within_limits_only() {
    assert_eq!((MIN_SIDE, MAX_SIDE), (3, 2048));
    for (width, height) in [(2, 10), (10, 2), (2049, 10), (10, 2049)] {
        assert!(
            Universe::new(width, height, Edges::Wrap).is_err(),
            "{width} x {height}"
        );
    }
    for (width, height) in [(3, 3), (2048, 2048), (3, 2048), (2048, 3)] {
        let u = Universe::new(width, height, Edges::Dead).unwrap();
        assert_eq!(
            (u.width(), u.height(), u.edges()),
            (width, height, Edges::Dead)
        );
        assert_eq!((u.generation(), u.live_count()), (0, 0));
    }
}

#[test]
fn set_edges_applies_from_the_next_generation() {
    // A blinker lying across the left/right boundary.
    let mut u = universe(5, 5, Edges::Wrap, &[(4, 2), (0, 2), (1, 2)]);
    u.step();
    assert_eq!(alive(&u), [(0, 1), (0, 2), (0, 3)].into());
    u.set_edges(Edges::Dead);
    u.step();
    assert_eq!(alive(&u), [(0, 2), (1, 2)].into());
}

// Timing

#[test]
#[ignore = "timing; run in release with --ignored --nocapture"]
fn timing_2048_squared() {
    let mut u = Universe::new(MAX_SIDE, MAX_SIDE, Edges::Wrap).unwrap();
    u.random_fill(0.25, 1);
    u.step();
    let steps = 100;
    let start = std::time::Instant::now();
    for _ in 0..steps {
        u.step();
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / steps as f64;
    println!(
        "2048 x 2048: {ms:.3} ms per step ({steps} steps, {} live at the end)",
        u.live_count()
    );
}
