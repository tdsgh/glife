//! The built-in and personal pattern libraries.

pub mod builtin;
pub mod personal;

/// A library file that could not be offered, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: String,
    pub reason: String,
}
