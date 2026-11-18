//! Grids other than the square one, rules other than B3/S23, and the generic
//! step.

use super::{alive, assert_storage_consistent, layer_rows, naive_layers, to_rows, Cells};
use crate::{Edges, PartsError, Pattern, Rule, RuleError, SizeError, Tiling, Universe};

/// SplitMix64, for test inputs drawn from a fixed seed.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `lo..=hi`.
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next() % (hi - lo + 1) as u64) as u32
    }

    /// A rule `tiling` can run.
    fn rule(&mut self, tiling: Tiling) -> Rule {
        let counts = (1u16 << (tiling.neighbors() + 1)) - 1;
        Rule {
            birth: self.next() as u16 & counts & !1,
            survival: self.next() as u16 & counts,
        }
    }
}

/// The neighbors of cell `(x, y)` from the coordinate rules, one entry per
/// neighbor position: a cell reached from two sides of a small torus is
/// listed twice.
fn neighbors(
    tiling: Tiling,
    (x, y): (u32, u32),
    (w, h): (u32, u32),
    wrap: bool,
) -> Vec<(u32, u32)> {
    let (x, y) = (x as i64, y as i64);
    let up = (x + y) % 2 == 0;
    let mut offsets: Vec<(i64, i64)> = Vec::new();
    match tiling {
        Tiling::Square => {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if (dx, dy) != (0, 0) {
                        offsets.push((dx, dy));
                    }
                }
            }
        }
        Tiling::Hexagonal => {
            let (left, right) = if y % 2 == 0 { (-1, 0) } else { (0, 1) };
            offsets.extend([
                (-1, 0),
                (1, 0),
                (left, -1),
                (right, -1),
                (left, 1),
                (right, 1),
            ]);
        }
        Tiling::Triangular3 => {
            offsets.extend([(-1, 0), (1, 0), (0, if up { 1 } else { -1 })]);
        }
        Tiling::Triangular12 => {
            // Three cells on the side of the point, five on the side of the
            // base.
            let (point, base) = if up { (-1, 1) } else { (1, -1) };
            offsets.extend([(-2, 0), (-1, 0), (1, 0), (2, 0)]);
            offsets.extend((-1..=1).map(|dx| (dx, point)));
            offsets.extend((-2..=2).map(|dx| (dx, base)));
        }
    }
    let (w, h) = (w as i64, h as i64);
    offsets
        .into_iter()
        .filter_map(|(dx, dy)| {
            let (nx, ny) = (x + dx, y + dy);
            if wrap {
                Some((nx.rem_euclid(w) as u32, ny.rem_euclid(h) as u32))
            } else if (0..w).contains(&nx) && (0..h).contains(&ny) {
                Some((nx as u32, ny as u32))
            } else {
                None
            }
        })
        .collect()
}

/// Every cell's neighbor list, row-major.
fn neighbor_table(tiling: Tiling, w: u32, h: u32, wrap: bool) -> Vec<Vec<(u32, u32)>> {
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|cell| neighbors(tiling, cell, (w, h), wrap))
        .collect()
}

/// Cell-by-cell `rule` over the neighbors in `table`, the reference for
/// `step()` on every grid.
fn naive_step(cells: &[Vec<bool>], table: &[Vec<(u32, u32)>], rule: Rule) -> Vec<Vec<bool>> {
    let width = cells[0].len();
    (0..cells.len())
        .map(|y| {
            (0..width)
                .map(|x| {
                    let count = table[y * width + x]
                        .iter()
                        .filter(|&&(nx, ny)| cells[ny as usize][nx as usize])
                        .count();
                    let mask = if cells[y][x] {
                        rule.survival
                    } else {
                        rule.birth
                    };
                    mask >> count & 1 == 1
                })
                .collect()
        })
        .collect()
}

// Reference check

#[test]
fn every_grid_matches_the_naive_stepper_under_presets_and_random_rules() {
    let mut draws = Draws(3);
    for tiling in Tiling::ALL {
        let (even_width, even_height) = tiling.wrap_needs_even();
        for edges in [Edges::Wrap, Edges::Dead] {
            let wrap = edges == Edges::Wrap;
            let mut sizes = vec![(3, 3), (4, 70), (63, 5), (64, 6), (65, 9), (70, 4)];
            for _ in 0..4 {
                sizes.push((draws.range(3, 70), draws.range(3, 70)));
            }
            for (mut width, mut height) in sizes {
                if wrap && even_width && width % 2 == 1 {
                    width += 1;
                }
                if wrap && even_height && height % 2 == 1 {
                    height += 1;
                }
                let table = neighbor_table(tiling, width, height, wrap);
                let mut rules: Vec<Rule> = tiling.presets().iter().map(|&(_, rule)| rule).collect();
                for _ in 0..3 {
                    rules.push(draws.rule(tiling));
                }
                for rule in rules {
                    let mut u = Universe::new(width, height, edges, tiling).unwrap();
                    u.set_rule(rule).unwrap();
                    u.random_fill(0.1 + 0.1 * draws.range(0, 5) as f64, draws.next());
                    let mut expected = to_rows(&u);
                    let mut expected_layers = layer_rows(&u);
                    for generation in 1..=50 {
                        u.step();
                        let before = expected;
                        expected = naive_step(&before, &table, rule);
                        expected_layers = naive_layers(&before, &expected, &expected_layers);
                        let case = format!("{tiling:?} {rule} {width} x {height} {edges:?}, generation {generation}");
                        assert!(to_rows(&u) == expected, "{case}");
                        assert!(layer_rows(&u) == expected_layers, "layers, {case}");
                        assert_storage_consistent(&u);
                    }
                }
            }
        }
    }
}

#[test]
fn generic_step_agrees_with_the_life_step_on_the_square_grid() {
    for (width, height, edges) in [
        (64, 64, Edges::Wrap),
        (129, 40, Edges::Dead),
        (3, 3, Edges::Wrap),
    ] {
        let mut u = Universe::new(width, height, edges, Tiling::Square).unwrap();
        u.random_fill(0.3, width as u64);
        let wrap = edges == Edges::Wrap;
        for _ in 0..30 {
            let mut generic = vec![0; u.grid.cells.len()];
            let live =
                crate::step_generic::step(&u.grid, Tiling::Square, Rule::LIFE, wrap, &mut generic);
            u.step();
            assert!(generic == u.grid.cells, "{width} x {height} {edges:?}");
            assert_eq!(live, u.live_count());
        }
    }
}

// Neighbors by hand

/// Checks one cell's neighbors on a wrapping grid, both in the reference
/// and in the step: under B1/S a lone cell dies and gives birth to exactly
/// its neighbors.
fn assert_neighbors(tiling: Tiling, size: (u32, u32), cell: (u32, u32), expected: &[(u32, u32)]) {
    let expected: Cells = expected.iter().copied().collect();
    assert_eq!(
        expected.len(),
        tiling.neighbors() as usize,
        "{tiling:?} {cell:?}: hand list"
    );
    let listed = neighbors(tiling, cell, size, true);
    assert_eq!(
        listed.len(),
        expected.len(),
        "{tiling:?} {cell:?}: reference"
    );
    assert_eq!(
        listed.into_iter().collect::<Cells>(),
        expected,
        "{tiling:?} {cell:?}: reference"
    );

    let mut u = Universe::new(size.0, size.1, Edges::Wrap, tiling).unwrap();
    u.set_rule(Rule::parse("B1/S", tiling).unwrap()).unwrap();
    u.set(cell.0, cell.1, true);
    u.step();
    assert_eq!(alive(&u), expected, "{tiling:?} {cell:?}: step");
}

#[test]
fn hexagonal_neighbors_by_hand() {
    let t = Tiling::Hexagonal;
    let size = (7, 6);
    // Even row: cells x - 1 and x above and below.
    assert_neighbors(
        t,
        size,
        (3, 2),
        &[(2, 2), (4, 2), (2, 1), (3, 1), (2, 3), (3, 3)],
    );
    // Odd row: cells x and x + 1 above and below.
    assert_neighbors(
        t,
        size,
        (3, 3),
        &[(2, 3), (4, 3), (3, 2), (4, 2), (3, 4), (4, 4)],
    );
    // Corners.
    assert_neighbors(
        t,
        size,
        (0, 0),
        &[(6, 0), (1, 0), (6, 5), (0, 5), (6, 1), (0, 1)],
    );
    assert_neighbors(
        t,
        size,
        (6, 0),
        &[(5, 0), (0, 0), (5, 5), (6, 5), (5, 1), (6, 1)],
    );
    assert_neighbors(
        t,
        size,
        (0, 5),
        &[(6, 5), (1, 5), (0, 4), (1, 4), (0, 0), (1, 0)],
    );
    assert_neighbors(
        t,
        size,
        (6, 5),
        &[(5, 5), (0, 5), (6, 4), (0, 4), (6, 0), (0, 0)],
    );
    // Edges: left on an odd row, right on an even row, top and bottom.
    assert_neighbors(
        t,
        size,
        (0, 3),
        &[(6, 3), (1, 3), (0, 2), (1, 2), (0, 4), (1, 4)],
    );
    assert_neighbors(
        t,
        size,
        (6, 2),
        &[(5, 2), (0, 2), (5, 1), (6, 1), (5, 3), (6, 3)],
    );
    assert_neighbors(
        t,
        size,
        (3, 0),
        &[(2, 0), (4, 0), (2, 5), (3, 5), (2, 1), (3, 1)],
    );
    assert_neighbors(
        t,
        size,
        (3, 5),
        &[(2, 5), (4, 5), (3, 4), (4, 4), (3, 0), (4, 0)],
    );
}

#[test]
fn triangular_12_neighbors_by_hand() {
    let t = Tiling::Triangular12;
    let size = (8, 6);
    #[rustfmt::skip]
    let cases = [
        // Up triangle: 3 above, 5 below.
        ((4, 2), [(2, 2), (3, 2), (5, 2), (6, 2), (3, 1), (4, 1), (5, 1),
                  (2, 3), (3, 3), (4, 3), (5, 3), (6, 3)]),
        // Down triangle: 5 above, 3 below.
        ((3, 2), [(1, 2), (2, 2), (4, 2), (5, 2), (1, 1), (2, 1), (3, 1), (4, 1), (5, 1),
                  (2, 3), (3, 3), (4, 3)]),
        // Up and down in an odd row.
        ((3, 3), [(1, 3), (2, 3), (4, 3), (5, 3), (2, 2), (3, 2), (4, 2),
                  (1, 4), (2, 4), (3, 4), (4, 4), (5, 4)]),
        ((4, 3), [(2, 3), (3, 3), (5, 3), (6, 3), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2),
                  (3, 4), (4, 4), (5, 4)]),
        // Corners: up, down, down, up.
        ((0, 0), [(6, 0), (7, 0), (1, 0), (2, 0), (7, 5), (0, 5), (1, 5),
                  (6, 1), (7, 1), (0, 1), (1, 1), (2, 1)]),
        ((7, 0), [(5, 0), (6, 0), (0, 0), (1, 0), (5, 5), (6, 5), (7, 5), (0, 5), (1, 5),
                  (6, 1), (7, 1), (0, 1)]),
        ((0, 5), [(6, 5), (7, 5), (1, 5), (2, 5), (6, 4), (7, 4), (0, 4), (1, 4), (2, 4),
                  (7, 0), (0, 0), (1, 0)]),
        ((7, 5), [(5, 5), (6, 5), (0, 5), (1, 5), (6, 4), (7, 4), (0, 4),
                  (5, 0), (6, 0), (7, 0), (0, 0), (1, 0)]),
    ];
    for (cell, expected) in cases {
        assert_neighbors(t, size, cell, &expected);
    }
    // The second cell in from each side, whose far neighbors wrap.
    assert_neighbors(
        t,
        size,
        (1, 2),
        &[
            (7, 2),
            (0, 2),
            (2, 2),
            (3, 2),
            (7, 1),
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 1),
            (0, 3),
            (1, 3),
            (2, 3),
        ],
    );
    assert_neighbors(
        t,
        size,
        (6, 2),
        &[
            (4, 2),
            (5, 2),
            (7, 2),
            (0, 2),
            (5, 1),
            (6, 1),
            (7, 1),
            (4, 3),
            (5, 3),
            (6, 3),
            (7, 3),
            (0, 3),
        ],
    );
}

#[test]
fn triangular_3_neighbors_by_hand() {
    let t = Tiling::Triangular3;
    let size = (8, 6);
    // Up: the third edge is below; down: above.
    assert_neighbors(t, size, (4, 2), &[(3, 2), (5, 2), (4, 3)]);
    assert_neighbors(t, size, (3, 2), &[(2, 2), (4, 2), (3, 1)]);
    assert_neighbors(t, size, (3, 3), &[(2, 3), (4, 3), (3, 4)]);
    assert_neighbors(t, size, (4, 3), &[(3, 3), (5, 3), (4, 2)]);
    // Corners.
    assert_neighbors(t, size, (0, 0), &[(7, 0), (1, 0), (0, 1)]);
    assert_neighbors(t, size, (7, 0), &[(6, 0), (0, 0), (7, 5)]);
    assert_neighbors(t, size, (0, 5), &[(7, 5), (1, 5), (0, 4)]);
    assert_neighbors(t, size, (7, 5), &[(6, 5), (0, 5), (7, 0)]);
    // Top edge down triangle, bottom edge up triangle.
    assert_neighbors(t, size, (3, 0), &[(2, 0), (4, 0), (3, 5)]);
    assert_neighbors(t, size, (3, 5), &[(2, 5), (4, 5), (3, 0)]);
}

// Tilings and rules

#[test]
fn tiling_ids_names_and_neighbor_counts() {
    let expected = [
        (Tiling::Square, "square", "Square", 8, (false, false)),
        (
            Tiling::Hexagonal,
            "hexagonal",
            "Hexagonal",
            6,
            (false, true),
        ),
        (
            Tiling::Triangular12,
            "triangular-12",
            "Triangular, 12 neighbors",
            12,
            (true, true),
        ),
        (
            Tiling::Triangular3,
            "triangular-3",
            "Triangular, 3 neighbors",
            3,
            (true, true),
        ),
    ];
    assert_eq!(Tiling::ALL, expected.map(|e| e.0));
    for (code, (tiling, id, name, neighbors, even)) in expected.into_iter().enumerate() {
        assert_eq!(tiling as usize, code);
        assert_eq!(tiling.id(), id);
        assert_eq!(Tiling::parse(id), Some(tiling));
        assert_eq!(tiling.name(), name);
        assert_eq!(tiling.neighbors(), neighbors);
        assert_eq!(tiling.wrap_needs_even(), even);
    }
    assert_eq!(Tiling::parse("Square"), None);
    assert_eq!(Tiling::parse("triangular"), None);
}

#[test]
fn every_preset_parses_and_displays() {
    let expected: [(Tiling, &[&str]); 4] = [
        (
            Tiling::Square,
            &["B3/S23", "B36/S23", "B2/S", "B3678/S34678"],
        ),
        (Tiling::Hexagonal, &["B2/S35", "B3/S23"]),
        (Tiling::Triangular12, &["B456/S45", "B4/S456"]),
        (Tiling::Triangular3, &["B2/S013", "B2/S23"]),
    ];
    for (tiling, texts) in expected {
        let presets = tiling.presets();
        assert_eq!(
            presets
                .iter()
                .map(|(_, r)| r.to_string())
                .collect::<Vec<_>>(),
            texts
        );
        assert_eq!(tiling.default_rule(), presets[0].1);
        for &(_, rule) in presets {
            let text = rule.to_string();
            assert_eq!(Rule::parse(&text, tiling), Ok(rule));
            assert_eq!(Rule::parse(&text.to_lowercase(), tiling), Ok(rule));
        }
    }
    assert_eq!(
        Rule::parse(" b3/s23 ", Tiling::Square),
        Ok(Rule {
            birth: 1 << 3,
            survival: 1 << 2 | 1 << 3
        })
    );
    assert_eq!(
        Rule::parse("B/S", Tiling::Square),
        Ok(Rule {
            birth: 0,
            survival: 0
        })
    );
}

#[test]
fn counts_from_10_up_are_written_with_commas() {
    let t12 = Tiling::Triangular12;
    let rule = Rule::parse("b4,10,11/s5,12", t12).unwrap();
    assert_eq!(
        rule,
        Rule {
            birth: 1 << 4 | 1 << 10 | 1 << 11,
            survival: 1 << 5 | 1 << 12
        }
    );
    assert_eq!(rule.to_string(), "B4,10,11/S5,12");
    // Once any count reaches 10, both lists use commas.
    let rule = Rule::parse("B456/S4,5,10", t12).unwrap();
    assert_eq!(rule.to_string(), "B4,5,6/S4,5,10");
    // Without a comma each digit is a count; a lone count from 10 up ends
    // with a comma.
    assert_eq!(
        Rule::parse("B4/S12", t12).unwrap(),
        Rule {
            birth: 1 << 4,
            survival: 1 << 1 | 1 << 2
        }
    );
    let rule = Rule::parse("B4/S12,", t12).unwrap();
    assert_eq!(
        rule,
        Rule {
            birth: 1 << 4,
            survival: 1 << 12
        }
    );
    assert_eq!(rule.to_string(), "B4/S12,");
    // A comma list of single digits reads as the same rule as the digit run.
    assert_eq!(
        Rule::parse("B3/S2,3", Tiling::Square),
        Rule::parse("B3/S23", Tiling::Square)
    );
    assert_eq!(
        Rule::parse("B3/S2,3", Tiling::Square).unwrap().to_string(),
        "B3/S23"
    );
    for preset in Tiling::ALL
        .iter()
        .flat_map(|t| t.presets().iter().map(move |p| (*t, p.1)))
    {
        let (tiling, rule) = preset;
        assert_eq!(Rule::parse(&rule.to_string(), tiling), Ok(rule));
    }
}

#[test]
fn parse_refuses_counts_the_grid_lacks_birth_on_0_and_other_text() {
    let count = |count, neighbors| Err(RuleError::Count { count, neighbors });
    assert_eq!(Rule::parse("B9/S", Tiling::Square), count(9, 8));
    assert_eq!(Rule::parse("B2/S37", Tiling::Hexagonal), count(7, 6));
    assert_eq!(Rule::parse("B2/S4", Tiling::Triangular3), count(4, 3));
    assert_eq!(Rule::parse("B2/S5,13", Tiling::Triangular12), count(13, 12));
    assert_eq!(Rule::parse("B3,10/S", Tiling::Square), count(10, 8));
    assert!(Rule::parse("B2/S3", Tiling::Triangular3).is_ok());
    assert!(Rule::parse("B2/S12,", Tiling::Triangular12).is_ok());
    for tiling in Tiling::ALL {
        assert_eq!(Rule::parse("B03/S23", tiling), Err(RuleError::BirthOnZero));
    }
    for text in [
        "", "B3S23", "3/23", "B3/23", "S23/B3", "B3/S2x", "B3 /S23", "B3/S23/", "B3/Sc",
        "B3/S2,,3", "B3/S,", "B3/S+2,3", "B3/S2, 3",
    ] {
        assert_eq!(
            Rule::parse(text, Tiling::Square),
            Err(RuleError::Notation(text.trim().to_string())),
            "{text:?}"
        );
    }
    assert_eq!(
        RuleError::Count {
            count: 7,
            neighbors: 6
        }
        .to_string(),
        "a cell on this grid has 6 neighbors, so a rule cannot count 7"
    );
}

#[test]
fn new_starts_under_the_default_rule_and_set_rule_changes_it() {
    for tiling in Tiling::ALL {
        let u = Universe::new(10, 10, Edges::Wrap, tiling).unwrap();
        assert_eq!((u.tiling(), u.rule()), (tiling, tiling.default_rule()));
    }
    let mut u = Universe::new(8, 6, Edges::Wrap, Tiling::Triangular3).unwrap();
    let b2s23 = Rule::parse("B2/S23", Tiling::Triangular3).unwrap();
    u.set_rule(b2s23).unwrap();
    assert_eq!(u.rule(), b2s23);
    // A lone triangle survives under B2/S013 and dies under B2/S23.
    u.set(2, 2, true);
    u.step();
    assert_eq!(u.live_count(), 0);
    u.set(2, 2, true);
    u.set_rule(Tiling::Triangular3.default_rule()).unwrap();
    u.step();
    assert_eq!(alive(&u), [(2, 2)].into());
}

#[test]
fn set_rule_refuses_a_rule_the_grid_cannot_run_and_changes_nothing() {
    let mut u = Universe::new(8, 6, Edges::Wrap, Tiling::Hexagonal).unwrap();
    for rule in [
        Rule {
            birth: 1 << 2,
            survival: 1 << 7,
        },
        Rule {
            birth: 1 << 2 | 1 << 15,
            survival: 0,
        },
        Rule {
            birth: 1 | 1 << 2,
            survival: 0,
        },
    ] {
        assert!(u.set_rule(rule).is_err(), "{rule}");
        assert_eq!(u.rule(), Tiling::Hexagonal.default_rule());
    }
    assert_eq!(
        u.set_rule(Rule {
            birth: 1,
            survival: 0
        }),
        Err(RuleError::BirthOnZero)
    );
}

// Sizes and edges

#[test]
fn new_refuses_odd_sides_with_wrapping_edges_per_grid() {
    // Whether each size may wrap: (5, 6), (6, 5), (5, 5), (6, 6).
    let expected = [
        (Tiling::Square, [true, true, true, true]),
        (Tiling::Hexagonal, [true, false, false, true]),
        (Tiling::Triangular12, [false, false, false, true]),
        (Tiling::Triangular3, [false, false, false, true]),
    ];
    for (tiling, wraps) in expected {
        for ((width, height), wraps) in [(5, 6), (6, 5), (5, 5), (6, 6)].into_iter().zip(wraps) {
            let result = Universe::new(width, height, Edges::Wrap, tiling);
            if wraps {
                assert!(result.is_ok(), "{tiling:?} {width} x {height}");
            } else {
                assert_eq!(
                    result.err(),
                    Some(SizeError::OddWrap {
                        tiling,
                        width,
                        height
                    }),
                    "{tiling:?} {width} x {height}"
                );
            }
            assert!(Universe::new(width, height, Edges::Dead, tiling).is_ok());
        }
        assert_eq!(
            Universe::new(2, 6, Edges::Dead, tiling).err(),
            Some(SizeError::Range {
                width: 2,
                height: 6
            })
        );
    }
    assert_eq!(
        SizeError::OddWrap {
            tiling: Tiling::Hexagonal,
            width: 6,
            height: 5
        }
        .to_string(),
        "a wrapping universe on this grid needs an even height, not 6 x 5"
    );
    assert_eq!(
        SizeError::OddWrap {
            tiling: Tiling::Triangular3,
            width: 5,
            height: 6
        }
        .to_string(),
        "a wrapping universe on this grid needs an even width and an even height, not 5 x 6"
    );
}

#[test]
fn set_edges_refuses_wrapping_on_an_odd_side_and_changes_nothing() {
    let mut u = Universe::new(6, 5, Edges::Dead, Tiling::Hexagonal).unwrap();
    u.set(1, 1, true);
    u.set(2, 1, true);
    assert_eq!(
        u.set_edges(Edges::Wrap),
        Err(SizeError::OddWrap {
            tiling: Tiling::Hexagonal,
            width: 6,
            height: 5
        })
    );
    assert_eq!(u.edges(), Edges::Dead);
    assert_eq!(alive(&u), [(1, 1), (2, 1)].into());

    let mut u = Universe::new(6, 6, Edges::Wrap, Tiling::Triangular12).unwrap();
    assert_eq!(u.set_edges(Edges::Dead), Ok(()));
    assert_eq!(u.set_edges(Edges::Wrap), Ok(()));
    assert_eq!(u.edges(), Edges::Wrap);
}

#[test]
fn from_parts_takes_the_tiling_and_rule() {
    let tiling = Tiling::Triangular12;
    let rule = tiling.presets()[1].1;
    let mut cells = Pattern::new(6, 4);
    cells.set(2, 1, true);
    let ages = vec![0u16; 24];
    let u = Universe::from_parts(6, 4, Edges::Wrap, tiling, rule, 5, &cells, &ages).unwrap();
    assert_eq!((u.tiling(), u.rule(), u.live_count()), (tiling, rule, 1));
    let bad = Rule {
        birth: 1 << 13,
        survival: 0,
    };
    assert_eq!(
        Universe::from_parts(6, 4, Edges::Wrap, tiling, bad, 5, &cells, &ages).err(),
        Some(PartsError::Rule(RuleError::Count {
            count: 13,
            neighbors: 12
        }))
    );
    let cells = Pattern::new(5, 4);
    let ages = vec![0u16; 20];
    assert!(matches!(
        Universe::from_parts(5, 4, Edges::Wrap, tiling, rule, 5, &cells, &ages),
        Err(PartsError::Size(SizeError::OddWrap { .. }))
    ));
}

#[test]
fn clone_and_clone_from_keep_the_tiling_and_rule() {
    let mut source = Universe::new(8, 6, Edges::Wrap, Tiling::Triangular12).unwrap();
    source
        .set_rule(Tiling::Triangular12.presets()[1].1)
        .unwrap();
    let copy = source.clone();
    assert_eq!(
        (copy.tiling(), copy.rule()),
        (source.tiling(), source.rule())
    );
    let mut target = Universe::new(8, 6, Edges::Wrap, Tiling::Square).unwrap();
    target.clone_from(&source);
    assert_eq!(
        (target.tiling(), target.rule()),
        (source.tiling(), source.rule())
    );
}

// The WP1 patterns, each under its grid's default rule

enum Kind {
    StillLife,
    Oscillator { period: u32 },
    Spaceship { period: u32, dx: i32, dy: i32 },
}

/// A copy of each `src-tauri/patterns/pending/*.toml`: name, tiling, offset,
/// kind and RLE body.
#[rustfmt::skip]
const PATTERNS: [(&str, Tiling, u32, Kind, &str); 19] = [
    ("hex-bays-glider", Tiling::Hexagonal, 0, Kind::Spaceship { period: 5, dx: -1, dy: 0 }, "3bo$bo$o2bobo2$o2bobo$bo$3bo!"),
    ("hex-flip-flop", Tiling::Hexagonal, 0, Kind::Oscillator { period: 2 }, "2o!"),
    ("hex-ring", Tiling::Hexagonal, 0, Kind::Oscillator { period: 3 }, "b2o$obo$b2o!"),
    ("hex-skipper", Tiling::Hexagonal, 0, Kind::Oscillator { period: 4 }, "o2bo$bo!"),
    ("hex-slow-turner", Tiling::Hexagonal, 0, Kind::Oscillator { period: 12 }, "2bo$o$2bo$2bobo!"),
    ("hex-triad", Tiling::Hexagonal, 0, Kind::Oscillator { period: 2 }, "o$bo$o!"),
    ("tri12-bays-glider", Tiling::Triangular12, 0, Kind::Spaceship { period: 8, dx: 2, dy: 0 }, "2b3o$2o2bobo$2bobo!"),
    ("tri12-flicker", Tiling::Triangular12, 1, Kind::Oscillator { period: 2 }, "bo$2o$obo!"),
    ("tri12-hexagon", Tiling::Triangular12, 0, Kind::StillLife, "3o$3o!"),
    ("tri12-pinwheel", Tiling::Triangular12, 1, Kind::Oscillator { period: 6 }, "bo$obo$4o!"),
    ("tri12-ring", Tiling::Triangular12, 0, Kind::StillLife, "3o$obo$obo$3o!"),
    ("tri12-spinner", Tiling::Triangular12, 1, Kind::Oscillator { period: 3 }, "3o$3o!"),
    ("tri3-chevron", Tiling::Triangular3, 1, Kind::StillLife, "bo$obo!"),
    ("tri3-four-step", Tiling::Triangular3, 1, Kind::Oscillator { period: 4 }, "o$4o!"),
    ("tri3-hook", Tiling::Triangular3, 0, Kind::StillLife, "2bo$obo$bo!"),
    ("tri3-pair", Tiling::Triangular3, 0, Kind::StillLife, "o$o!"),
    ("tri3-six-step", Tiling::Triangular3, 0, Kind::Oscillator { period: 6 }, "o$b2obo$ob2o!"),
    ("tri3-triple-step", Tiling::Triangular3, 0, Kind::Oscillator { period: 3 }, "2bo$3o$3o$2bo!"),
    ("tri3-twins", Tiling::Triangular3, 0, Kind::Oscillator { period: 2 }, "obo!"),
];

/// The live cells of an RLE body.
fn decode_rle(body: &str) -> Vec<(u32, u32)> {
    let (mut x, mut y, mut run) = (0, 0, 0);
    let mut cells = Vec::new();
    for c in body.chars() {
        let n = run.max(1);
        match c {
            '0'..='9' => {
                run = 10 * run + c.to_digit(10).unwrap();
                continue;
            }
            'b' => x += n,
            'o' => {
                cells.extend((x..x + n).map(|x| (x, y)));
                x += n;
            }
            '$' => (x, y) = (0, y + n),
            '!' => break,
            _ => panic!("unexpected {c:?} in RLE"),
        }
        run = 0;
    }
    cells
}

#[test]
fn every_wp1_pattern_keeps_its_kind_under_its_grids_default_rule() {
    for (name, tiling, offset, kind, body) in PATTERNS {
        let cells = decode_rle(body);
        let period = match kind {
            Kind::StillLife => 1,
            Kind::Oscillator { period } | Kind::Spaceship { period, .. } => period,
        };
        // Place the pattern where the grid's alignment matches its offset,
        // far enough from the dead edges for one period.
        let margin = 2 * period + 4;
        let (mut x0, mut y0) = (margin, margin);
        match tiling {
            Tiling::Hexagonal if y0 % 2 != offset => y0 += 1,
            Tiling::Triangular12 | Tiling::Triangular3 if (x0 + y0) % 2 != offset => x0 += 1,
            _ => {}
        }
        let size = 2 * margin + 10;
        let mut u = Universe::new(size, size, Edges::Dead, tiling).unwrap();
        let start: Cells = cells.iter().map(|&(x, y)| (x0 + x, y0 + y)).collect();
        for &(x, y) in &start {
            u.set(x, y, true);
        }
        for generation in 1..period {
            u.step();
            assert_ne!(
                alive(&u),
                start,
                "{name} repeats at generation {generation}"
            );
        }
        u.step();
        let expected: Cells = match kind {
            Kind::Spaceship { dx, dy, .. } => start
                .iter()
                .map(|&(x, y)| ((x as i32 + dx) as u32, (y as i32 + dy) as u32))
                .collect(),
            _ => start,
        };
        assert_eq!(alive(&u), expected, "{name} after {period} generations");
    }
}
