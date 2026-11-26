use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// Name of the ignore file, looked up in the root of the scanned folder.
pub const IGNORE_FILE_NAME: &str = ".gistignore";

/// One line of a `.gistignore` file.
struct Rule {
    /// Pattern without the leading `!`, trailing `/` or leading `/`.
    pattern: String,
    /// `!pattern` re-includes something an earlier rule excluded.
    negated: bool,
    /// `pattern/` only matches directories.
    dir_only: bool,
    /// Patterns with a `/` are matched against the path relative to the
    /// scan root. Others are matched against the bare file or folder name.
    anchored: bool,
}

#[derive(Default)]
pub struct IgnoreRules {
    rules: Vec<Rule>,
}

impl IgnoreRules {
    /// Reads `.gistignore` from the scan root. A missing file means no rules.
    /// A file that cannot be read is reported and treated as no rules.
    pub fn load(root: &Path) -> Self {
        let path = root.join(IGNORE_FILE_NAME);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                eprintln!("[WARN] Can not read {}: {}", path.display(), e);
                return Self::default();
            }
        };

        let rules: Vec<Rule> = text.lines().filter_map(Rule::parse).collect();
        if !rules.is_empty() {
            println!("Loaded {} ignore rules from {}", rules.len(), path.display());
        }
        Self { rules }
    }

    /// `relative` is the path below the scan root, using `/` as separator.
    /// Later rules override earlier ones, as in `.gitignore`.
    pub fn is_ignored(&self, relative: &str, is_dir: bool) -> bool {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        let mut ignored = false;

        for rule in &self.rules {
            if rule.dir_only && !is_dir {
                continue;
            }
            let subject = if rule.anchored { relative } else { name };
            if glob_match(rule.pattern.as_bytes(), subject.as_bytes()) {
                ignored = !rule.negated;
            }
        }
        ignored
    }
}

impl Rule {
    fn parse(line: &str) -> Option<Rule> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let (negated, line) = match line.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        let dir_only = line.ends_with('/');
        let line = line.trim_end_matches('/');
        // A `/` anywhere (including a leading one) anchors the pattern to the root.
        let anchored = line.contains('/');
        let pattern = line.trim_start_matches('/').to_string();

        if pattern.is_empty() {
            return None;
        }
        Some(Rule {
            pattern,
            negated,
            dir_only,
            anchored,
        })
    }
}

/// Glob matching on bytes:
/// - `*` matches any run of characters except `/`
/// - `?` matches one character except `/`
/// - `**` matches any run of characters including `/`
///
/// Non-ASCII characters only match literally or through `**`, because `?`
/// works on single bytes.
fn glob_match(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),

        Some((b'*', rest)) if rest.first() == Some(&b'*') => {
            // `**/` may also match zero directories, so skip the slash too.
            let rest = &rest[1..];
            let rest = if rest.first() == Some(&b'/') { &rest[1..] } else { rest };
            (0..=text.len()).any(|i| glob_match(rest, &text[i..]))
        }

        Some((b'*', rest)) => {
            // Try every length, but never let the match cross a `/`.
            (0..=text.len())
                .take_while(|&i| i == 0 || text[i - 1] != b'/')
                .any(|i| glob_match(rest, &text[i..]))
        }

        Some((b'?', rest)) => {
            !text.is_empty() && text[0] != b'/' && glob_match(rest, &text[1..])
        }

        Some((&c, rest)) => !text.is_empty() && text[0] == c && glob_match(rest, &text[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    fn m(p: &str, t: &str) -> bool {
        glob_match(p.as_bytes(), t.as_bytes())
    }

    #[test]
    fn star_does_not_cross_slash() {
        assert!(m("*.log", "app.log"));
        assert!(!m("*.log", "logs/app.log"));
        assert!(m("a*", "abc"));
    }

    #[test]
    fn double_star_crosses_slash() {
        assert!(m("**/*.tmp", "a/b/c.tmp"));
        assert!(m("**/*.tmp", "c.tmp"));
        assert!(m("docs/**", "docs/x/y.md"));
    }

    #[test]
    fn question_mark_matches_one_char() {
        assert!(m("a?c", "abc"));
        assert!(!m("a?c", "ac"));
        assert!(!m("a?c", "a/c"));
    }

    #[test]
    fn rules_parse_and_apply() {
        use super::{IgnoreRules, Rule};
        let rules = IgnoreRules {
            rules: ["node_modules/", "*.log", "!keep.log", "/drafts", "# comment", ""]
                .iter()
                .filter_map(|l| Rule::parse(l))
                .collect(),
        };
        assert!(rules.is_ignored("node_modules", true));
        assert!(!rules.is_ignored("node_modules", false));
        assert!(rules.is_ignored("x/app.log", false));
        assert!(!rules.is_ignored("x/keep.log", false));
        assert!(rules.is_ignored("drafts", true));
        assert!(!rules.is_ignored("sub/drafts", true));
    }
}
