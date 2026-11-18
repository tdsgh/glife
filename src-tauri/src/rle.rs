//! Reading and writing Golly-style RLE text. Square patterns are plain RLE; a
//! pattern file reuses the format for the other grids.

use glife_engine::{Pattern, Rule, Tiling};

const MAX_SIDE: u32 = 2048;

/// A bounded grid declared by a `:T w,h` style suffix on the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounded {
    /// `'T'` torus, `'P'` plane, anything else is a grid kind glife lacks.
    pub kind: char,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rle {
    pub pattern: Pattern,
    pub name: Option<String>,
    pub comment: Option<String>,
    /// The rule as written in the header, without the grid suffix; the grid's
    /// default when the header has none.
    pub rule: String,
    pub bounded: Option<Bounded>,
    pub notices: Vec<String>,
}

/// The rule a header names, as `tiling` runs it. Golly's `23/3` and `Life`
/// mean B3/S23 on the square grid.
pub fn rule_of(text: &str, tiling: Tiling) -> Result<Rule, String> {
    if tiling == Tiling::Square && matches!(text.to_ascii_uppercase().as_str(), "23/3" | "LIFE") {
        return Ok(Tiling::Square.default_rule());
    }
    Rule::parse(text, tiling).map_err(|e| e.to_string())
}

/// The refusal for RLE text that would describe a pattern of another grid.
pub fn square_only(tiling: Tiling) -> Result<(), String> {
    if tiling == Tiling::Square {
        Ok(())
    } else {
        Err(format!("RLE describes square grids, so it cannot be used with the {} grid.", tiling.name()))
    }
}

fn parse_side(text: &str, what: &str) -> Result<u32, String> {
    let n: u64 = text
        .trim()
        .parse()
        .map_err(|_| format!("The header's {what} is not a number."))?;
    if n == 0 || n > MAX_SIDE as u64 {
        return Err(format!(
            "The pattern's {what} is {n}; glife reads 1 to {MAX_SIDE} cells per side."
        ));
    }
    Ok(n as u32)
}

/// Reads the grid suffix after `:`, for example `T100,80`. A side of 0 means
/// unbounded, and a bound on only one side is ignored with a notice.
fn parse_bounded(suffix: &str, notices: &mut Vec<String>) -> Option<Bounded> {
    let mut chars = suffix.trim().chars();
    let kind = chars.next()?.to_ascii_uppercase();
    let sides: Vec<Option<u32>> = chars
        .as_str()
        .split(',')
        .map(|s| {
            // Golly allows an origin shift after the size, `T100+5,80-3`.
            let digits: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect();
    match sides.as_slice() {
        [Some(w), Some(h)] if *w > 0 && *h > 0 => Some(Bounded { kind, width: *w, height: *h }),
        [Some(w)] if *w > 0 => Some(Bounded { kind, width: *w, height: *w }),
        _ => {
            notices.push("The file declares a grid that is unbounded on a side; it was ignored.".into());
            None
        }
    }
}

pub fn parse(text: &str) -> Result<Rle, String> {
    parse_as(text, Tiling::Square, false)
}

/// Reads RLE text as a pattern of `tiling` with `offset`.
pub fn parse_as(text: &str, tiling: Tiling, offset: bool) -> Result<Rle, String> {
    let mut name: Option<String> = None;
    let mut comments: Vec<String> = Vec::new();
    let mut lines = text.lines();
    let mut header = None;
    for line in lines.by_ref() {
        let line = line.trim_end();
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('#') {
            let mut chars = rest.chars();
            let tag = chars.next();
            if rest.starts_with("CXRLE") {
                continue;
            }
            let value = chars.as_str().strip_prefix(' ').unwrap_or(chars.as_str());
            match tag {
                Some('N') => name = Some(value.trim().to_string()),
                Some('C' | 'c') => comments.push(value.to_string()),
                _ => {}
            }
        } else if trimmed.starts_with('x') && trimmed.contains('=') {
            header = Some(trimmed.to_string());
            break;
        }
    }
    let header = header.ok_or("No RLE pattern found: the `x = …, y = …` line is missing.")?;

    // `x = 3, y = 3, rule = B3/S23:T100,80` — the suffix holds a comma, so
    // split off only the first two fields.
    let mut fields = header.splitn(3, ',');
    let value = |field: Option<&str>, key: &str| -> Result<String, String> {
        let field = field.ok_or_else(|| format!("The header has no {key}."))?;
        let (k, v) = field.split_once('=').ok_or_else(|| format!("The header has no {key}."))?;
        if k.trim() != key {
            return Err(format!("The header has no {key}."));
        }
        Ok(v.trim().to_string())
    };
    let width = parse_side(&value(fields.next(), "x")?, "width")?;
    let height = parse_side(&value(fields.next(), "y")?, "height")?;
    let mut notices = Vec::new();
    let mut rule = tiling.default_rule().to_string();
    let mut bounded = None;
    if let Some(rest) = fields.next() {
        let written = value(Some(rest), "rule")?;
        let (rule_text, suffix) = match written.split_once(':') {
            Some((r, s)) => (r.trim(), Some(s)),
            None => (written.trim(), None),
        };
        if !rule_text.is_empty() {
            rule = rule_text.to_string();
        }
        if let Some(suffix) = suffix {
            bounded = parse_bounded(suffix, &mut notices);
        }
    }

    let mut pattern = Pattern::new(width, height, tiling, offset);
    let (mut x, mut y) = (0u64, 0u64);
    let mut run: Option<u64> = None;
    'body: for line in lines {
        for c in line.chars() {
            match c {
                '0'..='9' => {
                    let d = c as u64 - '0' as u64;
                    run = Some(run.unwrap_or(0).saturating_mul(10).saturating_add(d));
                    if run > Some(u32::MAX as u64) {
                        return Err("A run length in the pattern is too large.".into());
                    }
                }
                'b' | 'B' | 'o' | 'O' => {
                    let n = run.take().unwrap_or(1);
                    if matches!(c, 'o' | 'O') {
                        if x + n > width as u64 || y >= height as u64 {
                            return Err("The pattern has cells outside its declared size.".into());
                        }
                        for i in 0..n {
                            pattern.set((x + i) as u32, y as u32, true);
                        }
                    }
                    x += n;
                }
                '$' => {
                    y += run.take().unwrap_or(1);
                    x = 0;
                }
                '!' => break 'body,
                c if c.is_whitespace() => {}
                c if c.is_ascii_alphabetic() || c == '.' => {
                    return Err("The pattern has more than two cell states; glife reads only alive and dead.".into());
                }
                c => return Err(format!("Unexpected character {c:?} in the pattern.")),
            }
        }
    }
    Ok(Rle {
        pattern,
        name: name.filter(|n| !n.is_empty()),
        comment: if comments.is_empty() { None } else { Some(comments.join("\n")) },
        rule,
        bounded,
        notices,
    })
}

/// RLE text for `pattern`, 70 columns at most. The header names `rule` and
/// keeps the full rectangle; a `bounded` grid adds its `:T`/`:P` suffix.
pub fn write(
    pattern: &Pattern,
    rule: Rule,
    name: Option<&str>,
    comment: Option<&str>,
    bounded: Option<Bounded>,
) -> String {
    let mut out = String::new();
    if let Some(name) = name {
        out.push_str(&format!("#N {}\n", name.replace(['\r', '\n'], " ")));
    }
    for line in comment.into_iter().flat_map(str::lines) {
        out.push_str(&format!("#C {line}\n"));
    }
    out.push_str(&format!("x = {}, y = {}, rule = {rule}", pattern.width(), pattern.height()));
    if let Some(b) = bounded {
        out.push_str(&format!(":{}{},{}", b.kind, b.width, b.height));
    }
    out.push('\n');

    let mut tokens: Vec<String> = Vec::new();
    let mut dollars = 0u32;
    let token = |n: u32, c: char| if n == 1 { c.to_string() } else { format!("{n}{c}") };
    for y in 0..pattern.height() {
        if y > 0 {
            dollars += 1;
        }
        // Runs of one state, dropping the dead run at the end of the row.
        let mut runs: Vec<(u32, bool)> = Vec::new();
        for x in 0..pattern.width() {
            let alive = pattern.get(x, y);
            match runs.last_mut() {
                Some((n, state)) if *state == alive => *n += 1,
                _ => runs.push((1, alive)),
            }
        }
        if matches!(runs.last(), Some((_, false))) {
            runs.pop();
        }
        if runs.is_empty() {
            continue;
        }
        if dollars > 0 {
            tokens.push(token(dollars, '$'));
            dollars = 0;
        }
        tokens.extend(runs.into_iter().map(|(n, alive)| token(n, if alive { 'o' } else { 'b' })));
    }
    tokens.push("!".into());

    let mut line_len = 0;
    for t in tokens {
        if line_len + t.len() > 70 {
            out.push('\n');
            line_len = 0;
        }
        line_len += t.len();
        out.push_str(&t);
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn life() -> Rule {
        Tiling::Square.default_rule()
    }

    fn glider() -> Pattern {
        let mut p = Pattern::new(3, 3, Tiling::Square, false);
        for (x, y) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
            p.set(x, y, true);
        }
        p
    }

    #[test]
    fn round_trip_keeps_cells_size_name_and_comment() {
        // A dead margin: 8 x 6 around a glider.
        let mut p = Pattern::new(8, 6, Tiling::Square, false);
        for (x, y) in [(2, 1), (3, 2), (1, 3), (2, 3), (3, 3)] {
            p.set(x, y, true);
        }
        let text = write(&p, life(), Some("Glider"), Some("first line\nsecond line"), None);
        let back = parse(&text).unwrap();
        assert_eq!(back.pattern, p);
        assert_eq!(back.name.as_deref(), Some("Glider"));
        assert_eq!(back.comment.as_deref(), Some("first line\nsecond line"));
        assert!(back.notices.is_empty());
        assert!(text.contains("x = 8, y = 6, rule = B3/S23\n"));
    }

    #[test]
    fn written_body_is_exact_and_wrapped() {
        assert_eq!(
            write(&glider(), life(), None, None, None),
            "x = 3, y = 3, rule = B3/S23\nbo$2bo$3o!\n"
        );
        let mut wide = Pattern::new(200, 1, Tiling::Square, false);
        for x in (0..200).step_by(2) {
            wide.set(x, 0, true);
        }
        let text = write(&wide, life(), None, None, None);
        assert!(text.lines().all(|l| l.len() <= 70));
        assert_eq!(parse(&text).unwrap().pattern, wide);
    }

    #[test]
    fn empty_rows_and_empty_pattern_round_trip() {
        let mut p = Pattern::new(4, 5, Tiling::Square, false);
        p.set(0, 0, true);
        p.set(3, 4, true);
        assert_eq!(parse(&write(&p, life(), None, None, None)).unwrap().pattern, p);
        let empty = Pattern::new(5, 5, Tiling::Square, false);
        assert_eq!(parse(&write(&empty, life(), None, None, None)).unwrap().pattern, empty);
    }

    #[test]
    fn text_before_the_pattern_is_skipped() {
        let r = parse("some prose\nmore\n#N Name\nx = 3, y = 3\nbob$2bo$3o!\n").unwrap();
        assert_eq!(r.pattern, glider());
        assert_eq!(r.name.as_deref(), Some("Name"));
        assert_eq!(r.rule, "B3/S23");
    }

    #[test]
    fn text_without_a_pattern_is_an_error() {
        assert!(parse("hello world\n").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn multi_state_and_oversize_are_refused() {
        let e = parse("x = 3, y = 3, rule = Generations\n.A$2A!\n").unwrap_err();
        assert!(e.contains("more than two"), "{e}");
        assert!(parse("x = 2049, y = 3\n!\n").is_err());
        assert!(parse("x = 3, y = 2049\n!\n").is_err());
        assert!(parse("x = 0, y = 3\n!\n").is_err());
        assert!(parse("x = 2, y = 2\n3o!\n").is_err());
    }

    #[test]
    fn accepted_conway_spellings_have_no_notice() {
        for rule in ["B3/S23", "b3/s23", "23/3", "Life"] {
            let r = parse(&format!("x = 3, y = 3, rule = {rule}\nbob$2bo$3o!\n")).unwrap();
            assert!(r.notices.is_empty(), "{rule}");
            assert_eq!(r.rule, rule);
        }
    }

    #[test]
    fn other_rule_is_kept_for_the_host_to_compare() {
        let r = parse("x = 3, y = 3, rule = B36/S23\nbob$2bo$3o!\n").unwrap();
        assert_eq!(r.pattern, glider());
        assert_eq!(r.rule, "B36/S23");
        assert!(r.notices.is_empty());
        assert_eq!(rule_of(&r.rule, Tiling::Square).unwrap().to_string(), "B36/S23");
        assert!(rule_of("Generations", Tiling::Square).is_err());
    }

    #[test]
    fn a_hexagonal_body_reads_with_its_offset_and_default_rule() {
        let r = parse_as("x = 3, y = 3\nbob$obo$bo!\n", Tiling::Hexagonal, true).unwrap();
        assert_eq!((r.pattern.tiling(), r.pattern.offset()), (Tiling::Hexagonal, true));
        assert_eq!(r.rule, Tiling::Hexagonal.default_rule().to_string());
    }

    #[test]
    fn bounded_grid_suffixes_are_read() {
        for (suffix, kind) in [("T100,80", 'T'), ("P100,80", 'P'), ("K100,80", 'K')] {
            let r = parse(&format!("x = 3, y = 3, rule = B3/S23:{suffix}\nbob$2bo$3o!\n")).unwrap();
            assert_eq!(r.bounded, Some(Bounded { kind, width: 100, height: 80 }));
            assert_eq!(r.rule, "B3/S23");
            assert!(r.notices.is_empty());
        }
        let r = parse("x = 3, y = 3, rule = B3/S23:T0,80\n!\n").unwrap();
        assert_eq!(r.bounded, None);
        assert_eq!(r.notices.len(), 1);
    }

    #[test]
    fn whole_universe_output_carries_the_suffix() {
        let b = Bounded { kind: 'T', width: 3, height: 3 };
        let text = write(&glider(), life(), None, None, Some(b));
        assert!(text.contains("rule = B3/S23:T3,3\n"), "{text}");
        assert_eq!(parse(&text).unwrap().bounded, Some(b));
    }

    #[test]
    fn golly_samples_load() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/rle");
        let mut seen = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let r = parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(r.pattern.live_count() > 0, "{}", path.display());
            seen += 1;
        }
        assert!(seen >= 2);
    }
}
