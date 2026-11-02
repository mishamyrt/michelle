use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::ProviderKind;

/// Decode Git's C-quoted filenames, including octal escapes for UTF-8 bytes.
/// Unquoted or malformed paths are preserved verbatim.
pub fn unquote_path(path: &str) -> String {
    fn decode(path: &str) -> Option<String> {
        let quoted = path.strip_prefix('"')?.strip_suffix('"')?;
        let mut bytes = quoted.bytes();
        let mut output = Vec::with_capacity(quoted.len());
        while let Some(byte) = bytes.next() {
            output.push(match byte {
                b'"' => return None,
                b'\\' => match bytes.next()? {
                    b'a' => 7,
                    b'b' => 8,
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'v' => 11,
                    b'f' => 12,
                    b'r' => b'\r',
                    b'"' => b'"',
                    b'\\' => b'\\',
                    first @ b'0'..=b'3' => {
                        let second = bytes.next()?;
                        let third = bytes.next()?;
                        if !(b'0'..=b'7').contains(&second) || !(b'0'..=b'7').contains(&third) {
                            return None;
                        }
                        (first - b'0') * 64 + (second - b'0') * 8 + (third - b'0')
                    }
                    _ => return None,
                },
                other => other,
            });
        }
        String::from_utf8(output).ok()
    }

    decode(path).unwrap_or_else(|| path.to_owned())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BranchEntry {
    pub name: String,
    pub checked_out_elsewhere: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BranchSnapshot {
    pub repository: PathBuf,
    pub current: Option<String>,
    pub detached_head: Option<String>,
    pub default_branch: Option<String>,
    pub branches: Vec<BranchEntry>,
    pub additions: u64,
    pub deletions: u64,
}

impl BranchSnapshot {
    pub fn display_branch(&self) -> Option<&str> {
        self.current.as_deref().or(self.detached_head.as_deref())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommitSnapshot {
    pub branch: String,
    pub additions: u64,
    pub deletions: u64,
    pub staged_additions: u64,
    pub staged_deletions: u64,
    pub has_staged: bool,
    pub has_unstaged: bool,
    pub can_push: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentInvocation {
    pub provider: ProviderKind,
    pub binary: PathBuf,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreatedWorktree {
    pub path: PathBuf,
    pub branch: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_git_quoted_paths_as_utf8_bytes() {
        assert_eq!(
            unquote_path(r#""\320\232\320\276\320\274\320\260\320\275\320\264\320\260.md""#),
            "Команда.md"
        );
        assert_eq!(
            unquote_path(r#""Проекты/Geoportal/\320\232\320\260\321\200\321\202\320\260.md""#),
            "Проекты/Geoportal/Карта.md"
        );
        assert_eq!(
            unquote_path(r#""folder/\"name\"\\file\twith\ncontrols\a\b\v\f\r""#),
            "folder/\"name\"\\file\twith\ncontrols\u{7}\u{8}\u{b}\u{c}\r"
        );
    }

    #[test]
    fn preserves_unquoted_and_malformed_paths() {
        for path in [
            "src/lib.rs",
            "Проекты/Карта.md",
            r"literal\name\320\232.md",
            r#""incomplete\"#,
            r#""invalid\q""#,
            r#""\400""#,
            r#""\377""#,
            r#""\32""#,
            r#""\328""#,
        ] {
            assert_eq!(unquote_path(path), path);
        }
    }
}
