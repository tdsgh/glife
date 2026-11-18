//! The grids a universe can be laid out on, and the birth/survival rules they
//! run.

use std::fmt;

/// The shape of the cells and which cells are neighbors. Every grid stores
/// cell `(x, y)` the same way; only the neighborhoods differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tiling {
    /// Squares with 8 neighbors.
    Square = 0,
    /// Pointy-top hexagons with 6 neighbors; odd rows sit half a cell right.
    Hexagonal = 1,
    /// Triangles with 12 neighbors: 3 across an edge and 9 at a corner. Cell
    /// `(x, y)` points up when `x + y` is even.
    Triangular12 = 2,
    /// Triangles with the 3 neighbors across an edge, laid out as
    /// `Triangular12`.
    Triangular3 = 3,
}

impl Tiling {
    pub const ALL: [Tiling; 4] = [
        Tiling::Square,
        Tiling::Hexagonal,
        Tiling::Triangular12,
        Tiling::Triangular3,
    ];

    /// The id used in files and commands.
    pub fn id(self) -> &'static str {
        match self {
            Tiling::Square => "square",
            Tiling::Hexagonal => "hexagonal",
            Tiling::Triangular12 => "triangular-12",
            Tiling::Triangular3 => "triangular-3",
        }
    }

    /// The tiling with this id.
    pub fn parse(id: &str) -> Option<Tiling> {
        Tiling::ALL.into_iter().find(|t| t.id() == id)
    }

    /// The display name.
    pub fn name(self) -> &'static str {
        match self {
            Tiling::Square => "Square",
            Tiling::Hexagonal => "Hexagonal",
            Tiling::Triangular12 => "Triangular, 12 neighbors",
            Tiling::Triangular3 => "Triangular, 3 neighbors",
        }
    }

    /// How many neighbors every cell has.
    pub fn neighbors(self) -> u32 {
        match self {
            Tiling::Square => 8,
            Tiling::Hexagonal => 6,
            Tiling::Triangular12 => 12,
            Tiling::Triangular3 => 3,
        }
    }

    /// The rule a new universe on this grid starts with.
    pub fn default_rule(self) -> Rule {
        self.presets()[0].1
    }

    /// The named rules offered for this grid, the default first.
    pub fn presets(self) -> &'static [(&'static str, Rule)] {
        const SQUARE: &[(&str, Rule)] = &[
            ("Life", Rule::LIFE),
            ("HighLife", rule(&[3, 6], &[2, 3])),
            ("Seeds", rule(&[2], &[])),
            ("Day & Night", rule(&[3, 6, 7, 8], &[3, 4, 6, 7, 8])),
        ];
        const HEXAGONAL: &[(&str, Rule)] = &[
            ("Bays hexagonal", rule(&[2], &[3, 5])),
            ("Conway's rule", Rule::LIFE),
        ];
        const TRIANGULAR_12: &[(&str, Rule)] = &[
            ("Bays 4546", rule(&[4, 5, 6], &[4, 5])),
            ("Bays 4644", rule(&[4], &[4, 5, 6])),
        ];
        const TRIANGULAR_3: &[(&str, Rule)] = &[
            ("Triangle Life", rule(&[2], &[0, 1, 3])),
            ("Lone triangles die", rule(&[2], &[2, 3])),
        ];
        match self {
            Tiling::Square => SQUARE,
            Tiling::Hexagonal => HEXAGONAL,
            Tiling::Triangular12 => TRIANGULAR_12,
            Tiling::Triangular3 => TRIANGULAR_3,
        }
    }

    /// Whether wrapping edges need an even width and an even height.
    pub fn wrap_needs_even(self) -> (bool, bool) {
        match self {
            Tiling::Square => (false, false),
            Tiling::Hexagonal => (false, true),
            Tiling::Triangular12 | Tiling::Triangular3 => (true, true),
        }
    }
}

/// A birth/survival rule: bit `n` of `birth` set when a dead cell with `n`
/// live neighbors comes alive, bit `n` of `survival` set when a live cell
/// with `n` live neighbors stays alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rule {
    pub birth: u16,
    pub survival: u16,
}

const fn rule(birth: &[u32], survival: &[u32]) -> Rule {
    const fn mask(counts: &[u32]) -> u16 {
        let mut mask = 0;
        let mut i = 0;
        while i < counts.len() {
            mask |= 1 << counts[i];
            i += 1;
        }
        mask
    }
    Rule {
        birth: mask(birth),
        survival: mask(survival),
    }
}

impl Rule {
    /// B3/S23.
    pub(crate) const LIFE: Rule = rule(&[3], &[2, 3]);

    /// Reads `B…/S…`, case-insensitive. A list of counts without a comma has
    /// one digit per count (`B3/S23`); a list with a comma has numbers
    /// separated by commas (`B4,10,11/S5,12`), and may end with one, so that a
    /// lone count from 10 up reads `S12,`. Refuses a rule `tiling` cannot run.
    pub fn parse(text: &str, tiling: Tiling) -> Result<Rule, RuleError> {
        let text = text.trim();
        let notation = || RuleError::Notation(text.to_string());
        let lower = text.to_ascii_lowercase();
        let (birth, survival) = lower
            .strip_prefix('b')
            .and_then(|rest| rest.split_once("/s"))
            .ok_or_else(notation)?;
        let neighbors = tiling.neighbors();
        let mask = |counts: &str| {
            let numbers: Vec<&str> = if counts.contains(',') {
                counts
                    .strip_suffix(',')
                    .unwrap_or(counts)
                    .split(',')
                    .collect()
            } else {
                counts.matches(|_| true).collect()
            };
            numbers.into_iter().try_fold(0u16, |mask, number| {
                if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(notation());
                }
                match number.parse::<u32>() {
                    Ok(count) if count <= neighbors => Ok(mask | 1 << count),
                    Ok(count) => Err(RuleError::Count { count, neighbors }),
                    Err(_) => Err(notation()),
                }
            })
        };
        let rule = Rule {
            birth: mask(birth)?,
            survival: mask(survival)?,
        };
        rule.check(tiling)?;
        Ok(rule)
    }

    /// Whether `tiling` can run the rule: no count above its neighbor count
    /// and no birth on 0.
    pub(crate) fn check(self, tiling: Tiling) -> Result<(), RuleError> {
        let neighbors = tiling.neighbors();
        let beyond = (self.birth | self.survival) >> (neighbors + 1);
        if beyond != 0 {
            return Err(RuleError::Count {
                count: neighbors + 16 - beyond.leading_zeros(),
                neighbors,
            });
        }
        if self.birth & 1 != 0 {
            return Err(RuleError::BirthOnZero);
        }
        Ok(())
    }
}

/// Writes `B…/S…` as [`Rule::parse`] reads it: one digit per count, or,
/// when any count is 10 or more, numbers separated by commas.
impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let commas = (self.birth | self.survival) >> 10 != 0;
        let counts = |mask: u16| -> String {
            let numbers: Vec<String> = (0..16)
                .filter(|n| mask >> n & 1 == 1)
                .map(|n| n.to_string())
                .collect();
            match numbers.as_slice() {
                [lone] if lone.len() > 1 => format!("{lone},"),
                _ if commas => numbers.join(","),
                _ => numbers.concat(),
            }
        };
        write!(f, "B{}/S{}", counts(self.birth), counts(self.survival))
    }
}

/// A rule that is not in birth/survival notation or that a grid cannot run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    /// The text is not `B…/S…`.
    Notation(String),
    /// The rule names a count above the grid's neighbor count.
    Count { count: u32, neighbors: u32 },
    /// The rule gives birth on 0 neighbors.
    BirthOnZero,
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleError::Notation(text) => write!(
                f,
                "\"{text}\" is not a rule in B…/S… notation, such as B3/S23"
            ),
            RuleError::Count { count, neighbors } => write!(
                f,
                "a cell on this grid has {neighbors} neighbors, so a rule cannot count {count}"
            ),
            RuleError::BirthOnZero => f.write_str("birth on 0 neighbors is not supported"),
        }
    }
}

impl std::error::Error for RuleError {}
