//! Opening an RLE file as a whole universe.

use glife_engine::{Edges, PasteMode, Tiling, Universe, MAX_SIDE, MIN_SIDE};

use crate::rle::Rle;

/// A universe holding the RLE's pattern, plus notices for the operator. The
/// pattern's own sides are at most 2048, so only a declared grid can be refused.
pub fn universe_from_rle(rle: &Rle) -> Result<(Universe, Vec<String>), String> {
    let mut notices = rle.notices.clone();
    let p = &rle.pattern;
    let sides = MIN_SIDE..=MAX_SIDE;

    let declared = match rle.bounded {
        Some(b) if sides.contains(&b.width) && sides.contains(&b.height) => Some(b),
        Some(b) => {
            notices.push(format!(
                "The file declares a {} x {} grid, outside {MIN_SIDE} to {MAX_SIDE} per side; \
                 it was opened as an ordinary pattern.",
                b.width, b.height
            ));
            None
        }
        None => None,
    };

    let (width, height, edges) = match declared {
        Some(b) => {
            if p.width() > b.width || p.height() > b.height {
                return Err(format!(
                    "The pattern is {} x {} but the file's grid is {} x {}.",
                    p.width(),
                    p.height(),
                    b.width,
                    b.height
                ));
            }
            let edges = match b.kind {
                'T' => Edges::Wrap,
                'P' => Edges::Dead,
                kind => {
                    notices.push(format!(
                        "The file's grid type {kind} is not one glife has; it opened with dead edges."
                    ));
                    Edges::Dead
                }
            };
            (b.width, b.height, edges)
        }
        None => {
            let side = |n: u32| (3 * n).clamp(256, MAX_SIDE);
            (side(p.width()), side(p.height()), Edges::Dead)
        }
    };

    let mut u = Universe::new(width, height, edges, Tiling::Square).map_err(|e| e.to_string())?;
    let (ox, oy) = ((width - p.width()) / 2, (height - p.height()) / 2);
    u.place(p, ox as i64, oy as i64, PasteMode::Or).map_err(|e| e.to_string())?;
    Ok((u, notices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rle::parse;

    fn open(text: &str) -> Result<(Universe, Vec<String>), String> {
        universe_from_rle(&parse(text)?)
    }

    #[test]
    fn bounded_torus_wraps_at_its_size() {
        let (u, notices) = open("x = 3, y = 3, rule = B3/S23:T10,8\nbob$2bo$3o!\n").unwrap();
        assert_eq!((u.width(), u.height(), u.edges()), (10, 8, Edges::Wrap));
        assert_eq!((u.generation(), u.live_count()), (0, 5));
        assert!(notices.is_empty());
    }

    #[test]
    fn bounded_plane_has_dead_edges() {
        let (u, notices) = open("x = 3, y = 1, rule = B3/S23:P20,30\n3o!\n").unwrap();
        assert_eq!((u.width(), u.height(), u.edges()), (20, 30, Edges::Dead));
        assert!(notices.is_empty());
    }

    #[test]
    fn klein_bottle_opens_dead_with_a_notice() {
        let (u, notices) = open("x = 3, y = 1, rule = B3/S23:K20,30\n3o!\n").unwrap();
        assert_eq!((u.width(), u.height(), u.edges()), (20, 30, Edges::Dead));
        assert_eq!(notices.len(), 1);
    }

    #[test]
    fn declared_grid_out_of_range_opens_as_a_pattern() {
        let (u, notices) = open("x = 3, y = 1, rule = B3/S23:T2,30\n3o!\n").unwrap();
        assert_eq!((u.width(), u.height(), u.edges()), (256, 256, Edges::Dead));
        assert_eq!(notices.len(), 1);
    }

    #[test]
    fn pattern_bigger_than_its_declared_grid_is_refused() {
        assert!(open("x = 5, y = 1, rule = B3/S23:T4,4\n5o!\n").is_err());
    }

    #[test]
    fn small_pattern_opens_centered_at_256() {
        let (u, _) = open("x = 10, y = 10\n10o$10o$10o$10o$10o$10o$10o$10o$10o$10o!\n").unwrap();
        assert_eq!((u.width(), u.height(), u.edges()), (256, 256, Edges::Dead));
        assert_eq!(u.live_count(), 100);
        assert!(u.get(123, 123) && u.get(132, 132));
        assert!(!u.get(122, 123) && !u.get(133, 132));
        assert_eq!(u.generation(), 0);
        assert_eq!(u.age(123, 123), 0);
    }

    #[test]
    fn wide_pattern_gets_a_wide_grid() {
        let (u, _) = open("x = 1000, y = 10\n1000o!\n").unwrap();
        assert_eq!((u.width(), u.height()), (2048, 256));
    }

    #[test]
    fn side_of_2049_is_refused() {
        assert!(open("x = 2049, y = 10\n!\n").is_err());
    }
}
