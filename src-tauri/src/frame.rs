//! Encodes a snapshot region into frame layout v1: little-endian 32-bit
//! words, a 16-word header, then layers (kind, word count, data).

use crate::sim::Snapshot;
use glife_engine::Edges;

const FORMAT_VERSION: u32 = 1;
const KIND_ALIVE_BITS: u32 = 0;
const KIND_AGE: u32 = 1;
const KIND_TRAIL: u32 = 2;
/// Bits of the `layers` request.
pub const LAYER_AGE: u32 = 1;
pub const LAYER_TRAIL: u32 = 2;

/// Encodes the cells of the requested region, which is clipped to the grid
/// and aligned to 32 columns. When `since` equals the snapshot's version the
/// reply has no layers. The region is in cells and may be fractional or
/// negative, as the view computes it. The alive bits are always sent; `layers`
/// adds the age layer (bit 0) and the trail layer (bit 1).
pub fn encode(
    snapshot: &Snapshot,
    x0: f64,
    y0: f64,
    w: f64,
    h: f64,
    since: u64,
    layers: u32,
) -> Vec<u8> {
    let u = &snapshot.universe;
    let (width, height) = (u.width() as f64, u.height() as f64);
    let cols = u.width().next_multiple_of(32) as f64;
    let left = ((x0.max(0.0) / 32.0).floor() * 32.0).min(cols);
    let right = (((x0 + w).max(0.0).min(width) / 32.0).ceil() * 32.0).min(cols).max(left);
    let top = y0.floor().clamp(0.0, height);
    let bottom = (y0 + h).ceil().clamp(top, height);
    let (rx, ry, rw, rh) = (left as u32, top as u32, (right - left) as u32, (bottom - top) as u32);

    let layers = layers & (LAYER_AGE | LAYER_TRAIL);
    let bytes_per_layer = (rw / 4 * rh) as usize;
    let mut words: Vec<u32> = Vec::with_capacity(
        16 + (rw / 32 * rh) as usize + 2 + layers.count_ones() as usize * (bytes_per_layer + 2),
    );
    let version = snapshot.version;
    let generation = u.generation();
    words.extend([
        FORMAT_VERSION,
        snapshot.playing as u32
            | (snapshot.can_undo as u32) << 1
            | (snapshot.can_redo as u32) << 2
            | (snapshot.dirty as u32) << 3,
        version as u32,
        (version >> 32) as u32,
        generation as u32,
        (generation >> 32) as u32,
        u.live_count(),
        snapshot.gens_per_sec.to_bits(),
        u.width(),
        u.height(),
        match u.edges() {
            Edges::Wrap => 0,
            Edges::Dead => 1,
        },
        rx,
        ry,
        rw,
        rh,
    ]);
    if since == version {
        words.push(0);
    } else {
        words.extend([1 + layers.count_ones(), KIND_ALIVE_BITS, rw / 32 * rh]);
        u.copy_region(rx, ry, rw, rh, &mut words);
        if layers & LAYER_AGE != 0 {
            words.extend([KIND_AGE, rw / 4 * rh]);
            u.copy_ages(rx, ry, rw, rh, &mut words);
        }
        if layers & LAYER_TRAIL != 0 {
            words.extend([KIND_TRAIL, rw / 4 * rh]);
            u.copy_trail(rx, ry, rw, rh, &mut words);
        }
    }
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glife_engine::Universe;
    use std::path::PathBuf;

    fn snapshot() -> Snapshot {
        let mut universe = Universe::new(64, 3, Edges::Dead).unwrap();
        for (x, y) in [(0, 0), (3, 0), (31, 0), (32, 0), (63, 0), (1, 1), (40, 1)] {
            universe.set(x, y, true);
        }
        Snapshot {
            version: 0x1_0000_0005,
            universe,
            playing: true,
            gens_per_sec: 12.5,
            revision: 1,
            dirty: false,
            can_undo: false,
            can_redo: false,
        }
    }

    /// The layout written out by hand, independent of the encoder.
    fn expected_words() -> Vec<u32> {
        vec![
            1, 1, 5, 1, 0, 0, 7, 12.5f32.to_bits(), 64, 3, 1, 0, 0, 64, 2, //
            1, 0, 4, // one alive-bits layer of 2 rows x 2 words
            0x8000_0009, 0x8000_0001, 0x0000_0002, 0x0000_0100,
        ]
    }

    #[test]
    fn matches_the_fixture_byte_for_byte() {
        let bytes = encode(&snapshot(), 0.0, 0.0, 64.0, 2.0, 0, 0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/frame-v1.bin");
        if !path.exists() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &bytes).unwrap();
        }
        assert_eq!(bytes, std::fs::read(&path).unwrap());
        let expected: Vec<u8> = expected_words().iter().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn unchanged_version_replies_with_zero_layers() {
        let snapshot = snapshot();
        let bytes = encode(&snapshot, 0.0, 0.0, 64.0, 2.0, snapshot.version, 3);
        let mut expected: Vec<u32> = expected_words()[..15].to_vec();
        expected.push(0);
        let expected: Vec<u8> = expected.iter().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn region_is_clipped_to_the_grid_and_aligned_to_32() {
        let snapshot = snapshot();
        let words = |bytes: Vec<u8>| -> Vec<u32> {
            bytes.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
        };
        let w = words(encode(&snapshot, 33.0, -5.0, 1000.0, 1000.0, 0, 0));
        assert_eq!(w[11..15], [32, 0, 32, 3]);
        assert_eq!(w[15..18], [1, 0, 3]);
        // A region wholly outside the grid is empty.
        let w = words(encode(&snapshot, 500.0, 500.0, 10.0, 10.0, 0, 0));
        assert_eq!(w[13..15], [0, 0]);
        assert_eq!(w[15..18], [1, 0, 0]);
    }

    #[test]
    fn flag_bits_carry_undo_redo_and_unsaved_changes() {
        let flags = |playing, can_undo, can_redo, dirty| {
            let snapshot = Snapshot { playing, can_undo, can_redo, dirty, ..snapshot() };
            let bytes = encode(&snapshot, 0.0, 0.0, 64.0, 2.0, 0, 0);
            u32::from_le_bytes(bytes[4..8].try_into().unwrap())
        };
        assert_eq!(flags(false, false, false, false), 0);
        assert_eq!(flags(true, false, false, false), 1);
        assert_eq!(flags(false, true, false, false), 2);
        assert_eq!(flags(false, false, true, false), 4);
        assert_eq!(flags(false, false, false, true), 8);
        assert_eq!(flags(true, true, true, true), 15);
    }

    /// A 64 x 3 universe with a block aged 305 and another that loses two
    /// cells to overpopulation, so the layers hold an age above 255 and
    /// trail counters of 1, 2 and 3.
    fn layered_snapshot() -> Snapshot {
        let mut universe = Universe::new(64, 3, Edges::Dead).unwrap();
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            universe.set(x, y, true);
        }
        for _ in 0..300 {
            universe.step();
        }
        for (x, y) in [(50, 0), (51, 0), (50, 1), (51, 1)] {
            universe.set(x, y, true);
        }
        for _ in 0..5 {
            universe.step();
        }
        universe.set(52, 0, true);
        universe.set(52, 1, true);
        for _ in 0..3 {
            universe.step();
        }
        Snapshot {
            version: 7,
            universe,
            playing: false,
            gens_per_sec: 0.0,
            revision: 1,
            dirty: false,
            can_undo: false,
            can_redo: false,
        }
    }

    /// The layered frame written out by hand. Ages: the block is 308, capped
    /// at 255; cells 50, 51, 52 of row 0 are 6, 5, 1 and of row 1 are 7, 0, 2.
    fn layered_words(layers: &[u32]) -> Vec<u32> {
        let mut words = vec![1, 0, 7, 0, 308, 0, 6, 0f32.to_bits(), 64, 3, 1, 0, 0, 64, 2];
        words.push(layers.len() as u32 + 1);
        words.extend([0, 4, 0x3, 0, 0x3, 0x0008_0000]);
        for layer in layers {
            if *layer == 1 {
                words.extend([1, 32]);
                words.extend([0xffff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0506_0000, 1, 0, 0]);
                words.extend([0xffff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0007_0000, 2, 0, 0]);
            } else {
                words.extend([2, 32]);
                words.extend([0; 12]);
                words.extend([0x0302_0000, 2, 0, 0]);
                words.extend([0; 12]);
                words.extend([0x0001_0000, 1, 0, 0]);
            }
        }
        words
    }

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn layers_match_the_fixture_byte_for_byte() {
        let snapshot = layered_snapshot();
        let u = &snapshot.universe;
        assert_eq!((u.age(0, 0), u.age(50, 0), u.trail(51, 0), u.trail(50, 1)), (308, 6, 3, 1));
        let bytes = encode(&snapshot, 0.0, 0.0, 64.0, 2.0, 0, LAYER_AGE | LAYER_TRAIL);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/frame-v1-layers.bin");
        if !path.exists() {
            std::fs::write(&path, &bytes).unwrap();
        }
        assert_eq!(bytes, std::fs::read(&path).unwrap());
        assert_eq!(bytes, le(&layered_words(&[1, 2])));
    }

    #[test]
    fn requesting_only_the_trail_sends_kinds_0_and_2() {
        let bytes = encode(&layered_snapshot(), 0.0, 0.0, 64.0, 2.0, 0, LAYER_TRAIL);
        assert_eq!(bytes, le(&layered_words(&[2])));
        let age = encode(&layered_snapshot(), 0.0, 0.0, 64.0, 2.0, 0, LAYER_AGE);
        assert_eq!(age, le(&layered_words(&[1])));
    }

    /// Run with `cargo test --release -p glife-app -- --ignored --nocapture timing`.
    #[test]
    #[ignore]
    fn timing_full_frame_with_both_layers() {
        let mut universe = Universe::new(2048, 2048, Edges::Wrap).unwrap();
        universe.random_fill(0.25, 1);
        for _ in 0..10 {
            universe.step();
        }
        let snapshot = Snapshot {
            version: 2,
            universe,
            playing: true,
            gens_per_sec: 0.0,
            revision: 1,
            dirty: false,
            can_undo: false,
            can_redo: false,
        };
        let runs = 20;
        let start = std::time::Instant::now();
        let mut len = 0;
        for _ in 0..runs {
            len = encode(&snapshot, 0.0, 0.0, 2048.0, 2048.0, 0, LAYER_AGE | LAYER_TRAIL).len();
        }
        println!("encode 2048 x 2048, both layers: {:.2} ms, {len} bytes", start.elapsed().as_secs_f64() * 1000.0 / runs as f64);
    }
}
