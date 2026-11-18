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
    for y in 0..u.height() {
        for x in 0..u.width() {
            let i = (y * u.width() + x) as usize;
            let trail = u.trail[i];
            let marked =
                (u.trailing[y as usize * u.grid.words_per_row + x as usize / 64] >> (x % 64)) & 1
                    == 1;
            assert_eq!(
                marked,
                (1..=254).contains(&trail),
                "trailing bit at ({x}, {y})"
            );
            assert!(
                !(u.get(x, y) && trail != 0),
                "live cell ({x}, {y}) has trail {trail}"
            );
        }
    }
    for y in 0..u.grid.height {
        let last = u.trailing[(y + 1) * u.grid.words_per_row - 1];
        assert_eq!(last & !mask, 0, "trailing padding bit set in row {y}");
    }
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

/// Ages and trail after one generation from `before` to `after`, cell by
/// cell, the reference for the layers `step()` keeps.
fn naive_layers(
    before: &[Vec<bool>],
    after: &[Vec<bool>],
    layers: &[Vec<(u16, u8)>],
) -> Vec<Vec<(u16, u8)>> {
    (0..layers.len())
        .map(|y| {
            (0..layers[y].len())
                .map(|x| {
                    let (age, trail) = layers[y][x];
                    match (before[y][x], after[y][x]) {
                        (true, true) => (age.saturating_add(1), 0),
                        (false, true) => (0, 0),
                        (true, false) => (age, 1),
                        (false, false) if (1..=254).contains(&trail) => (age, trail + 1),
                        (false, false) => (age, trail),
                    }
                })
                .collect()
        })
        .collect()
}

fn layer_rows(u: &Universe) -> Vec<Vec<(u16, u8)>> {
    (0..u.height())
        .map(|y| {
            (0..u.width())
                .map(|x| (u.age(x, y), u.trail(x, y)))
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
                let mut expected_layers = layer_rows(&u);
                for generation in 1..=20 {
                    u.step();
                    let before = expected;
                    expected = naive_step(&before, edges == Edges::Wrap);
                    expected_layers = naive_layers(&before, &expected, &expected_layers);
                    assert!(
                        to_rows(&u) == expected,
                        "{width} x {height} {edges:?}, generation {generation}"
                    );
                    assert!(
                        layer_rows(&u) == expected_layers,
                        "layers, {width} x {height} {edges:?}, generation {generation}"
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

// Ages

#[test]
fn block_cells_reach_age_n_after_n_generations() {
    let block = [(3, 3), (4, 3), (3, 4), (4, 4)];
    let mut u = universe(8, 8, Edges::Dead, &block);
    for n in 1..=10 {
        u.step();
        for &(x, y) in &block {
            assert_eq!(u.age(x, y), n, "({x}, {y})");
        }
    }
}

#[test]
fn blinker_ends_stay_at_age_zero_and_its_centre_counts_up() {
    let mut u = universe(5, 5, Edges::Dead, &[(1, 2), (2, 2), (3, 2)]);
    for n in 1..=6 {
        u.step();
        assert_eq!(u.age(2, 2), n);
        let ends = if n % 2 == 1 {
            [(2, 1), (2, 3)]
        } else {
            [(1, 2), (3, 2)]
        };
        for (x, y) in ends {
            assert_eq!(u.age(x, y), 0, "({x}, {y}) at generation {n}");
        }
    }
}

#[test]
fn age_saturates_at_u16_max() {
    let block = [(3, 3), (4, 3), (3, 4), (4, 4)];
    let mut u = universe(8, 8, Edges::Dead, &block);
    u.ages[3 * 8 + 3] = u16::MAX - 1;
    u.step();
    assert_eq!(u.age(3, 3), u16::MAX);
    u.step();
    assert_eq!(u.age(3, 3), u16::MAX);
    assert_eq!(u.age(4, 4), 2);
}

#[test]
fn set_gives_age_zero_to_new_cells_and_keeps_live_ones() {
    let block = [(3, 3), (4, 3), (3, 4), (4, 4)];
    let mut u = universe(8, 8, Edges::Dead, &block);
    u.step();
    u.step();
    u.set(3, 3, true);
    assert_eq!(u.age(3, 3), 2);
    u.set(3, 3, false);
    assert_eq!(u.age(3, 3), 2, "a cell killed by an edit keeps its age");
    u.set(3, 3, true);
    assert_eq!(u.age(3, 3), 0);
    assert_eq!((u.age(8, 0), u.trail(0, 8)), (0, 0));
}

#[test]
fn a_dead_cell_keeps_the_age_it_died_at() {
    // A block cell crowded by two added cells dies after three generations.
    let mut u = universe(9, 9, Edges::Dead, &[(1, 1), (2, 1), (1, 2), (2, 2)]);
    for _ in 0..3 {
        u.step();
    }
    u.set(3, 3, true);
    u.set(3, 2, true);
    u.step();
    assert!(!u.get(2, 2));
    assert_eq!((u.age(2, 2), u.trail(2, 2)), (3, 1));
}

// Trail

#[test]
fn trail_counts_up_after_a_death_by_the_rule_and_stops_at_255() {
    let mut u = universe(5, 5, Edges::Dead, &[(2, 2)]);
    for n in 1..=300u32 {
        u.step();
        assert_eq!(u.trail(2, 2) as u32, n.min(255), "generation {n}");
        assert_storage_consistent(&u);
    }
}

#[test]
fn a_cell_killed_by_set_has_no_trail() {
    let mut u = universe(5, 5, Edges::Dead, &[(2, 2)]);
    u.set(2, 2, false);
    assert_eq!(u.trail(2, 2), 0);
    u.step();
    assert_eq!(u.trail(2, 2), 0);
    assert_storage_consistent(&u);
}

#[test]
fn birth_resets_the_trail() {
    // The blinker's ends die and are born again every other generation.
    let mut u = universe(5, 5, Edges::Dead, &[(1, 2), (2, 2), (3, 2)]);
    u.step();
    assert_eq!((u.trail(1, 2), u.trail(2, 1)), (1, 0));
    u.step();
    assert_eq!((u.trail(1, 2), u.trail(2, 1)), (0, 1));

    // A saturated trail is reset by a birth too.
    let mut u = universe(7, 7, Edges::Dead, &[(3, 3)]);
    for _ in 0..300 {
        u.step();
    }
    assert_eq!(u.trail(3, 3), 255);
    for (x, y) in [(2, 2), (4, 2), (3, 4)] {
        u.set(x, y, true);
    }
    u.step();
    assert!(u.get(3, 3));
    assert_eq!((u.age(3, 3), u.trail(3, 3)), (0, 0));
    assert_storage_consistent(&u);

    // And so is a trail cut short by an edit.
    let mut u = universe(5, 5, Edges::Dead, &[(2, 2)]);
    u.step();
    u.set(2, 2, true);
    assert_eq!(u.trail(2, 2), 0);
    assert_storage_consistent(&u);
}

#[test]
fn clear_and_random_fill_zero_both_layers() {
    let zeroed = |u: &Universe| u.ages.iter().all(|&a| a == 0) && u.trail.iter().all(|&t| t == 0);
    let mut u = Universe::new(70, 20, Edges::Wrap).unwrap();
    u.random_fill(0.4, 3);
    for _ in 0..10 {
        u.step();
    }
    assert!(!zeroed(&u));
    u.clear();
    assert!(zeroed(&u));
    assert_storage_consistent(&u);

    u.random_fill(0.4, 3);
    for _ in 0..10 {
        u.step();
    }
    u.random_fill(0.4, 4);
    assert!(zeroed(&u));
    assert_storage_consistent(&u);
}

// clone_from

#[test]
fn clone_from_gives_an_equal_universe() {
    let mut source = Universe::new(100, 40, Edges::Dead).unwrap();
    source.random_fill(0.3, 5);
    for _ in 0..7 {
        source.step();
    }
    let assert_equal = |copy: &Universe| {
        assert_eq!(
            (copy.width(), copy.height(), copy.edges()),
            (100, 40, Edges::Dead)
        );
        assert_eq!(
            (copy.generation(), copy.live_count()),
            (source.generation(), source.live_count())
        );
        assert_eq!(copy.grid.words_per_row, source.grid.words_per_row);
        assert_eq!(copy.grid.cells, source.grid.cells);
        assert_eq!(copy.ages, source.ages);
        assert_eq!(copy.trail, source.trail);
        assert_eq!(copy.trailing, source.trailing);
    };

    // Into a larger and a smaller universe, so buffers shrink and grow.
    for (width, height) in [(300, 300), (3, 3)] {
        let mut copy = Universe::new(width, height, Edges::Wrap).unwrap();
        copy.random_fill(0.5, 1);
        copy.step();
        copy.clone_from(&source);
        assert_equal(&copy);
        let mut stepped = source.clone();
        copy.step();
        stepped.step();
        assert_eq!(copy.grid.cells, stepped.grid.cells);
        assert_eq!(copy.ages, stepped.ages);
    }
    assert_equal(&source.clone());
}

#[test]
fn clone_from_reuses_the_targets_buffers() {
    let source = universe(64, 64, Edges::Wrap, &GLIDER);
    let mut copy = Universe::new(64, 64, Edges::Wrap).unwrap();
    let buffers = (
        copy.grid.cells.as_ptr(),
        copy.ages.as_ptr(),
        copy.trail.as_ptr(),
        copy.trailing.as_ptr(),
    );
    copy.clone_from(&source);
    assert_eq!(
        (
            copy.grid.cells.as_ptr(),
            copy.ages.as_ptr(),
            copy.trail.as_ptr(),
            copy.trailing.as_ptr(),
        ),
        buffers
    );
}

// copy_ages and copy_trail

#[test]
fn copy_ages_saturates_to_255_in_byte_order() {
    let mut u = Universe::new(40, 3, Edges::Dead).unwrap();
    for (x, age) in [(0, 1), (1, 255), (2, 256), (3, u16::MAX), (5, 7)] {
        u.ages[40 + x] = age;
    }
    let mut out = vec![99];
    u.copy_ages(0, 1, 32, 1, &mut out);
    assert_eq!(out, [99, 0xffff_ff01, 7 << 8, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn copy_ages_partial_last_word_and_trail() {
    // Width 42: the last word of a 64-wide region holds cells 40 and 41, then
    // zeros.
    let mut u = Universe::new(42, 3, Edges::Dead).unwrap();
    u.ages[42 + 40] = 3;
    u.ages[42 + 41] = 4;
    u.trail[42 + 41] = 9;
    u.trail[42 + 39] = 200;
    let mut ages = Vec::new();
    u.copy_ages(0, 1, 64, 1, &mut ages);
    let mut expected = vec![0; 16];
    expected[10] = 3 | 4 << 8;
    assert_eq!(ages, expected);

    let mut trail = Vec::new();
    u.copy_trail(0, 0, 64, 2, &mut trail);
    assert_eq!(trail.len(), 32);
    assert!(trail[..16].iter().all(|&w| w == 0));
    let mut expected = vec![0; 16];
    expected[9] = 200 << 24;
    expected[10] = 9 << 8;
    assert_eq!(trail[16..], expected);
}

#[test]
fn copy_trail_region_at_the_right_edge() {
    let mut u = universe(200, 3, Edges::Dead, &[(150, 1)]);
    u.step();
    u.step();
    let mut expected = vec![0; 16];
    let mut trail = Vec::new();
    u.copy_trail(128, 1, 96, 1, &mut trail);
    expected[(150 - 128) / 4] = 2 << 16;
    assert_eq!(trail.len(), 24);
    assert_eq!(trail[..16], expected);
    assert!(trail[16..].iter().all(|&w| w == 0));

    // Cells 196..199 fill the last in-grid word.
    for x in 196..200 {
        u.ages[2 * 200 + x] = x as u16 - 195;
    }
    let mut ages = Vec::new();
    u.copy_ages(192, 2, 32, 1, &mut ages);
    assert_eq!(ages, [0, 0x0403_0201, 0, 0, 0, 0, 0, 0]);
}

#[test]
#[should_panic(expected = "aligned to 32")]
fn copy_ages_rejects_unaligned_columns() {
    let u = universe(70, 10, Edges::Wrap, &[]);
    u.copy_ages(0, 0, 48, 1, &mut Vec::new());
}

#[test]
#[should_panic(expected = "inside the grid")]
fn copy_trail_rejects_regions_past_the_grid() {
    let u = universe(70, 10, Edges::Wrap, &[]);
    u.copy_trail(64, 0, 64, 1, &mut Vec::new());
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

// Patterns and regions

use crate::{Axis, PasteMode, Pattern, Rect};

fn pattern(w: u32, h: u32, cells: &[(u32, u32)]) -> Pattern {
    let mut p = Pattern::new(w, h);
    for &(x, y) in cells {
        p.set(x, y, true);
    }
    p
}

/// Every cell's alive state, age and trail.
fn full_state(u: &Universe) -> Vec<(bool, u16, u8)> {
    let mut v = Vec::new();
    for y in 0..u.height() {
        for x in 0..u.width() {
            v.push((u.get(x, y), u.age(x, y), u.trail(x, y)));
        }
    }
    v
}

/// Alive state and age of live cells only, which is what a capture keeps.
fn live_state(u: &Universe) -> Vec<(bool, u16)> {
    full_state(u)
        .into_iter()
        .map(|(a, age, _)| (a, if a { age } else { 0 }))
        .collect()
}

#[test]
fn place_or_keeps_ages_and_gives_new_cells_age_zero() {
    let mut u = universe(10, 10, Edges::Dead, &[(3, 3), (4, 3), (3, 4), (4, 4)]);
    u.step();
    u.step();
    assert_eq!(u.age(3, 3), 2);
    let p = pattern(3, 3, &[(0, 0), (1, 1)]);
    u.place(&p, 2, 2, PasteMode::Or).unwrap();
    assert_eq!(u.age(3, 3), 2);
    assert!(u.get(2, 2));
    assert_eq!(u.age(2, 2), 0);
    assert_eq!(u.live_count(), 5);
    assert_storage_consistent(&u);
}

#[test]
fn place_copy_replaces_the_rectangle_without_trail() {
    let mut u = universe(10, 10, Edges::Dead, &[(2, 2), (3, 2), (2, 3), (3, 3)]);
    u.step();
    u.step();
    let before = u.age(2, 2);
    assert!(before > 0);
    // A dead cell with a trail elsewhere inside the rectangle's reach.
    let mut v = universe(10, 10, Edges::Dead, &[(8, 8)]);
    v.step();
    assert_eq!(v.trail(8, 8), 1);
    let p = pattern(2, 2, &[(0, 0)]);
    u.place(&p, 2, 2, PasteMode::Copy).unwrap();
    assert!(u.get(2, 2) && !u.get(3, 2) && !u.get(2, 3) && !u.get(3, 3));
    assert_eq!(u.age(2, 2), before);
    assert_eq!(u.trail(3, 2), 0);
    assert_eq!(u.live_count(), 1);
    // Placing over a dead cell with a trail: Copy leaves a dead cell's trail.
    v.place(&p, 8, 8, PasteMode::Copy).unwrap();
    assert!(v.get(8, 8) && v.trail(8, 8) == 0 && v.age(8, 8) == 0);
    assert_storage_consistent(&u);
    assert_storage_consistent(&v);
}

#[test]
fn place_wraps_across_edges_and_corners() {
    let p = pattern(2, 2, &[(0, 0), (1, 0), (0, 1), (1, 1)]);
    for (x, y, expected) in [
        (-1, 3, vec![(7, 3), (7, 4), (0, 3), (0, 4)]),
        (3, -1, vec![(3, 5), (4, 5), (3, 0), (4, 0)]),
        (7, 5, vec![(7, 5), (0, 5), (7, 0), (0, 0)]),
        (-1, -1, vec![(7, 5), (0, 5), (7, 0), (0, 0)]),
    ] {
        let mut u = Universe::new(8, 6, Edges::Wrap).unwrap();
        u.place(&p, x, y, PasteMode::Or).unwrap();
        assert_eq!(alive(&u), expected.into_iter().collect::<Cells>(), "at {x},{y}");
    }
}

#[test]
fn place_on_dead_edges_drops_the_outside_part() {
    let p = pattern(2, 2, &[(0, 0), (1, 0), (0, 1), (1, 1)]);
    let mut u = Universe::new(8, 6, Edges::Dead).unwrap();
    u.place(&p, -1, -1, PasteMode::Or).unwrap();
    assert_eq!(alive(&u), [(0, 0)].into());
    u.clear();
    u.place(&p, 7, 5, PasteMode::Or).unwrap();
    assert_eq!(alive(&u), [(7, 5)].into());
    u.clear();
    u.place(&p, 20, 20, PasteMode::Or).unwrap();
    assert_eq!(u.live_count(), 0);
}

#[test]
fn place_refuses_a_pattern_larger_than_the_grid() {
    let mut u = Universe::new(8, 6, Edges::Wrap).unwrap();
    let err = u.place(&Pattern::new(9, 2), 0, 0, PasteMode::Or).unwrap_err();
    assert_eq!((err.pattern, err.grid), ((9, 2), (8, 6)));
    assert!(u.place(&Pattern::new(2, 7), 0, 0, PasteMode::Or).is_err());
    assert!(u.place(&Pattern::new(8, 6), 0, 0, PasteMode::Or).is_ok());
}

#[test]
fn footprint_agrees_with_where_place_writes() {
    let full = |w, h| {
        let mut p = Pattern::new(w, h);
        for y in 0..h {
            for x in 0..w {
                p.set(x, y, true);
            }
        }
        p
    };
    for edges in [Edges::Wrap, Edges::Dead] {
        for (w, h) in [(1, 1), (3, 2), (8, 6)] {
            for x in -9..=9 {
                for y in -7..=7 {
                    let mut u = Universe::new(8, 6, edges).unwrap();
                    u.place(&full(w, h), x, y, PasteMode::Or).unwrap();
                    let mut covered = Cells::new();
                    for r in u.footprint(w, h, x, y) {
                        for cy in r.y..r.y + r.h {
                            for cx in r.x..r.x + r.w {
                                assert!(covered.insert((cx, cy)), "overlap");
                            }
                        }
                    }
                    assert_eq!(alive(&u), covered, "{edges:?} {w}x{h} at {x},{y}");
                }
            }
        }
    }
    let u = Universe::new(8, 6, Edges::Wrap).unwrap();
    assert_eq!(u.footprint(2, 2, 7, 5).len(), 4);
}

#[test]
fn flip_twice_is_identity_and_flip_moves_ages_and_trail() {
    let mut u = Universe::new(12, 9, Edges::Wrap).unwrap();
    u.random_fill(0.4, 5);
    for _ in 0..7 {
        u.step();
    }
    let r = Rect { x: 2, y: 1, w: 7, h: 6 };
    let before = full_state(&u);
    for axis in [Axis::Horizontal, Axis::Vertical] {
        let mut v = u.clone();
        v.flip_rect(r, axis);
        assert_ne!(full_state(&v), before);
        for dy in 0..r.h {
            for dx in 0..r.w {
                let (mx, my) = match axis {
                    Axis::Horizontal => (r.w - 1 - dx, dy),
                    Axis::Vertical => (dx, r.h - 1 - dy),
                };
                let (a, b) = ((r.x + dx, r.y + dy), (r.x + mx, r.y + my));
                assert_eq!(
                    (v.get(a.0, a.1), v.age(a.0, a.1), v.trail(a.0, a.1)),
                    (u.get(b.0, b.1), u.age(b.0, b.1), u.trail(b.0, b.1))
                );
            }
        }
        assert_storage_consistent(&v);
        // The trailing bit layer follows: stepping matches stepping a copy
        // flipped the same way only if trail counters keep fading.
        v.flip_rect(r, axis);
        assert_eq!(full_state(&v), before);
    }
}

#[test]
fn extract_after_place_gives_the_pattern_back() {
    let p = pattern(5, 4, &[(0, 0), (4, 3), (2, 1), (1, 3)]);
    for edges in [Edges::Wrap, Edges::Dead] {
        let mut u = Universe::new(20, 20, edges).unwrap();
        u.place(&p, 6, 7, PasteMode::Or).unwrap();
        assert_eq!(u.extract(Rect { x: 6, y: 7, w: 5, h: 4 }), p);
    }
}

#[test]
fn kill_rect_kills_without_trail_and_keeps_the_rest() {
    let mut u = universe(8, 8, Edges::Dead, &[(1, 1), (2, 2), (6, 6)]);
    u.kill_rect(Rect { x: 0, y: 0, w: 4, h: 4 });
    assert_eq!(alive(&u), [(6, 6)].into());
    assert_eq!(u.trail(1, 1), 0);
    assert_eq!(u.live_count(), 1);
    assert_storage_consistent(&u);
}

#[test]
fn erasing_a_dead_cell_leaves_its_trail() {
    let mut u = universe(8, 8, Edges::Dead, &[(4, 4)]);
    u.step();
    assert_eq!(u.trail(4, 4), 1);
    u.set(4, 4, false);
    assert_eq!(u.trail(4, 4), 1);
}

#[test]
fn kill_rect_and_copy_placing_clear_trail_in_the_rectangle() {
    let mut u = universe(8, 8, Edges::Dead, &[(4, 4), (1, 1)]);
    u.step();
    assert_eq!((u.trail(4, 4), u.trail(1, 1)), (1, 1));
    u.kill_rect(Rect { x: 3, y: 3, w: 2, h: 2 });
    assert_eq!((u.trail(4, 4), u.trail(1, 1)), (0, 1));
    u.place(&pattern(2, 2, &[(0, 0)]), 0, 0, PasteMode::Copy).unwrap();
    assert!(u.get(0, 0) && !u.get(1, 1) && u.trail(1, 1) == 0);
    assert_storage_consistent(&u);
}

#[test]
fn restore_after_edits_restores_cells_ages_and_count() {
    let mut u = Universe::new(40, 30, Edges::Wrap).unwrap();
    u.random_fill(0.3, 11);
    for _ in 0..5 {
        u.step();
    }
    let whole = Rect { x: 0, y: 0, w: 40, h: 30 };
    let part = Rect { x: 5, y: 4, w: 20, h: 12 };
    let blob = pattern(6, 6, &[(0, 0), (1, 1), (5, 5), (2, 4)]);
    for rects in [vec![whole], vec![part]] {
        let mut v = u.clone();
        let (g, count) = (v.generation(), v.live_count());
        let expected = live_state(&v);
        let capture = v.capture(&rects);
        v.flip_rect(part, Axis::Horizontal);
        v.kill_rect(Rect { x: 8, y: 8, w: 6, h: 6 });
        v.place(&blob, 20, 12, PasteMode::Copy).unwrap();
        v.place(&blob, 38, 28, PasteMode::Or).unwrap();
        v.restore(&capture);
        if rects[0] == whole {
            assert_eq!(live_state(&v), expected);
            assert_eq!(v.live_count(), count);
        } else {
            let after = live_state(&v);
            for y in 0..30u32 {
                for x in 0..40u32 {
                    let i = (y * 40 + x) as usize;
                    let inside = (5..25).contains(&x) && (4..16).contains(&y);
                    if inside {
                        assert_eq!(after[i], expected[i], "{x},{y}");
                    }
                }
            }
        }
        assert_eq!(v.generation(), g);
        assert_storage_consistent(&v);
        for y in rects[0].y..rects[0].y + rects[0].h {
            for x in rects[0].x..rects[0].x + rects[0].w {
                assert_eq!(v.trail(x, y), 0);
            }
        }
    }
}

#[test]
fn restore_after_random_fill_and_clear_restores_everything() {
    let mut u = Universe::new(33, 17, Edges::Dead).unwrap();
    u.random_fill(0.5, 2);
    u.step();
    let whole = Rect { x: 0, y: 0, w: 33, h: 17 };
    let expected = live_state(&u);
    let count = u.live_count();
    let capture = u.capture(&[whole]);
    u.random_fill(0.2, 3);
    u.restore(&capture);
    assert_eq!(live_state(&u), expected);
    assert_eq!(u.live_count(), count);
    u.clear();
    u.restore(&capture);
    assert_eq!(live_state(&u), expected);
    assert_storage_consistent(&u);
}

#[test]
fn restore_cell_puts_back_one_cell() {
    let mut u = universe(8, 8, Edges::Dead, &[(2, 2), (2, 3), (3, 2), (3, 3)]);
    u.step();
    u.step();
    let age = u.age(2, 2);
    let cell = u.capture_cell(2, 2);
    u.set(2, 2, false);
    u.restore_cell(cell);
    assert!(u.get(2, 2));
    assert_eq!(u.age(2, 2), age);
    let dead = u.capture_cell(6, 6);
    u.set(6, 6, true);
    u.restore_cell(dead);
    assert!(!u.get(6, 6));
    assert_eq!(u.live_count(), 4);
    assert_storage_consistent(&u);
}

#[test]
fn whole_grid_capture_is_compact() {
    let mut u = Universe::new(MAX_SIDE, MAX_SIDE, Edges::Wrap).unwrap();
    u.random_fill(0.25, 1);
    let capture = u.capture(&[Rect { x: 0, y: 0, w: MAX_SIDE, h: MAX_SIDE }]);
    let bits = (MAX_SIDE as usize * MAX_SIDE as usize) / 8;
    assert!(
        capture.bytes() - bits < 1 << 20,
        "ages took {} bytes",
        capture.bytes() - bits
    );
    assert!(capture.bytes() < bits + (1 << 20));
}

#[test]
fn thumbnail_downsamples_and_keeps_single_cells() {
    let mut big = Pattern::new(2048, 2048);
    big.set(2047, 2047, true);
    let t = big.thumbnail(64);
    assert_eq!((t.width(), t.height()), (64, 64));
    assert_eq!(t.live_count(), 1);
    let mut one = Pattern::new(2048, 2048);
    one.set(1000, 5, true);
    assert_eq!(one.thumbnail(64).live_count(), 1);
    let wide = Pattern::new(2000, 100).thumbnail(64);
    assert_eq!((wide.width(), wide.height()), (64, 4));
    let small = pattern(10, 3, &[(0, 0), (9, 2), (4, 1)]);
    assert_eq!(small.thumbnail(64), small);
}

#[test]
fn pattern_words_and_flip() {
    let p = pattern(40, 2, &[(0, 0), (33, 0), (39, 1)]);
    let mut words = Vec::new();
    p.to_words(&mut words);
    assert_eq!(words, [40, 2, 1, 1 << 1, 0, 1 << 7]);
    assert_eq!(p.live_count(), 3);
    assert_eq!(p.flipped(Axis::Horizontal).flipped(Axis::Horizontal), p);
    assert!(p.flipped(Axis::Horizontal).get(39, 0));
    assert!(p.flipped(Axis::Vertical).get(39, 0));
    assert!(!p.get(40, 0));
    assert!(Pattern::new(1, 1).live_count() == 0);
}

#[test]
fn from_parts_builds_a_universe_with_ages() {
    let p = pattern(4, 3, &[(1, 1)]);
    let mut ages = vec![0u16; 12];
    ages[5] = 9;
    let u = Universe::from_parts(4, 3, Edges::Dead, 7, &p, &ages).unwrap();
    assert_eq!((u.generation(), u.live_count(), u.age(1, 1)), (7, 1, 9));
    assert!(Universe::from_parts(4, 3, Edges::Dead, 0, &p, &ages[1..]).is_err());
    assert!(Universe::from_parts(5, 3, Edges::Dead, 0, &p, &ages).is_err());
    assert!(Universe::from_parts(2, 3, Edges::Dead, 0, &p, &ages).is_err());
    let mut u = u;
    u.set_generation(3);
    assert_eq!(u.generation(), 3);
}

// Timing

#[test]
#[ignore = "timing; run in release with --ignored --nocapture"]
fn timing_2048_squared() {
    let mut u = Universe::new(MAX_SIDE, MAX_SIDE, Edges::Wrap).unwrap();
    u.random_fill(0.25, 1);
    let time_steps = |u: &mut Universe, steps: u32, when: &str| {
        let start = std::time::Instant::now();
        for _ in 0..steps {
            u.step();
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / steps as f64;
        let trailing: u32 = u.trailing.iter().map(|w| w.count_ones()).sum();
        println!(
            "2048 x 2048 {when}: {ms:.3} ms per step ({steps} steps, {} live and {trailing} trailing at the end)",
            u.live_count()
        );
    };
    time_steps(&mut u, 10, "right after a 25 % fill");
    while u.generation() < 1000 {
        u.step();
    }
    time_steps(&mut u, 100, "after 1000 generations");
}
