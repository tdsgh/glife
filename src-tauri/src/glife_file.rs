//! The `.glife` universe file: magic, a JSON header, then deflated sections.

use std::io::{Read, Write};
use std::path::Path;

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use glife_engine::{Edges, Pattern, Rule, Tiling, Universe, MAX_SIDE, MIN_SIDE};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"GLIFE\0\r\n";
const FORMAT: u32 = 1;
const MAX_HEADER: usize = 1 << 20;

#[derive(Serialize, Deserialize)]
struct Section {
    kind: String,
    codec: String,
    bytes: u64,
}

#[derive(Serialize, Deserialize)]
struct Header {
    format: u32,
    tiling: String,
    width: u32,
    height: u32,
    edges: String,
    rule: String,
    generation: u64,
    sections: Vec<Section>,
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).expect("write to memory");
    enc.finish().expect("write to memory")
}

/// Inflates `data`, refusing more than `expected` bytes or fewer.
fn inflate(data: &[u8], expected: usize, what: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(expected);
    DeflateDecoder::new(data)
        .take(expected as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|_| format!("The {what} section is damaged."))?;
    if out.len() != expected {
        return Err(format!("The {what} section has the wrong size."));
    }
    Ok(out)
}

/// Writes `bytes` to `path` through a temporary file in the same folder and a
/// rename, so a failed write leaves an existing file intact.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path.file_name().ok_or_else(|| std::io::Error::other("the path has no file name"))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn write(path: &Path, universe: &Universe) -> Result<(), String> {
    let (w, h) = (universe.width(), universe.height());
    let words_per_row = (w as usize).div_ceil(64);
    let mut alive = vec![0u64; words_per_row * h as usize];
    let mut ages: Vec<u8> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if universe.get(x, y) {
                alive[y as usize * words_per_row + (x / 64) as usize] |= 1 << (x % 64);
                ages.extend_from_slice(&universe.age(x, y).to_le_bytes());
            }
        }
    }
    let alive: Vec<u8> = alive.iter().flat_map(|word| word.to_le_bytes()).collect();
    let (alive, ages) = (deflate(&alive), deflate(&ages));
    let header = Header {
        format: FORMAT,
        tiling: universe.tiling().id().into(),
        width: w,
        height: h,
        edges: match universe.edges() {
            Edges::Wrap => "wrap",
            Edges::Dead => "dead",
        }
        .into(),
        rule: universe.rule().to_string(),
        generation: universe.generation(),
        sections: vec![
            Section { kind: "alive".into(), codec: "deflate".into(), bytes: alive.len() as u64 },
            Section { kind: "ages".into(), codec: "deflate".into(), bytes: ages.len() as u64 },
        ],
    };
    let header = serde_json::to_vec(&header).expect("header serializes");
    let mut file = Vec::with_capacity(12 + header.len() + alive.len() + ages.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&(header.len() as u32).to_le_bytes());
    file.extend_from_slice(&header);
    file.extend_from_slice(&alive);
    file.extend_from_slice(&ages);
    write_atomic(path, &file).map_err(|e| format!("Could not save the file: {e}."))
}

pub fn read(path: &Path) -> Result<Universe, String> {
    let data = std::fs::read(path).map_err(|e| format!("Could not read the file: {e}."))?;
    parse(&data)
}

fn parse(data: &[u8]) -> Result<Universe, String> {
    if data.len() < 12 || &data[..8] != MAGIC {
        return Err("This is not a glife file.".into());
    }
    let header_len = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    if header_len > MAX_HEADER {
        return Err("The file's header is damaged.".into());
    }
    let header_bytes = data.get(12..12 + header_len).ok_or("The file is truncated.")?;
    let raw: serde_json::Value =
        serde_json::from_slice(header_bytes).map_err(|_| "The file's header is damaged.")?;
    let format = raw.get("format").and_then(|f| f.as_u64()).ok_or("The file's header is damaged.")?;
    if format > FORMAT as u64 {
        return Err(format!(
            "This file was written by a newer version of glife (file format {format}); update glife to open it."
        ));
    }
    let header: Header =
        serde_json::from_value(raw).map_err(|e| format!("The file's header is damaged: {e}."))?;
    let tiling = Tiling::parse(&header.tiling).ok_or_else(|| {
        format!(
            "This file needs a newer version of glife: it uses the grid {:?}, which this version does not know.",
            header.tiling
        )
    })?;
    let rule = Rule::parse(&header.rule, tiling)
        .map_err(|e| format!("The file's rule {} cannot run on the {} grid: {e}.", header.rule, tiling.id()))?;
    let edges = match header.edges.as_str() {
        "wrap" => Edges::Wrap,
        "dead" => Edges::Dead,
        other => return Err(format!("The file's edge mode {other:?} is not supported.")),
    };
    let sides = MIN_SIDE..=MAX_SIDE;
    if !sides.contains(&header.width) || !sides.contains(&header.height) {
        return Err(format!(
            "The file's grid size {} x {} is outside {MIN_SIDE} to {MAX_SIDE} per side.",
            header.width, header.height
        ));
    }
    let (w, h) = (header.width, header.height);

    let mut offset = 12 + header_len;
    let mut alive: Option<&[u8]> = None;
    let mut ages: Option<&[u8]> = None;
    for section in &header.sections {
        let end = usize::try_from(section.bytes)
            .ok()
            .and_then(|n| offset.checked_add(n))
            .ok_or("The file is truncated.")?;
        let body = data.get(offset..end).ok_or("The file is truncated.")?;
        offset = end;
        match section.kind.as_str() {
            "alive" | "ages" => {
                if section.codec != "deflate" {
                    return Err(format!("The {} section uses an unknown codec.", section.kind));
                }
                if section.kind == "alive" { alive = Some(body) } else { ages = Some(body) }
            }
            _ => {}
        }
    }

    let words_per_row = (w as usize).div_ceil(64);
    let alive = inflate(alive.ok_or("The file has no cells.")?, words_per_row * h as usize * 8, "cells")?;
    let words: Vec<u64> =
        alive.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
    let mut cells = Pattern::new(w, h, tiling, false);
    let mut live = 0usize;
    for y in 0..h {
        for x in 0..w {
            if words[y as usize * words_per_row + (x / 64) as usize] >> (x % 64) & 1 == 1 {
                cells.set(x, y, true);
                live += 1;
            }
        }
    }
    let mut all_ages = vec![0u16; w as usize * h as usize];
    if let Some(ages) = ages {
        let ages = inflate(ages, live * 2, "ages")?;
        let mut next = ages.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
        for y in 0..h {
            for x in 0..w {
                if cells.get(x, y) {
                    all_ages[y as usize * w as usize + x as usize] = next.next().unwrap_or(0);
                }
            }
        }
    }
    Universe::from_parts(w, h, edges, tiling, rule, header.generation, &cells, &all_ages)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Universe {
        let mut u = Universe::new(70, 40, Edges::Wrap, Tiling::Square).unwrap();
        u.random_fill(0.3, 7);
        for _ in 0..25 {
            u.step();
        }
        u
    }

    fn assert_same(a: &Universe, b: &Universe) {
        assert_eq!((a.width(), a.height(), a.edges()), (b.width(), b.height(), b.edges()));
        assert_eq!(a.generation(), b.generation());
        for y in 0..a.height() {
            for x in 0..a.width() {
                assert_eq!(a.get(x, y), b.get(x, y), "cell {x},{y}");
                if a.get(x, y) {
                    assert_eq!(a.age(x, y), b.age(x, y), "age {x},{y}");
                }
            }
        }
    }

    /// A header plus sections, laid out as the format describes.
    fn assemble(header: &serde_json::Value, sections: &[&[u8]]) -> Vec<u8> {
        let header = serde_json::to_vec(header).unwrap();
        let mut file = MAGIC.to_vec();
        file.extend_from_slice(&(header.len() as u32).to_le_bytes());
        file.extend_from_slice(&header);
        for s in sections {
            file.extend_from_slice(s);
        }
        file
    }

    #[test]
    fn save_load_then_100_steps_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("u.glife");
        let mut original = sample();
        write(&path, &original).unwrap();
        let mut loaded = read(&path).unwrap();
        assert_same(&original, &loaded);
        for _ in 0..100 {
            original.step();
            loaded.step();
        }
        assert_same(&original, &loaded);
    }

    #[test]
    fn committed_fixture_loads_to_its_known_content() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/universe-v1.glife");
        let u = read(&path).unwrap();
        assert_eq!((u.width(), u.height(), u.edges(), u.generation()), (10, 8, Edges::Wrap, 12));
        let live = [(1, 0, 3), (2, 1, 3), (0, 2, 3), (1, 2, 4), (2, 2, 5), (7, 5, 0), (8, 5, 0), (9, 5, 7)];
        assert_eq!(u.live_count() as usize, live.len());
        for (x, y, age) in live {
            assert!(u.get(x, y), "{x},{y}");
            assert_eq!(u.age(x, y), age, "{x},{y}");
        }
    }

    #[test]
    fn unknown_section_and_header_key_are_skipped() {
        // Row 1 has bit 1 set; rows are one word each.
        let mut words = [0u64; 4];
        words[1] = 1 << 1;
        let alive: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let alive = deflate(&alive);
        let ages = deflate(&9u16.to_le_bytes());
        let extra = b"whatever";
        let header = serde_json::json!({
            "format": 1, "tiling": "square", "width": 5, "height": 4, "edges": "dead",
            "rule": "B3/S23", "generation": 3, "future": [1, 2],
            "sections": [
                {"kind": "alive", "codec": "deflate", "bytes": alive.len()},
                {"kind": "palette", "codec": "raw", "bytes": extra.len()},
                {"kind": "ages", "codec": "deflate", "bytes": ages.len()},
            ],
        });
        let u = parse(&assemble(&header, &[&alive, extra, &ages])).unwrap();
        assert_eq!((u.generation(), u.live_count()), (3, 1));
        assert!(u.get(1, 1));
        assert_eq!(u.age(1, 1), 9);
    }

    #[test]
    fn newer_format_is_refused() {
        let header = serde_json::json!({"format": 2, "sections": []});
        let e = parse(&assemble(&header, &[])).err().unwrap();
        assert!(e.contains("newer version of glife"), "{e}");
    }

    #[test]
    fn truncated_and_foreign_files_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("u.glife");
        write(&path, &sample()).unwrap();
        let data = std::fs::read(&path).unwrap();
        for cut in [0, 5, 11, 20, data.len() / 2, data.len() - 1] {
            assert!(parse(&data[..cut]).is_err(), "cut at {cut}");
        }
        assert!(parse(b"not a glife file at all").is_err());
    }

    #[test]
    fn bad_sizes_are_errors_not_panics() {
        for (w, h) in [(0, 5), (5, 0), (2, 5), (2049, 5), (5, 4096)] {
            let header = serde_json::json!({
                "format": 1, "tiling": "square", "width": w, "height": h, "edges": "dead",
                "rule": "B3/S23", "generation": 0, "sections": [],
            });
            assert!(parse(&assemble(&header, &[])).is_err(), "{w}x{h}");
        }
    }

    /// The fixture's live cells and their ages, as written when it was committed.
    const FIXTURE_HEX_CELLS: [(u32, u32, u16); 20] = [
        (4, 0, 1), (0, 1, 1), (1, 2, 1), (3, 2, 2), (8, 2, 3), (9, 2, 1), (1, 3, 0), (2, 3, 3), (7, 3, 3), (9, 3, 1),
        (7, 4, 0), (8, 4, 1), (9, 4, 0), (6, 5, 3), (7, 5, 0), (3, 6, 0), (7, 6, 4), (1, 7, 3), (2, 7, 1), (3, 7, 1),
    ];

    fn grid_sample(tiling: Tiling, rule: &str) -> Universe {
        let mut u = Universe::new(40, 24, Edges::Wrap, tiling).unwrap();
        u.set_rule(Rule::parse(rule, tiling).unwrap()).unwrap();
        u.random_fill(0.3, 11);
        for _ in 0..15 {
            u.step();
        }
        u
    }

    #[test]
    fn every_grid_round_trips_cells_ages_tiling_and_rule() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("u.glife");
        for (tiling, rule) in [
            (Tiling::Square, "B3/S23"),
            (Tiling::Square, "B36/S23"),
            (Tiling::Hexagonal, "B2/S35"),
            (Tiling::Hexagonal, "B3/S23"),
            (Tiling::Triangular12, "B456/S45"),
            (Tiling::Triangular12, "B4,10,11/S5,12"),
            (Tiling::Triangular3, "B2/S013"),
        ] {
            let original = grid_sample(tiling, rule);
            write(&path, &original).unwrap();
            let loaded = read(&path).unwrap();
            assert_same(&original, &loaded);
            assert_eq!((loaded.tiling(), loaded.rule()), (tiling, original.rule()), "{tiling:?} {rule}");
            assert_eq!(loaded.rule(), Rule::parse(rule, tiling).unwrap());
        }
    }

    #[test]
    fn the_hexagonal_fixture_loads_under_its_rule() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/universe-hex-v1.glife");
        let u = read(&path).unwrap();
        assert_eq!((u.tiling(), u.width(), u.height(), u.edges()), (Tiling::Hexagonal, 10, 8, Edges::Wrap));
        // B3/S23 rather than the grid's default, B2/S35.
        assert_eq!(u.rule(), Rule::parse("B3/S23", Tiling::Hexagonal).unwrap());
        assert_ne!(u.rule(), Tiling::Hexagonal.default_rule());
        assert_eq!(u.generation(), 4);
        let live: Vec<(u32, u32, u16)> = (0..8)
            .flat_map(|y| (0..10).map(move |x| (x, y)))
            .filter(|&(x, y)| u.get(x, y))
            .map(|(x, y)| (x, y, u.age(x, y)))
            .collect();
        assert_eq!(live, FIXTURE_HEX_CELLS);
    }

    #[test]
    fn a_phase_2_file_opens_as_a_square_universe_under_b3_s23() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/universe-v1.glife");
        let u = read(&path).unwrap();
        assert_eq!((u.tiling(), u.rule()), (Tiling::Square, Tiling::Square.default_rule()));
    }

    #[test]
    fn an_unknown_tiling_is_refused_with_the_newer_glife_message() {
        let header = serde_json::json!({
            "format": 1, "tiling": "pentagonal", "width": 5, "height": 4, "edges": "dead",
            "rule": "B3/S23", "generation": 0, "sections": [],
        });
        let e = parse(&assemble(&header, &[])).err().unwrap();
        assert!(e.contains("newer version of glife") && e.contains("pentagonal"), "{e}");
    }

    #[test]
    fn a_rule_the_grid_cannot_run_is_refused_naming_it() {
        for (tiling, rule) in [("hexagonal", "B3/S27"), ("square", "B0/S23"), ("triangular-3", "B2/S4"), ("square", "Life")] {
            let header = serde_json::json!({
                "format": 1, "tiling": tiling, "width": 6, "height": 4, "edges": "dead",
                "rule": rule, "generation": 0, "sections": [],
            });
            let e = parse(&assemble(&header, &[])).err().unwrap();
            assert!(e.contains(rule) && e.contains(tiling), "{e}");
        }
    }

    #[test]
    fn a_wrapping_file_with_an_odd_side_the_grid_cannot_wrap_is_refused() {
        let alive = deflate(&[0; 4 * 8]);
        let ages = deflate(&[]);
        let header = serde_json::json!({
            "format": 1, "tiling": "triangular-12", "width": 7, "height": 4, "edges": "wrap",
            "rule": "B456/S45", "generation": 0,
            "sections": [
                {"kind": "alive", "codec": "deflate", "bytes": alive.len()},
                {"kind": "ages", "codec": "deflate", "bytes": ages.len()},
            ],
        });
        let e = parse(&assemble(&header, &[&alive, &ages])).err().unwrap();
        assert!(e.contains("even"), "{e}");
    }

    #[test]
    fn write_into_a_read_only_folder_fails_and_keeps_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("u.glife");
        let original = sample();
        write(&path, &original).unwrap();
        let before = std::fs::read(&path).unwrap();

        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(dir.path(), perms.clone()).unwrap();
        let result = write(&path, &Universe::new(5, 5, Edges::Dead, Tiling::Square).unwrap());
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(dir.path(), perms).unwrap();

        // A superuser ignores the permission; there is nothing to check then.
        if result.is_err() {
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert!(std::fs::read_dir(dir.path()).unwrap().count() == 1);
        }
    }
}
