//! One pattern as a TOML file, shared by the built-in and personal libraries.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::rle::{self, Rle};

const FORMAT: u32 = 1;

/// Fields past `rle` are set only by built-in files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternFile {
    pub format: u32,
    pub tiling: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed: Option<String>,
    pub rle: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dx: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dy: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gliders: Option<u32>,
}

/// The current time as RFC 3339, in UTC.
pub fn now() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).expect("RFC 3339 formats")
}

impl PatternFile {
    /// A personal-library file for `rle` text, stamped with the current time
    /// as both `created` and `changed`.
    pub fn personal(name: &str, comment: Option<&str>, rle: String) -> PatternFile {
        let stamp = now();
        PatternFile {
            format: FORMAT,
            tiling: "square".into(),
            name: name.into(),
            comment: comment.filter(|c| !c.is_empty()).map(String::from),
            created: Some(stamp.clone()),
            changed: Some(stamp),
            rle,
            category: None,
            description: None,
            period: None,
            dx: None,
            dy: None,
            gliders: None,
        }
    }

    pub fn parse(text: &str) -> Result<PatternFile, String> {
        let raw: toml::Table = text.parse().map_err(|e| format!("Not a valid pattern file: {e}"))?;
        if let Some(format) = raw.get("format").and_then(|f| f.as_integer()) {
            if format > FORMAT as i64 {
                return Err(format!(
                    "This pattern was written by a newer version of glife (file format {format})."
                ));
            }
        }
        let file: PatternFile =
            raw.try_into().map_err(|e| format!("Not a valid pattern file: {e}"))?;
        if file.format != FORMAT {
            return Err(format!("Unknown pattern file format {}.", file.format));
        }
        if file.tiling != "square" {
            return Err(format!("The pattern's tiling {:?} is not supported.", file.tiling));
        }
        if file.name.trim().is_empty() {
            return Err("The pattern has no name.".into());
        }
        for (key, stamp) in [("created", &file.created), ("changed", &file.changed)] {
            if let Some(s) = stamp {
                OffsetDateTime::parse(s, &Rfc3339)
                    .map_err(|_| format!("The {key} date {s:?} is not an RFC 3339 time."))?;
            }
        }
        Ok(file)
    }

    pub fn render(&self) -> Result<String, String> {
        toml::to_string(self).map_err(|e| format!("Could not write the pattern file: {e}"))
    }

    /// The pattern's cells, read from the `rle` field.
    pub fn rle(&self) -> Result<Rle, String> {
        rle::parse(&self.rle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_round_trip() {
        let mut f = PatternFile::personal("My glider", Some("line 1\nline 2"), "x = 3, y = 3\nbob$2bo$3o!\n".into());
        f.comment = Some("line 1\nline 2".into());
        let text = f.render().unwrap();
        assert_eq!(PatternFile::parse(&text).unwrap(), f);
        assert_eq!(f.rle().unwrap().pattern.live_count(), 5);
    }

    #[test]
    fn builtin_variant_round_trips() {
        let mut f = PatternFile::personal("Gun", None, "x = 1, y = 1\no!\n".into());
        f.created = None;
        f.changed = None;
        f.category = Some("gun".into());
        f.description = Some("Emits gliders.\nSecond line.".into());
        f.period = Some(30);
        f.dx = Some(-1);
        f.dy = Some(1);
        f.gliders = Some(1);
        assert_eq!(PatternFile::parse(&f.render().unwrap()).unwrap(), f);
    }

    #[test]
    fn newer_format_and_bad_dates_are_refused() {
        let base = "tiling = \"square\"\nname = \"x\"\nrle = \"x = 1, y = 1\\no!\"\n";
        assert!(PatternFile::parse(&format!("format = 2\n{base}")).unwrap_err().contains("newer"));
        assert!(PatternFile::parse(&format!("format = 1\ncreated = \"yesterday\"\n{base}")).is_err());
        assert!(PatternFile::parse(&format!("format = 1\n{base}")).is_ok());
        assert!(PatternFile::parse("not toml = = =").is_err());
    }

    #[test]
    fn committed_fixture_loads() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/pattern-v1.toml");
        let f = PatternFile::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(f.name, "Sample glider");
        assert_eq!(f.comment.as_deref(), Some("Kept for tests.\nTwo lines."));
        assert_eq!(f.created.as_deref(), Some("2026-01-02T03:04:05Z"));
        let r = f.rle().unwrap();
        assert_eq!((r.pattern.width(), r.pattern.height(), r.pattern.live_count()), (3, 3, 5));
    }
}
