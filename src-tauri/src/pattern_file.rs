//! One pattern as a TOML file, shared by the built-in and personal libraries.

use glife_engine::{Pattern, Rule, Tiling};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::rle;

const FORMAT: u32 = 1;

/// Fields past `rle` are set only by built-in files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternFile {
    pub format: u32,
    pub tiling: String,
    /// 1 when the pattern's row 0 is a shifted row (hexagonal) or its cell
    /// `(0, 0)` points down (triangular).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: u8,
    #[serde(default)]
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

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// The current time as RFC 3339, in UTC.
pub fn now() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).expect("RFC 3339 formats")
}

impl PatternFile {
    /// A file for `pattern` under `rule`, with no timestamps.
    pub fn from_pattern(pattern: &Pattern, rule: Rule, name: &str, comment: Option<&str>) -> PatternFile {
        PatternFile {
            format: FORMAT,
            tiling: pattern.tiling().id().into(),
            offset: pattern.offset() as u8,
            name: name.into(),
            comment: comment.filter(|c| !c.is_empty()).map(String::from),
            created: None,
            changed: None,
            rle: rle::write(pattern, rule, None, None, None),
            category: None,
            description: None,
            period: None,
            dx: None,
            dy: None,
            gliders: None,
        }
    }

    /// A personal-library file, stamped with the current time as both
    /// `created` and `changed`.
    pub fn personal(pattern: &Pattern, rule: Rule, name: &str, comment: Option<&str>) -> PatternFile {
        let stamp = now();
        PatternFile {
            created: Some(stamp.clone()),
            changed: Some(stamp),
            ..PatternFile::from_pattern(pattern, rule, name, comment)
        }
    }

    /// Reads a pattern file; it must have a name.
    pub fn parse(text: &str) -> Result<PatternFile, String> {
        let file = PatternFile::parse_inner(text)?;
        if file.name.trim().is_empty() {
            return Err("The pattern has no name.".into());
        }
        Ok(file)
    }

    /// Reads pattern text as copied to the clipboard. `None` when the text
    /// is not a TOML table with an `rle` key, so it may be RLE; an error when
    /// it is pattern text that cannot be used. The name may be empty.
    pub fn parse_pattern_text(text: &str) -> Result<Option<PatternFile>, String> {
        match text.parse::<toml::Table>() {
            Ok(table) if table.contains_key("rle") => PatternFile::parse_inner(text).map(Some),
            _ => Ok(None),
        }
    }

    /// Pattern text: the pattern file without `created` and `changed`.
    pub fn pattern_text(pattern: &Pattern, rule: Rule, name: &str, comment: Option<&str>) -> Result<String, String> {
        PatternFile::from_pattern(pattern, rule, name, comment).render()
    }

    fn parse_inner(text: &str) -> Result<PatternFile, String> {
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
        let Some(tiling) = Tiling::parse(&file.tiling) else {
            return Err(format!(
                "The pattern is for the grid {:?}, which needs a newer version of glife.",
                file.tiling
            ));
        };
        if file.offset > 1 || (tiling == Tiling::Square && file.offset != 0) {
            return Err(format!("The offset {} does not fit the {} grid.", file.offset, tiling.id()));
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

    /// The pattern's cells and the rule it was made under, read from the
    /// `rle` field. A rule the grid cannot run is refused.
    pub fn cells(&self) -> Result<(Pattern, Rule), String> {
        let tiling = Tiling::parse(&self.tiling).ok_or_else(|| format!("Unknown grid {:?}.", self.tiling))?;
        let rle = rle::parse_as(&self.rle, tiling, self.offset == 1)?;
        let rule = rle::rule_of(&rle.rule, tiling)
            .map_err(|e| format!("The pattern's rule {} cannot run on the {} grid: {e}.", rle.rule, tiling.id()))?;
        Ok((rle.pattern, rule))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn life() -> Rule {
        Tiling::Square.default_rule()
    }

    fn sample(tiling: Tiling, offset: bool) -> Pattern {
        let mut p = Pattern::new(4, 3, tiling, offset);
        for (x, y) in [(0, 0), (2, 0), (1, 1), (3, 2)] {
            p.set(x, y, true);
        }
        p
    }

    /// A file around raw `rle` text.
    fn raw(name: &str, rle: &str) -> PatternFile {
        PatternFile { rle: rle.into(), ..PatternFile::from_pattern(&Pattern::new(1, 1, Tiling::Square, false), life(), name, None) }
    }

    #[test]
    fn personal_round_trip() {
        let p = sample(Tiling::Square, false);
        let f = PatternFile::personal(&p, life(), "My glider", Some("line 1\nline 2"));
        let text = f.render().unwrap();
        assert_eq!(PatternFile::parse(&text).unwrap(), f);
        assert_eq!(f.cells().unwrap(), (p, life()));
    }

    #[test]
    fn pattern_text_round_trips_on_each_grid() {
        for tiling in Tiling::ALL {
            for offset in [false, true] {
                if tiling == Tiling::Square && offset {
                    continue;
                }
                let rule = tiling.presets().last().unwrap().1;
                let p = sample(tiling, offset);
                let text = PatternFile::pattern_text(&p, rule, "Mine", Some("a\nb")).unwrap();
                assert!(!text.contains("created") && !text.contains("changed"), "{text}");
                let file = PatternFile::parse_pattern_text(&text).unwrap().unwrap();
                assert_eq!((file.name.as_str(), file.comment.as_deref()), ("Mine", Some("a\nb")));
                assert_eq!(file.cells().unwrap(), (p, rule), "{tiling:?} {offset}");
            }
        }
    }

    #[test]
    fn pattern_text_may_have_no_name_but_a_file_may_not() {
        let text = PatternFile::pattern_text(&sample(Tiling::Hexagonal, false), Tiling::Hexagonal.default_rule(), "", None).unwrap();
        assert_eq!(PatternFile::parse_pattern_text(&text).unwrap().unwrap().name, "");
        assert!(PatternFile::parse(&text).unwrap_err().contains("no name"));
    }

    #[test]
    fn rle_text_and_other_text_are_not_pattern_text() {
        assert!(PatternFile::parse_pattern_text("x = 3, y = 3\nbob$2bo$3o!\n").unwrap().is_none());
        assert!(PatternFile::parse_pattern_text("hello").unwrap().is_none());
        let bad = "format = 1\ntiling = \"pentagonal\"\nrle = \"x = 1, y = 1\\no!\"\n";
        assert!(PatternFile::parse_pattern_text(bad).unwrap_err().contains("newer"));
    }

    #[test]
    fn builtin_variant_round_trips() {
        let mut f = raw("Gun", "x = 1, y = 1\no!\n");
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
    fn bad_offsets_and_rules_are_refused() {
        let file = |tiling: &str, offset: u8, rule: &str| {
            format!("format = 1\ntiling = \"{tiling}\"\noffset = {offset}\nname = \"x\"\nrle = \"x = 1, y = 1, rule = {rule}\\no!\"\n")
        };
        assert!(PatternFile::parse(&file("square", 1, "B3/S23")).is_err());
        assert!(PatternFile::parse(&file("hexagonal", 2, "B2/S35")).is_err());
        let ok = PatternFile::parse(&file("hexagonal", 1, "B2/S35")).unwrap();
        assert!(ok.cells().is_ok());
        let too_many = PatternFile::parse(&file("triangular-3", 0, "B34/S2")).unwrap();
        assert!(too_many.cells().unwrap_err().contains("triangular-3"));
        let nonsense = PatternFile::parse(&file("square", 0, "Generations")).unwrap();
        assert!(nonsense.cells().is_err());
    }

    #[test]
    fn committed_fixture_loads_as_a_square_pattern() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/pattern-v1.toml");
        let f = PatternFile::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(f.name, "Sample glider");
        assert_eq!(f.offset, 0);
        assert_eq!(f.comment.as_deref(), Some("Kept for tests.\nTwo lines."));
        assert_eq!(f.created.as_deref(), Some("2026-01-02T03:04:05Z"));
        let (p, rule) = f.cells().unwrap();
        assert_eq!((p.tiling(), p.offset(), p.width(), p.height(), p.live_count()), (Tiling::Square, false, 3, 3, 5));
        assert_eq!(rule, life());
    }
}
