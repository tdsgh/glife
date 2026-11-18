//! Patterns on every grid: offsets, flips, alignment and placing.

use std::collections::BTreeSet;

use super::super::{alive, assert_storage_consistent, full_state, live_state, Cells};
use super::{decode_rle, Draws, Kind, PATTERNS};
use crate::{Axis, Edges, PasteMode, Pattern, Rect, Tiling, Universe};

const AXES: [Axis; 2] = [Axis::Horizontal, Axis::Vertical];

/// The offset a pattern needs with its top-left cell at `(x, y)`, from the
/// coordinate rules.
fn offset_at(tiling: Tiling, x: i64, y: i64) -> bool {
    match tiling {
        Tiling::Square => false,
        Tiling::Hexagonal => y.rem_euclid(2) == 1,
        Tiling::Triangular12 | Tiling::Triangular3 => (x + y).rem_euclid(2) == 1,
    }
}

fn pattern_of(tiling: Tiling, offset: bool, (w, h): (u32, u32), cells: &[(u32, u32)]) -> Pattern {
    let mut p = Pattern::new(w, h, tiling, offset);
    for &(x, y) in cells {
        p.set(x, y, true);
    }
    p
}

fn cells_of(p: &Pattern) -> Cells {
    let mut cells = Cells::new();
    for y in 0..p.height() {
        for x in 0..p.width() {
            if p.get(x, y) {
                cells.insert((x, y));
            }
        }
    }
    cells
}

/// A WP1 pattern at its own offset, in a box that just holds it.
fn wp1_pattern(tiling: Tiling, offset: u32, body: &str) -> Pattern {
    let cells = decode_rle(body);
    let w = cells.iter().map(|c| c.0).max().unwrap() + 1;
    let h = cells.iter().map(|c| c.1).max().unwrap() + 1;
    pattern_of(tiling, offset == 1, (w, h), &cells)
}

/// A random `w` by `h` pattern on `tiling`, with a random offset.
fn random_pattern(draws: &mut Draws, tiling: Tiling, w: u32, h: u32) -> Pattern {
    let offset = tiling != Tiling::Square && draws.next() % 2 == 1;
    let mut p = Pattern::new(w, h, tiling, offset);
    for y in 0..h {
        for x in 0..w {
            p.set(x, y, draws.next() % 2 == 1);
        }
    }
    p
}

/// A shape's cells, laid out with `offset`, up to a translation that keeps
/// every cell's shape and orientation: hexagons by their centers in half
/// cells, triangles by their positions and the orientation of the top-left
/// one. Cells of a universe have offset `false`.
fn shape(tiling: Tiling, offset: bool, cells: &Cells) -> (BTreeSet<(i64, i64)>, bool) {
    let points: Vec<(i64, i64)> = cells
        .iter()
        .map(|&(x, y)| match tiling {
            Tiling::Hexagonal => (2 * x as i64 + ((y % 2 == 1) != offset) as i64, y as i64),
            _ => (x as i64, y as i64),
        })
        .collect();
    let min_x = points.iter().map(|p| p.0).min().unwrap_or(0);
    let min_y = points.iter().map(|p| p.1).min().unwrap_or(0);
    let triangles = matches!(tiling, Tiling::Triangular12 | Tiling::Triangular3);
    let down = triangles && ((min_x + min_y) % 2 == 1) != offset;
    (
        points
            .iter()
            .map(|&(x, y)| (x - min_x, y - min_y))
            .collect(),
        down,
    )
}

/// Runs `p` for `generations` in a dead-edged universe with an empty margin
/// around it wide enough that nothing reaches the edge, and returns the
/// same window afterwards.
fn run(p: &Pattern, generations: u32) -> Pattern {
    let margin = 2 * generations + 4;
    let mut padded = Pattern::new(
        p.width() + 2 * margin,
        p.height() + 2 * margin,
        p.tiling(),
        p.offset(),
    );
    for (x, y) in cells_of(p) {
        padded.set(x + margin, y + margin, true);
    }
    // A top-left cell where the padded pattern lines up as it is.
    let (x0, y0): (u32, u32) = [(0, 0), (1, 0), (0, 1)]
        .into_iter()
        .find(|&(x, y)| offset_at(p.tiling(), x as i64, y as i64) == p.offset())
        .unwrap();
    let window = Rect {
        x: x0,
        y: y0,
        w: padded.width(),
        h: padded.height(),
    };
    let mut u = Universe::new(window.w + 1, window.h + 1, Edges::Dead, p.tiling()).unwrap();
    u.place(&padded, x0 as i64, y0 as i64, PasteMode::Or)
        .unwrap();
    assert_eq!(u.live_count(), p.live_count(), "placed as it is");
    for _ in 0..generations {
        u.step();
    }
    u.extract(window)
}

// Offsets and the flip table

#[test]
fn extract_takes_the_offset_of_the_rectangles_origin() {
    for tiling in Tiling::ALL {
        let u = Universe::new(10, 10, Edges::Dead, tiling).unwrap();
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1), (4, 7), (3, 6)] {
            let p = u.extract(Rect { x, y, w: 3, h: 2 });
            assert_eq!(p.tiling(), tiling);
            assert_eq!(
                p.offset(),
                offset_at(tiling, x as i64, y as i64),
                "{tiling:?} at {x},{y}"
            );
        }
    }
}

#[test]
#[should_panic(expected = "square pattern")]
fn a_square_pattern_has_no_offset() {
    Pattern::new(2, 2, Tiling::Square, true);
}

#[test]
fn flips_follow_the_table_by_hand() {
    // Hexagonal, row 0 unshifted: unshifted rows x -> w - x, shifted rows
    // x -> w - 1 - x, one column wider.
    let hex = pattern_of(Tiling::Hexagonal, false, (3, 2), &[(0, 0), (0, 1), (2, 1)]);
    let h = hex.flipped(Axis::Horizontal);
    assert_eq!((h.width(), h.height(), h.offset()), (4, 2, false));
    assert_eq!(cells_of(&h), [(3, 0), (2, 1), (0, 1)].into());
    // Vertical: rows reverse; the offset flips with an even height.
    let v = hex.flipped(Axis::Vertical);
    assert_eq!((v.width(), v.height(), v.offset()), (3, 2, true));
    assert_eq!(cells_of(&v), [(0, 1), (0, 0), (2, 0)].into());
    let hex3 = pattern_of(Tiling::Hexagonal, true, (2, 3), &[(1, 0)]);
    let v = hex3.flipped(Axis::Vertical);
    assert_eq!((v.offset(), cells_of(&v)), (true, [(1, 2)].into()));
    // Triangular: x -> w - 1 - x, offset flips with an even width;
    // y -> h - 1 - y, offset flips with an odd height.
    for tiling in [Tiling::Triangular12, Tiling::Triangular3] {
        let tri = pattern_of(tiling, false, (4, 3), &[(0, 0), (3, 2)]);
        let h = tri.flipped(Axis::Horizontal);
        assert_eq!(
            (h.width(), h.offset(), cells_of(&h)),
            (4, true, [(3, 0), (0, 2)].into())
        );
        let v = tri.flipped(Axis::Vertical);
        assert_eq!(
            (v.height(), v.offset(), cells_of(&v)),
            (3, true, [(0, 2), (3, 0)].into())
        );
        let odd = pattern_of(tiling, true, (3, 2), &[(0, 0)]);
        assert!(odd.flipped(Axis::Horizontal).offset());
        assert!(odd.flipped(Axis::Vertical).offset());
    }
}

#[test]
fn flipping_twice_gives_back_the_same_cells_and_offset_on_every_grid() {
    let mut draws = Draws(21);
    for tiling in Tiling::ALL {
        for _ in 0..40 {
            let (w, h) = (draws.range(1, 12), draws.range(1, 12));
            let p = random_pattern(&mut draws, tiling, w, h);
            for axis in AXES {
                let twice = p.flipped(axis).flipped(axis);
                assert_eq!(twice.offset(), p.offset(), "{tiling:?} {axis:?}");
                if (tiling, axis) == (Tiling::Hexagonal, Axis::Horizontal) {
                    // Each flip adds a column: the cells come back one
                    // column right, in a pattern two columns wider.
                    assert_eq!((twice.width(), twice.height()), (w + 2, h));
                    let moved: Cells = cells_of(&p).iter().map(|&(x, y)| (x + 1, y)).collect();
                    assert_eq!(cells_of(&twice), moved);
                } else {
                    assert_eq!(twice, p, "{tiling:?} {axis:?}");
                }
            }
        }
    }
}

#[test]
fn a_flipped_pattern_runs_as_the_mirror_of_the_original() {
    let mut cases: Vec<(String, Pattern, u32)> = PATTERNS
        .iter()
        .map(|(name, tiling, offset, kind, body)| {
            let generations = match *kind {
                Kind::StillLife => 2,
                Kind::Oscillator { period } | Kind::Spaceship { period, .. } => period,
            };
            (
                name.to_string(),
                wp1_pattern(*tiling, *offset, body),
                generations,
            )
        })
        .collect();
    let mut draws = Draws(8);
    for tiling in Tiling::ALL {
        for i in 0..6 {
            let (w, h) = (draws.range(3, 7), draws.range(3, 7));
            let p = random_pattern(&mut draws, tiling, w, h);
            cases.push((format!("{tiling:?} soup {i}"), p, 5));
        }
    }
    for (name, p, generations) in cases {
        let after = run(&p, generations);
        for axis in AXES {
            let flipped = p.flipped(axis);
            assert_eq!(
                run(&flipped, generations),
                after.flipped(axis),
                "{name}, {axis:?}"
            );
        }
    }
}

// Alignment and placing

#[test]
fn aligned_realigns_hexagonal_rows_and_pads_triangles() {
    let hex = pattern_of(Tiling::Hexagonal, false, (2, 3), &[(0, 0), (1, 1), (1, 2)]);
    assert_eq!(hex.aligned(false), hex);
    // Row 1 was shifted and becomes unshifted, so it moves one cell right.
    let a = hex.aligned(true);
    assert_eq!((a.width(), a.height(), a.offset()), (3, 3, true));
    assert_eq!(cells_of(&a), [(0, 0), (2, 1), (1, 2)].into());
    for tiling in [Tiling::Triangular12, Tiling::Triangular3] {
        let tri = pattern_of(tiling, true, (2, 2), &[(0, 0), (1, 1)]);
        assert_eq!(tri.aligned(true), tri);
        let a = tri.aligned(false);
        assert_eq!((a.width(), a.offset()), (3, false));
        assert_eq!(cells_of(&a), [(1, 0), (2, 1)].into());
    }
    let square = pattern_of(Tiling::Square, false, (2, 2), &[(0, 0)]);
    assert_eq!(square.aligned(false), square);
}

#[test]
fn an_extracted_pattern_placed_back_at_its_origin_is_unchanged() {
    let mut draws = Draws(5);
    for tiling in Tiling::ALL {
        for edges in [Edges::Wrap, Edges::Dead] {
            let mut u = Universe::new(14, 12, edges, tiling).unwrap();
            u.random_fill(0.4, draws.next());
            for _ in 0..30 {
                let (w, h) = (draws.range(1, 14), draws.range(1, 12));
                let (x, y) = (draws.range(0, 14 - w), draws.range(0, 12 - h));
                let rect = Rect { x, y, w, h };
                let p = u.extract(rect);
                let (ax, ay) = ((x + w / 2) as i64, (y + h / 2) as i64);
                assert_eq!(p.placement(ax, ay), (p.clone(), x as i64, y as i64));
                let mut v = Universe::new(14, 12, edges, tiling).unwrap();
                v.place(&p, x as i64, y as i64, PasteMode::Copy).unwrap();
                assert_eq!(v.extract(rect), p, "{tiling:?} {edges:?} {rect:?}");
                assert_eq!(v.live_count(), p.live_count());
            }
        }
    }
}

#[test]
fn placement_is_what_place_stamps_and_lines_up_with_the_grid() {
    let mut draws = Draws(13);
    for tiling in Tiling::ALL {
        for edges in [Edges::Wrap, Edges::Dead] {
            let mut u = Universe::new(12, 10, edges, tiling).unwrap();
            u.random_fill(0.3, draws.next());
            u.step();
            for _ in 0..60 {
                let (w, h) = (draws.range(1, 6), draws.range(1, 6));
                let p = random_pattern(&mut draws, tiling, w, h);
                let (ax, ay) = (
                    draws.range(0, 30) as i64 - 10,
                    draws.range(0, 26) as i64 - 8,
                );
                let (q, tx, ty) = p.placement(ax, ay);
                let (x, y) = (ax - (p.width() / 2) as i64, ay - (p.height() / 2) as i64);
                let case = format!(
                    "{tiling:?} {edges:?} {}x{} offset {} at {ax},{ay}",
                    p.width(),
                    p.height(),
                    p.offset()
                );
                assert_eq!(q.offset(), offset_at(tiling, tx, ty), "{case}");
                assert_eq!(q.tiling(), tiling);
                // Hexagonal patterns are realigned where they are; triangular
                // ones move one cell right.
                let expected = if p.offset() == offset_at(tiling, x, y) {
                    (p.clone(), x, y)
                } else if tiling == Tiling::Hexagonal {
                    (p.aligned(!p.offset()), x, y)
                } else {
                    (p.clone(), x + 1, y)
                };
                assert_eq!((q.clone(), tx, ty), expected, "{case}");
                // The same shape as the pattern.
                assert_eq!(
                    shape(tiling, q.offset(), &cells_of(&q)),
                    shape(tiling, p.offset(), &cells_of(&p)),
                    "{case}"
                );
                // place stamps it, and touches nothing outside its footprint.
                let mut a = u.clone();
                a.place(&p, x, y, PasteMode::Copy).unwrap();
                let mut b = u.clone();
                b.place(&q, tx, ty, PasteMode::Copy).unwrap();
                assert_eq!(full_state(&a), full_state(&b), "{case}");
                let mut inside = Cells::new();
                for r in u.footprint(q.width(), q.height(), tx, ty) {
                    for cy in r.y..r.y + r.h {
                        for cx in r.x..r.x + r.w {
                            inside.insert((cx, cy));
                        }
                    }
                }
                let (before, after) = (full_state(&u), full_state(&a));
                for cy in 0..10 {
                    for cx in 0..12 {
                        let i = (cy * 12 + cx) as usize;
                        if !inside.contains(&(cx, cy)) {
                            assert_eq!(after[i], before[i], "{case}: ({cx}, {cy}) outside");
                        }
                    }
                }
                assert_storage_consistent(&a);
            }
        }
    }
}

#[test]
fn a_pattern_placed_at_the_other_parity_keeps_its_shape_and_runs_true() {
    for (name, tiling, offset, kind, body) in PATTERNS {
        let p = wp1_pattern(tiling, offset, body);
        let period = match kind {
            Kind::StillLife => 1,
            Kind::Oscillator { period } | Kind::Spaceship { period, .. } => period,
        };
        let size = p.width() + 4 * period + 12;
        let centre = (size / 2) as i64;
        let mut starts = Vec::new();
        // An anchor where the pattern lines up as it is, then one of the other
        // parity: the next row on the hexagonal grid, the next cell on the
        // triangular grids.
        let next = |(ax, ay): (i64, i64)| match tiling {
            Tiling::Hexagonal => (ax, ay + 1),
            _ => (ax + 1, ay),
        };
        let top_left =
            |(ax, ay): (i64, i64)| (ax - (p.width() / 2) as i64, ay - (p.height() / 2) as i64);
        let mut anchor = (centre, centre);
        let (x, y) = top_left(anchor);
        if offset_at(tiling, x, y) != p.offset() {
            anchor = next(anchor);
        }
        for other in [false, true] {
            let at = if other { next(anchor) } else { anchor };
            let (x, y) = top_left(at);
            assert_eq!(offset_at(tiling, x, y) != p.offset(), other, "{name}");
            let mut u = Universe::new(size, size, Edges::Dead, tiling).unwrap();
            u.place(&p, x, y, PasteMode::Or).unwrap();
            let start = alive(&u);
            assert_eq!(start.len() as u32, p.live_count(), "{name}");
            for _ in 0..period {
                u.step();
            }
            let expected: Cells = match kind {
                Kind::Spaceship { dx, dy, .. } => start
                    .iter()
                    .map(|&(x, y)| ((x as i32 + dx) as u32, (y as i32 + dy) as u32))
                    .collect(),
                _ => start.clone(),
            };
            assert_eq!(alive(&u), expected, "{name}, other parity {other}");
            starts.push(shape(tiling, false, &start));
        }
        assert_eq!(
            starts[0], starts[1],
            "{name}: the same shape at both parities"
        );
    }
}

#[test]
fn place_refuses_a_pattern_of_another_tiling() {
    for grid in Tiling::ALL {
        for tiling in Tiling::ALL.into_iter().filter(|&t| t != grid) {
            let mut u = Universe::new(10, 10, Edges::Wrap, grid).unwrap();
            let p = pattern_of(tiling, false, (2, 2), &[(0, 0), (1, 1)]);
            let err = u.place(&p, 3, 3, PasteMode::Or).unwrap_err();
            assert_eq!((err.pattern_tiling, err.grid_tiling), (tiling, grid));
            let message = err.to_string();
            assert!(
                message.contains(tiling.id()) && message.contains(grid.id()),
                "{message}"
            );
            assert_eq!(u.live_count(), 0);
        }
    }
}

#[test]
fn place_refuses_a_pattern_too_wide_once_realigned() {
    let mut u = Universe::new(6, 6, Edges::Wrap, Tiling::Hexagonal).unwrap();
    let p = pattern_of(Tiling::Hexagonal, false, (6, 2), &[(0, 0), (5, 1)]);
    assert!(u.place(&p, 0, 0, PasteMode::Or).is_ok());
    let err = u.place(&p, 0, 1, PasteMode::Or).unwrap_err();
    assert_eq!((err.pattern, err.grid), ((7, 2), (6, 6)));
    // A triangular pattern moves instead, and still fits.
    let mut u = Universe::new(6, 6, Edges::Wrap, Tiling::Triangular3).unwrap();
    let p = pattern_of(Tiling::Triangular3, false, (6, 2), &[(0, 0), (5, 1)]);
    u.place(&p, 1, 0, PasteMode::Or).unwrap();
    assert_eq!(alive(&u), [(2, 0), (1, 1)].into());
}

// Flipping a selection

/// Where `flip_rect` sends cell `(x, y)` of `rect`, relative to its origin,
/// from the flip table and the placing rules.
fn mirrored(tiling: Tiling, axis: Axis, rect: Rect, (x, y): (u32, u32)) -> (u32, u32) {
    let (w, h) = (rect.w, rect.h);
    let o = offset_at(tiling, rect.x as i64, rect.y as i64);
    match (tiling, axis) {
        (Tiling::Square, Axis::Horizontal) => (w - 1 - x, y),
        (Tiling::Square, Axis::Vertical) => (x, h - 1 - y),
        (Tiling::Hexagonal, Axis::Horizontal) => {
            let shifted = (y % 2 == 1) != o;
            (if shifted { w - 1 - x } else { w - x }, y)
        }
        (Tiling::Hexagonal, Axis::Vertical) => {
            let y = h - 1 - y;
            // With an even height the flipped offset is !o: its shifted rows
            // become unshifted in place and move one cell right.
            let moves = h % 2 == 0 && (y % 2 == 1) == o;
            (x + moves as u32, y)
        }
        (_, Axis::Horizontal) => (w - 1 - x + (w % 2 == 0) as u32, y),
        (_, Axis::Vertical) => (x + (h % 2 == 1) as u32, h - 1 - y),
    }
}

#[test]
fn flip_rect_moves_ages_and_trail_and_flip_footprint_covers_what_it_writes() {
    let mut draws = Draws(34);
    let (gw, gh) = (12, 10);
    for tiling in Tiling::ALL {
        for edges in [Edges::Wrap, Edges::Dead] {
            let mut u = Universe::new(gw, gh, edges, tiling).unwrap();
            u.random_fill(0.45, draws.next());
            for _ in 0..6 {
                u.step();
            }
            let mut rects = vec![
                Rect {
                    x: 0,
                    y: 0,
                    w: gw,
                    h: gh,
                },
                Rect {
                    x: 0,
                    y: 1,
                    w: gw,
                    h: 5,
                },
                Rect {
                    x: 7,
                    y: 2,
                    w: 5,
                    h: 4,
                },
                Rect {
                    x: 8,
                    y: 3,
                    w: 4,
                    h: 3,
                },
            ];
            for _ in 0..30 {
                let (w, h) = (draws.range(1, gw), draws.range(1, gh));
                rects.push(Rect {
                    x: draws.range(0, gw - w),
                    y: draws.range(0, gh - h),
                    w,
                    h,
                });
            }
            for rect in rects {
                for axis in AXES {
                    let case = format!("{tiling:?} {edges:?} {rect:?} {axis:?}");
                    let mut v = u.clone();
                    v.flip_rect(rect, axis);
                    assert_storage_consistent(&v);
                    let mut written = Cells::new();
                    for dy in 0..rect.h {
                        for dx in 0..rect.w {
                            let (mx, my) = mirrored(tiling, axis, rect, (dx, dy));
                            assert!(mx <= rect.w, "{case}");
                            let to_x = match edges {
                                Edges::Wrap => (rect.x + mx) % gw,
                                Edges::Dead if rect.x + mx < gw => rect.x + mx,
                                Edges::Dead => continue,
                            };
                            let (to, from) = ((to_x, rect.y + my), (rect.x + dx, rect.y + dy));
                            assert!(written.insert(to), "{case}: two cells land on {to:?}");
                            assert_eq!(
                                (v.get(to.0, to.1), v.age(to.0, to.1), v.trail(to.0, to.1)),
                                (
                                    u.get(from.0, from.1),
                                    u.age(from.0, from.1),
                                    u.trail(from.0, from.1)
                                ),
                                "{case}: {from:?} to {to:?}"
                            );
                        }
                    }
                    let mut touched = written.clone();
                    for y in rect.y..rect.y + rect.h {
                        for x in rect.x..rect.x + rect.w {
                            touched.insert((x, y));
                            if !written.contains(&(x, y)) {
                                assert_eq!(
                                    (v.get(x, y), v.age(x, y), v.trail(x, y)),
                                    (false, u.age(x, y), 0),
                                    "{case}: ({x}, {y}) killed"
                                );
                            }
                        }
                    }
                    for y in 0..gh {
                        for x in 0..gw {
                            if !touched.contains(&(x, y)) {
                                assert_eq!(
                                    (v.get(x, y), v.age(x, y), v.trail(x, y)),
                                    (u.get(x, y), u.age(x, y), u.trail(x, y)),
                                    "{case}: ({x}, {y}) untouched"
                                );
                            }
                        }
                    }
                    let footprint = u.flip_footprint(rect, axis);
                    let mut covered = Cells::new();
                    for r in &footprint {
                        assert!(r.x + r.w <= gw && r.y + r.h <= gh, "{case}");
                        for y in r.y..r.y + r.h {
                            for x in r.x..r.x + r.w {
                                assert!(covered.insert((x, y)), "{case}: footprint overlaps");
                            }
                        }
                    }
                    // Rectangles cover every row of the column past the
                    // right side, though only some rows may reach it.
                    assert!(
                        covered.is_superset(&touched),
                        "{case}: footprint misses a cell"
                    );
                    let past = (rect.x + rect.w) % gw;
                    let reaches = touched.iter().any(|&(x, _)| x == past) && rect.w < gw;
                    let mut expected = touched.clone();
                    if reaches {
                        expected.extend((rect.y..rect.y + rect.h).map(|y| (past, y)));
                    }
                    assert_eq!(covered, expected, "{case}: footprint");
                    // A capture of the footprint undoes the flip.
                    let mut undone = u.clone();
                    let capture = undone.capture(&footprint);
                    undone.flip_rect(rect, axis);
                    undone.restore(&capture);
                    assert_eq!(live_state(&undone), live_state(&u), "{case}: undo");
                    assert_eq!(undone.live_count(), u.live_count());
                }
            }
        }
    }
}

#[test]
fn flip_rect_commutes_with_the_step_on_every_grid() {
    let mut draws = Draws(55);
    for tiling in Tiling::ALL {
        for &(_, rule) in tiling.presets() {
            for _ in 0..3 {
                let (gw, gh) = (40, 30);
                let mut u = Universe::new(gw, gh, Edges::Dead, tiling).unwrap();
                u.set_rule(rule).unwrap();
                for y in 12..18 {
                    for x in 16..24 {
                        u.set(x, y, draws.next() % 2 == 1);
                    }
                }
                u.step();
                u.step();
                // The rectangle leaves the last column free for cells that
                // reach past its right side.
                let rect = Rect {
                    x: 0,
                    y: 0,
                    w: gw - 1,
                    h: gh,
                };
                for axis in AXES {
                    let mut a = u.clone();
                    a.flip_rect(rect, axis);
                    let mut b = u.clone();
                    for _ in 0..5 {
                        a.step();
                        b.step();
                    }
                    b.flip_rect(rect, axis);
                    assert_eq!(full_state(&a), full_state(&b), "{tiling:?} {rule} {axis:?}");
                }
            }
        }
    }
}
