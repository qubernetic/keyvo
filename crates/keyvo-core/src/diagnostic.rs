//! Validation errors of the configuration files, with file, line and column
//! (spec §7.1).

use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// A position in a text file, both 1-based; the column counts characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Line, starting at 1.
    pub line: usize,
    /// Column in characters, starting at 1.
    pub column: usize,
}

impl Position {
    /// The position of byte `offset` in `text`. An offset past the end or
    /// inside a character is clamped to the character that contains it.
    #[must_use]
    pub fn of_offset(text: &str, offset: usize) -> Self {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let before = &text[..offset];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        Self {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

/// One problem in one configuration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The file the problem is in.
    pub path: PathBuf,
    /// Where in the file; `None` when the parser could not point at a place.
    pub position: Option<Position>,
    /// What is wrong, without the position.
    pub message: String,
}

impl Diagnostic {
    /// A diagnostic at the start of byte range `span` of `text`.
    #[must_use]
    pub fn at(path: &Path, text: &str, span: Option<Range<usize>>, message: String) -> Self {
        Self {
            path: path.to_path_buf(),
            position: span.map(|span| Position::of_offset(text, span.start)),
            message,
        }
    }

    /// A diagnostic from a TOML parse or deserialization error.
    #[must_use]
    pub fn from_toml(path: &Path, text: &str, error: &toml::de::Error) -> Self {
        Self::at(
            path,
            text,
            error.span(),
            error.message().trim_end().to_owned(),
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.position {
            Some(Position { line, column }) => write!(
                f,
                "{}:{line}:{column}: {}",
                self.path.display(),
                self.message
            ),
            None => write!(f, "{}: {}", self.path.display(), self.message),
        }
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_maps_to_line_and_character_column() {
        let text = "a = 1\nbé = 2\n";
        assert_eq!(
            Position::of_offset(text, 0),
            Position { line: 1, column: 1 }
        );
        assert_eq!(
            Position::of_offset(text, 6),
            Position { line: 2, column: 1 }
        );
        // "bé = " is 6 bytes but 5 characters.
        let two = text.find('2').unwrap();
        assert_eq!(
            Position::of_offset(text, two),
            Position { line: 2, column: 6 }
        );
    }

    #[test]
    fn offset_inside_a_character_or_past_the_end_is_clamped() {
        let text = "é";
        assert_eq!(
            Position::of_offset(text, 1),
            Position { line: 1, column: 1 }
        );
        assert_eq!(
            Position::of_offset(text, 99),
            Position { line: 1, column: 2 }
        );
    }

    #[test]
    fn display_names_file_line_and_column() {
        let d = Diagnostic::at(Path::new("p.toml"), "x\ny", Some(2..3), "bad".into());
        assert_eq!(d.to_string(), "p.toml:2:1: bad");
        let d = Diagnostic::at(Path::new("p.toml"), "", None, "bad".into());
        assert_eq!(d.to_string(), "p.toml: bad");
    }
}
