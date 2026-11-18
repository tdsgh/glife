//! The personal library: a folder of pattern files, one per pattern.

use std::path::{Path, PathBuf};

use glife_engine::{Pattern, Rule};

use super::Problem;
use crate::glife_file::write_atomic;
use crate::pattern_file::{self, PatternFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Another pattern already has the name, ignoring case.
    NameTaken,
    NotFound,
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NameTaken => f.write_str("A pattern with that name already exists."),
            Error::NotFound => f.write_str("There is no pattern with that name."),
            Error::Other(m) => f.write_str(m),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// The file's name inside the library folder.
    pub file: String,
    pub name: String,
    pub comment: Option<String>,
    pub pattern: Pattern,
    /// The rule the pattern was made under.
    pub rule: Rule,
    pub live: u32,
    pub created: Option<String>,
    pub changed: Option<String>,
}

pub struct Library {
    dir: PathBuf,
}

/// A file name for `name`: letters, digits, space, `-`, `_` and `.` stay,
/// anything else becomes `_`.
fn file_stem(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '_' })
        .collect();
    let mut stem: String = mapped.trim_matches(|c| c == ' ' || c == '.').chars().take(100).collect();
    if stem.is_empty() {
        stem = "pattern".into();
    }
    const RESERVED: [&str; 8] = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "LPT1", "LPT2"];
    if RESERVED.contains(&stem.to_ascii_uppercase().as_str()) {
        stem.push('_');
    }
    stem
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl Library {
    /// The library in `dir`, which is created when it does not exist.
    pub fn open(dir: &Path) -> Result<Library, String> {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Could not open the pattern library folder: {e}"))?;
        Ok(Library { dir: dir.to_path_buf() })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every readable pattern, by name, and the files that could not be read.
    /// Two files with the same name: the later file name is the problem.
    pub fn list(&self) -> (Vec<Entry>, Vec<Problem>) {
        let mut entries: Vec<Entry> = Vec::new();
        let mut problems = Vec::new();
        let mut files: Vec<PathBuf> = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "toml"))
                .collect(),
            Err(e) => {
                problems.push(Problem { file: self.dir.display().to_string(), reason: format!("Could not read the folder: {e}") });
                return (entries, problems);
            }
        };
        files.sort();
        for path in files {
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            match read_entry(&path, file.clone()) {
                Ok(entry) => {
                    if entries.iter().any(|e| same_name(&e.name, &entry.name)) {
                        problems.push(Problem {
                            file,
                            reason: format!("Another file already holds a pattern named {:?}.", entry.name),
                        });
                    } else {
                        entries.push(entry);
                    }
                }
                Err(reason) => problems.push(Problem { file, reason }),
            }
        }
        entries.sort_by_key(|e| e.name.to_lowercase());
        (entries, problems)
    }

    fn find(&self, name: &str) -> Option<Entry> {
        self.list().0.into_iter().find(|e| same_name(&e.name, name))
    }

    fn check_name(name: &str) -> Result<&str, Error> {
        let name = name.trim();
        if name.is_empty() || name.contains(['\r', '\n']) {
            return Err(Error::Other("A pattern name must be one non-empty line.".into()));
        }
        Ok(name)
    }

    /// A free `<stem>.toml`, or `<stem> 2.toml` and so on.
    fn new_file_name(&self, name: &str) -> String {
        let stem = file_stem(name);
        let mut n = 1;
        loop {
            let candidate =
                if n == 1 { format!("{stem}.toml") } else { format!("{stem} {n}.toml") };
            if !self.dir.join(&candidate).exists() {
                return candidate;
            }
            n += 1;
        }
    }

    fn write(&self, file_name: &str, file: &PatternFile) -> Result<(), Error> {
        let text = file.render().map_err(Error::Other)?;
        write_atomic(&self.dir.join(file_name), text.as_bytes())
            .map_err(|e| Error::Other(format!("Could not write the pattern: {e}")))
    }

    /// Saves `pattern`, made under `rule`, as `name`, with its grid and offset.
    /// A name already taken, on any grid, fails with `NameTaken` unless
    /// `replace`, which keeps the old comment when `comment` is empty, keeps
    /// `created` and updates `changed`.
    pub fn save(
        &self,
        name: &str,
        comment: Option<&str>,
        pattern: &Pattern,
        rule: Rule,
        replace: bool,
    ) -> Result<(), Error> {
        let name = Self::check_name(name)?;
        let comment = comment.filter(|c| !c.is_empty());
        let new = PatternFile::personal(pattern, rule, name, comment);
        match self.find(name) {
            None => self.write(&self.new_file_name(name), &new),
            Some(_) if !replace => Err(Error::NameTaken),
            Some(old) => {
                let mut file = read_file(&self.dir.join(&old.file)).map_err(Error::Other)?;
                file.name = name.into();
                file.tiling = new.tiling;
                file.offset = new.offset;
                file.rle = new.rle;
                if let Some(c) = comment {
                    file.comment = Some(c.into());
                }
                file.changed = Some(pattern_file::now());
                self.write(&old.file, &file)
            }
        }
    }

    /// Renames `name` to `new_name`. With `replace`, a different pattern
    /// already called `new_name` is deleted.
    pub fn rename(&self, name: &str, new_name: &str, replace: bool) -> Result<(), Error> {
        let new_name = Self::check_name(new_name)?;
        let entry = self.find(name).ok_or(Error::NotFound)?;
        let other = self.find(new_name).filter(|o| o.file != entry.file);
        if other.is_some() && !replace {
            return Err(Error::NameTaken);
        }
        let mut file = read_file(&self.dir.join(&entry.file)).map_err(Error::Other)?;
        file.name = new_name.into();
        file.changed = Some(pattern_file::now());
        self.write(&entry.file, &file)?;
        if let Some(other) = other {
            std::fs::remove_file(self.dir.join(other.file))
                .map_err(|e| Error::Other(format!("Could not remove the replaced pattern: {e}")))?;
        }
        Ok(())
    }

    /// Sets or, when empty, removes the comment.
    pub fn set_comment(&self, name: &str, comment: &str) -> Result<(), Error> {
        let entry = self.find(name).ok_or(Error::NotFound)?;
        let mut file = read_file(&self.dir.join(&entry.file)).map_err(Error::Other)?;
        file.comment = Some(comment.to_string()).filter(|c| !c.is_empty());
        file.changed = Some(pattern_file::now());
        self.write(&entry.file, &file)
    }

    /// Whether a pattern was deleted.
    pub fn delete(&self, name: &str) -> Result<bool, Error> {
        let Some(entry) = self.find(name) else { return Ok(false) };
        std::fs::remove_file(self.dir.join(entry.file))
            .map_err(|e| Error::Other(format!("Could not delete the pattern: {e}")))?;
        Ok(true)
    }
}

fn read_file(path: &Path) -> Result<PatternFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read the file: {e}"))?;
    PatternFile::parse(&text)
}

fn read_entry(path: &Path, file_name: String) -> Result<Entry, String> {
    let file = read_file(path)?;
    let (pattern, rule) = file.cells()?;
    Ok(Entry {
        file: file_name,
        name: file.name,
        comment: file.comment,
        live: pattern.live_count(),
        pattern,
        rule,
        created: file.created,
        changed: file.changed,
    })
}

#[cfg(test)]
mod tests {
    use glife_engine::Tiling;

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

    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(&dir.path().join("patterns")).unwrap();
        (dir, lib)
    }

    #[test]
    fn save_and_list() {
        let (_d, lib) = library();
        assert!(lib.list().0.is_empty());
        lib.save("Zebra", Some("stripes\nof two lines"), &glider(), life(), false).unwrap();
        lib.save("apple", None, &Pattern::new(4, 2, Tiling::Square, false), life(), false).unwrap();
        let (entries, problems) = lib.list();
        assert!(problems.is_empty());
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["apple", "Zebra"]);
        assert_eq!(entries[1].pattern, glider());
        assert_eq!(entries[1].comment.as_deref(), Some("stripes\nof two lines"));
        assert_eq!(entries[1].live, 5);
        assert_eq!((entries[0].pattern.width(), entries[0].pattern.height()), (4, 2));
        assert!(entries[1].created.is_some() && entries[1].created == entries[1].changed);
    }

    #[test]
    fn same_name_without_replace_fails_ignoring_case() {
        let (_d, lib) = library();
        lib.save("Glider", None, &glider(), life(), false).unwrap();
        assert_eq!(lib.save("glider", None, &Pattern::new(2, 2, Tiling::Square, false), life(), false), Err(Error::NameTaken));
        assert_eq!(lib.list().0[0].pattern, glider());
    }

    #[test]
    fn replace_keeps_comment_and_created_and_updates_changed() {
        let (_d, lib) = library();
        lib.save("Glider", Some("mine"), &glider(), life(), false).unwrap();
        let before = lib.list().0.remove(0);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        lib.save("GLIDER", None, &Pattern::new(2, 2, Tiling::Square, false), life(), true).unwrap();
        let (entries, _) = lib.list();
        assert_eq!(entries.len(), 1);
        let after = &entries[0];
        assert_eq!(after.name, "GLIDER");
        assert_eq!(after.comment.as_deref(), Some("mine"));
        assert_eq!(after.created, before.created);
        assert_ne!(after.changed, before.changed);
        assert_eq!(after.pattern, Pattern::new(2, 2, Tiling::Square, false));
        lib.save("GLIDER", Some("new"), &glider(), life(), true).unwrap();
        assert_eq!(lib.list().0[0].comment.as_deref(), Some("new"));
    }

    #[test]
    fn rename_to_a_taken_name_fails_unless_replacing() {
        let (_d, lib) = library();
        lib.save("A", None, &glider(), life(), false).unwrap();
        lib.save("B", None, &Pattern::new(2, 2, Tiling::Square, false), life(), false).unwrap();
        assert_eq!(lib.rename("A", "b", false), Err(Error::NameTaken));
        assert_eq!(lib.rename("Nope", "C", false), Err(Error::NotFound));
        lib.rename("A", "C", false).unwrap();
        lib.rename("C", "c2", false).unwrap();
        lib.rename("c2", "B", true).unwrap();
        let (entries, _) = lib.list();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "B");
        assert_eq!(entries[0].pattern, glider());
        // A case-only change is not a clash with itself.
        lib.rename("B", "b", false).unwrap();
        assert_eq!(lib.list().0[0].name, "b");
    }

    #[test]
    fn comment_and_delete() {
        let (_d, lib) = library();
        lib.save("A", Some("x"), &glider(), life(), false).unwrap();
        lib.set_comment("a", "line 1\nline 2").unwrap();
        assert_eq!(lib.list().0[0].comment.as_deref(), Some("line 1\nline 2"));
        lib.set_comment("A", "").unwrap();
        assert_eq!(lib.list().0[0].comment, None);
        assert_eq!(lib.delete("A"), Ok(true));
        assert_eq!(lib.delete("A"), Ok(false));
        assert!(lib.list().0.is_empty());
    }

    #[test]
    fn file_names_are_safe_and_collisions_get_a_number() {
        let (_d, lib) = library();
        lib.save("a/b:c", None, &glider(), life(), false).unwrap();
        lib.save("a_b:c", None, &glider(), life(), false).unwrap();
        lib.save("CON", None, &glider(), life(), false).unwrap();
        let (entries, _) = lib.list();
        let mut files: Vec<_> = entries.iter().map(|e| e.file.as_str()).collect();
        files.sort();
        assert_eq!(files, ["CON_.toml", "a_b_c 2.toml", "a_b_c.toml"]);
        assert!(lib.save("  ", None, &glider(), life(), false).is_err());
        assert!(lib.save("two\nlines", None, &glider(), life(), false).is_err());
    }

    #[test]
    fn unreadable_file_is_listed_and_not_touched() {
        let (_d, lib) = library();
        lib.save("Good", None, &glider(), life(), false).unwrap();
        let bad = lib.dir().join("bad.toml");
        std::fs::write(&bad, "name = [ broken").unwrap();
        let (entries, problems) = lib.list();
        assert_eq!(entries.len(), 1);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].file, "bad.toml");
        lib.save("Another", None, &glider(), life(), false).unwrap();
        lib.delete("Good").unwrap();
        assert_eq!(std::fs::read_to_string(&bad).unwrap(), "name = [ broken");
    }

    #[test]
    fn committed_fixture_loads() {
        let (_d, lib) = library();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/pattern-v1.toml");
        std::fs::copy(fixture, lib.dir().join("sample.toml")).unwrap();
        let (entries, problems) = lib.list();
        assert!(problems.is_empty());
        assert_eq!(entries[0].name, "Sample glider");
        assert_eq!(entries[0].pattern, glider());
        assert_eq!(entries[0].created.as_deref(), Some("2026-01-02T03:04:05Z"));
    }
}
