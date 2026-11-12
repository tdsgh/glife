//! Encodes a snapshot region into frame layout v1: little-endian 32-bit
//! words, a 16-word header, then layers (kind, word count, data).

use crate::sim::Snapshot;
use glife_engine::Edges;

const FORMAT_VERSION: u32 = 1;
const KIND_ALIVE_BITS: u32 = 0;

/// Encodes the cells of the requested region, which is clipped to the grid
/// and aligned to 32 columns. When `since` equals the snapshot's version the
/// reply has no layers. The region is in cells and may be fractional or
/// negative, as the view computes it.
pub fn encode(snapshot: &Snapshot, x0: f64, y0: f64, w: f64, h: f64, since: u64) -> Vec<u8> {
    let u = &snapshot.universe;
    let (width, height) = (u.width() as f64, u.height() as f64);
    let cols = u.width().next_multiple_of(32) as f64;
    let left = ((x0.max(0.0) / 32.0).floor() * 32.0).min(cols);
    let right = (((x0 + w).max(0.0).min(width) / 32.0).ceil() * 32.0).min(cols).max(left);
    let top = y0.floor().clamp(0.0, height);
    let bottom = (y0 + h).ceil().clamp(top, height);
    let (rx, ry, rw, rh) = (left as u32, top as u32, (right - left) as u32, (bottom - top) as u32);

    let mut words: Vec<u32> = Vec::with_capacity(16 + (rw / 32 * rh) as usize + 2);
    let version = snapshot.version;
    let generation = u.generation();
    words.extend([
        FORMAT_VERSION,
        snapshot.playing as u32,
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
        words.extend([1, KIND_ALIVE_BITS, rw / 32 * rh]);
        u.copy_region(rx, ry, rw, rh, &mut words);
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
        Snapshot { version: 0x1_0000_0005, universe, playing: true, gens_per_sec: 12.5 }
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
        let bytes = encode(&snapshot(), 0.0, 0.0, 64.0, 2.0, 0);
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
        let bytes = encode(&snapshot, 0.0, 0.0, 64.0, 2.0, snapshot.version);
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
        let w = words(encode(&snapshot, 33.0, -5.0, 1000.0, 1000.0, 0));
        assert_eq!(w[11..15], [32, 0, 32, 3]);
        assert_eq!(w[15..18], [1, 0, 3]);
        // A region wholly outside the grid is empty.
        let w = words(encode(&snapshot, 500.0, 500.0, 10.0, 10.0, 0));
        assert_eq!(w[13..15], [0, 0]);
        assert_eq!(w[15..18], [1, 0, 0]);
    }
}
