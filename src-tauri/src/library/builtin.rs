//! The built-in library: a folder of pattern files, each checked against its
//! stated period and displacement when loaded.

use std::path::Path;

use glife_engine::{Edges, PasteMode, Pattern, Rect, Tiling, Universe};

use super::Problem;
use crate::pattern_file::PatternFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    StillLife,
    Oscillator,
    Spaceship,
    Gun,
    Methuselah,
}

impl Category {
    pub fn parse(text: &str) -> Option<Category> {
        Some(match text {
            "still-life" => Category::StillLife,
            "oscillator" => Category::Oscillator,
            "spaceship" => Category::Spaceship,
            "gun" => Category::Gun,
            "methuselah" => Category::Methuselah,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Category::StillLife => "still-life",
            Category::Oscillator => "oscillator",
            Category::Spaceship => "spaceship",
            Category::Gun => "gun",
            Category::Methuselah => "methuselah",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// The file name without `.toml`.
    pub id: String,
    pub name: String,
    pub category: Category,
    pub description: String,
    pub pattern: Pattern,
    pub live: u32,
    pub period: Option<u32>,
    pub speed: Option<String>,
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// A spaceship's speed for display: `c/4 diagonal`, `c/2 orthogonal`, `2c/5 orthogonal`.
pub fn speed(period: u32, dx: i32, dy: i32) -> String {
    let distance = dx.unsigned_abs().max(dy.unsigned_abs());
    let g = gcd(distance, period).max(1);
    let (n, d) = (distance / g, period / g);
    let ratio = if n == 1 { format!("c/{d}") } else { format!("{n}c/{d}") };
    let direction = if dx == 0 || dy == 0 {
        "orthogonal"
    } else if dx.abs() == dy.abs() {
        "diagonal"
    } else {
        "oblique"
    };
    format!("{ratio} {direction}")
}

/// Loads every `*.toml` in `dir`. A file that cannot be read or fails its
/// check is returned as a problem; the rest load.
pub fn load(dir: &Path) -> (Vec<Entry>, Vec<Problem>) {
    let mut entries = Vec::new();
    let mut problems = Vec::new();
    let mut files: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .collect(),
        Err(e) => {
            problems.push(Problem { file: dir.display().to_string(), reason: format!("Could not read the folder: {e}") });
            return (entries, problems);
        }
    };
    files.sort();
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        match load_one(&path) {
            Ok(entry) => entries.push(entry),
            Err(reason) => problems.push(Problem { file, reason }),
        }
    }
    entries.sort_by(|a, b| {
        (a.category as u8, a.name.to_lowercase()).cmp(&(b.category as u8, b.name.to_lowercase()))
    });
    (entries, problems)
}

fn load_one(path: &Path) -> Result<Entry, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read the file: {e}"))?;
    let file = PatternFile::parse(&text)?;
    let category = file.category.as_deref().ok_or("The file has no category.")?;
    let category = Category::parse(category).ok_or_else(|| format!("Unknown category {category:?}."))?;
    let pattern = file.rle()?.pattern;
    let period = file.period;
    check(category, &pattern, &file)?;
    let speed = (category == Category::Spaceship)
        .then(|| speed(period.unwrap_or(1), file.dx.unwrap_or(0), file.dy.unwrap_or(0)));
    Ok(Entry {
        id: path.file_stem().unwrap().to_string_lossy().into_owned(),
        name: file.name,
        category,
        description: file.description.unwrap_or_default(),
        live: pattern.live_count(),
        pattern,
        period,
        speed,
    })
}

/// Runs the category's check on a dead-edged grid with a margin of
/// `2 x period` around the pattern. A methuselah is not checked.
pub fn check(category: Category, pattern: &Pattern, file: &PatternFile) -> Result<(), String> {
    if category == Category::Methuselah {
        return Ok(());
    }
    if pattern.live_count() == 0 {
        return Err("The pattern has no live cells.".into());
    }
    let period = match (category, file.period) {
        (Category::StillLife, _) => 1,
        (_, Some(p)) if p > 0 => p,
        _ => return Err("The file states no period.".into()),
    };
    let (dx, dy) = match category {
        Category::Spaceship => (
            file.dx.ok_or("The file states no dx.")?,
            file.dy.ok_or("The file states no dy.")?,
        ),
        _ => (0, 0),
    };
    let gliders = match category {
        Category::Gun => file.gliders.filter(|&g| g > 0).ok_or("The file states no gliders per period.")?,
        _ => 0,
    };

    let margin = 2 * period;
    let (w, h) = (pattern.width(), pattern.height());
    let mut u = Universe::new(w + 2 * margin, h + 2 * margin, Edges::Dead, Tiling::Square)
        .map_err(|e| e.to_string())?;
    u.place(pattern, margin as i64, margin as i64, PasteMode::Or).map_err(|e| e.to_string())?;

    // Whether the whole grid holds `pattern` with its top-left cell at `(x, y)` and nothing else.
    let holds_alone = |u: &Universe, x: i64, y: i64| {
        u.live_count() == pattern.live_count() && holds(u, pattern, x, y)
    };

    match category {
        Category::StillLife => {
            u.step();
            if !holds_alone(&u, margin as i64, margin as i64) {
                return Err("The pattern changes in one generation, so it is not a still life.".into());
            }
        }
        Category::Oscillator => {
            for g in 1..=period {
                u.step();
                let same = holds_alone(&u, margin as i64, margin as i64);
                if g < period && same {
                    return Err(format!("The pattern returns after {g} generations, not {period}."));
                }
                if g == period && !same {
                    return Err(format!("The pattern is not back at its start after {period} generations."));
                }
            }
        }
        Category::Spaceship => {
            for _ in 0..period {
                u.step();
            }
            if !holds_alone(&u, margin as i64 + dx as i64, margin as i64 + dy as i64) {
                return Err(format!(
                    "After {period} generations the pattern is not the start shape moved by ({dx}, {dy})."
                ));
            }
        }
        Category::Gun => {
            for g in 1..=4 * period {
                u.step();
                if g == period && !holds(&u, pattern, margin as i64, margin as i64) {
                    return Err(format!("The gun's box is not back at its start after {period} generations."));
                }
            }
            if !holds(&u, pattern, margin as i64, margin as i64) {
                return Err(format!("The gun's box is not back at its start after {} generations.", 4 * period));
            }
            let outside = count_gliders(&u, Rect { x: margin, y: margin, w, h });
            if outside != Some(4 * gliders) {
                return Err(format!(
                    "After {} generations the cells outside the gun do not form {} gliders.",
                    4 * period,
                    4 * gliders
                ));
            }
        }
        Category::Methuselah => unreachable!(),
    }
    Ok(())
}

/// Whether the cells of `u` in `pattern`'s rectangle at `(x, y)` equal `pattern`.
fn holds(u: &Universe, pattern: &Pattern, x: i64, y: i64) -> bool {
    let (w, h) = (pattern.width() as i64, pattern.height() as i64);
    if x < 0 || y < 0 || x + w > u.width() as i64 || y + h > u.height() as i64 {
        return false;
    }
    u.extract(Rect { x: x as u32, y: y as u32, w: w as u32, h: h as u32 }) == *pattern
}

/// The number of live groups outside `skip`, or `None` unless every
/// 8-connected group has the five cells of a glider.
fn count_gliders(u: &Universe, skip: Rect) -> Option<u32> {
    let (w, h) = (u.width() as i64, u.height() as i64);
    let inside = |x: i64, y: i64| {
        x >= skip.x as i64 && x < (skip.x + skip.w) as i64 && y >= skip.y as i64 && y < (skip.y + skip.h) as i64
    };
    let live = |x: i64, y: i64| x >= 0 && y >= 0 && x < w && y < h && !inside(x, y) && u.get(x as u32, y as u32);
    let mut seen = std::collections::HashSet::new();
    let mut groups = 0;
    for y in 0..h {
        for x in 0..w {
            if !live(x, y) || seen.contains(&(x, y)) {
                continue;
            }
            let mut stack = vec![(x, y)];
            seen.insert((x, y));
            let mut size = 0;
            while let Some((cx, cy)) = stack.pop() {
                size += 1;
                for (nx, ny) in (-1..=1).flat_map(|i| (-1..=1).map(move |j| (cx + i, cy + j))) {
                    if live(nx, ny) && seen.insert((nx, ny)) {
                        stack.push((nx, ny));
                    }
                }
            }
            if size != 5 {
                return None;
            }
            groups += 1;
        }
    }
    Some(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("patterns")
    }

    fn file(name: &str, category: &str, period: Option<u32>, body: &str) -> String {
        let mut f = PatternFile::personal(name, None, format!("x = 4, y = 4\n{body}\n"));
        f.created = None;
        f.changed = None;
        f.category = Some(category.into());
        f.period = period;
        f.render().unwrap()
    }

    #[test]
    fn every_shipped_pattern_loads_and_passes_its_check() {
        let (entries, problems) = load(&patterns_dir());
        assert!(problems.is_empty(), "{problems:?}");
        assert!(entries.len() >= 28, "only {} patterns", entries.len());
        for c in [Category::StillLife, Category::Oscillator, Category::Spaceship, Category::Gun, Category::Methuselah] {
            assert!(entries.iter().any(|e| e.category == c), "no {c:?}");
        }
        for e in &entries {
            assert!(!e.description.is_empty(), "{} has no description", e.id);
        }
        let glider = entries.iter().find(|e| e.id == "glider").unwrap();
        assert_eq!(glider.speed.as_deref(), Some("c/4 diagonal"));
        assert_eq!(glider.period, Some(4));
        assert_eq!(glider.live, 5);
    }

    #[test]
    fn broken_files_are_problems_and_the_rest_load() {
        let dir = tempfile::tempdir().unwrap();
        let put = |name: &str, text: &str| std::fs::write(dir.path().join(name), text).unwrap();
        put("block.toml", &file("Block", "still-life", None, "2o$2o!"));
        put("blinker.toml", &file("Blinker", "oscillator", Some(2), "3o!"));
        put("wrong-period.toml", &file("Wrong", "oscillator", Some(3), "3o!"));
        put("not-still.toml", &file("Not still", "still-life", None, "3o!"));
        put("garbage.toml", "this is = = not toml");
        put("no-category.toml", &file("X", "oscillator", Some(2), "3o!").replace("category = \"oscillator\"\n", ""));
        put("notes.txt", "ignored");
        let (entries, problems) = load(dir.path());
        let mut ok: Vec<_> = entries.iter().map(|e| e.id.as_str()).collect();
        ok.sort();
        assert_eq!(ok, ["blinker", "block"]);
        let mut bad: Vec<_> = problems.iter().map(|p| p.file.as_str()).collect();
        bad.sort();
        assert_eq!(bad, ["garbage.toml", "no-category.toml", "not-still.toml", "wrong-period.toml"]);
        assert!(problems.iter().all(|p| !p.reason.is_empty()));
    }

    #[test]
    fn speeds_are_derived() {
        assert_eq!(speed(4, 1, 1), "c/4 diagonal");
        assert_eq!(speed(4, 2, 0), "c/2 orthogonal");
        assert_eq!(speed(4, 0, -2), "c/2 orthogonal");
        assert_eq!(speed(5, 2, 0), "2c/5 orthogonal");
        assert_eq!(speed(3, 2, 1), "2c/3 oblique");
    }
}
