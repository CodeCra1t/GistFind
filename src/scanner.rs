use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use crate::ignore::IgnoreRules;

/// Upper bound on the bytes read from one file in head-only mode and for the
/// head part of log files. Keeps single huge lines from exhausting memory.
const MAX_HEAD_BYTES: usize = 2_000_000;

/// Iterates over lines, replacing invalid UTF-8 instead of failing the whole file.
fn lossy_lines<R: BufRead>(reader: R) -> impl Iterator<Item = io::Result<String>> {
    reader.split(b'\n').map(|r| {
        r.map(|bytes| {
            String::from_utf8_lossy(&bytes)
                .trim_end_matches('\r')
                .to_string()
        })
    })
}

fn read_safe_content(path: &Path) -> io::Result<String> {
    let is_log = path.extension().and_then(|s| s.to_str()) == Some("log");

    if is_log {
        read_head_and_tail(path, 5000, 5000)
    } else {
        read_head_only(path, 20_000)
    }
}

fn read_head_only(path: &Path, limit: usize) -> io::Result<String> {
    let reader = BufReader::new(File::open(path)?);
    let mut content = String::new();

    for (i, line) in lossy_lines(reader).enumerate() {
        if i >= limit || content.len() >= MAX_HEAD_BYTES {
            break;
        }
        content.push_str(&line?);
        content.push('\n');
    }
    Ok(content)
}

fn read_head_and_tail(path: &Path, head_limit: usize, tail_limit: usize) -> io::Result<String> {
    let reader = BufReader::new(File::open(path)?);

    let mut head: Vec<String> = Vec::new();
    let mut head_bytes = 0usize;
    let mut tail: VecDeque<String> = VecDeque::with_capacity(tail_limit);
    let mut skipped = 0usize;

    for line in lossy_lines(reader) {
        let line = line?;
        if head.len() < head_limit && head_bytes + line.len() <= MAX_HEAD_BYTES {
            head_bytes += line.len();
            head.push(line);
        } else {
            if tail.len() == tail_limit {
                tail.pop_front();
                skipped += 1;
            }
            tail.push_back(line);
        }
    }

    if skipped == 0 {
        // Whole file fits into head + tail: nothing was skipped.
        head.extend(tail);
        return Ok(head.join("\n"));
    }

    let mut result = head.join("\n");
    result.push_str(&format!(
        "\n\n--- [GistFind: skipped {} lines in middle] ---\n\n",
        skipped
    ));
    let tail_lines: Vec<String> = tail.into_iter().collect();
    result.push_str(&tail_lines.join("\n"));
    Ok(result)
}

/// Scans `root` recursively. Entries matched by `.gistignore` in `root` are skipped;
/// ignored folders are not entered at all.
pub fn scan_directory<F>(root: &Path, f: &mut F) -> io::Result<()>
where
    F: FnMut(&Path, &str),
{
    let rules = IgnoreRules::load(root);
    walk(root, root, &rules, f)
}

/// Path of `path` relative to `root`, with `/` as separator on every platform.
fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn walk<F>(root: &Path, dir: &Path, rules: &IgnoreRules, f: &mut F) -> io::Result<()>
where
    F: FnMut(&Path, &str),
{
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("[WARN] Can not read directory {}: {}", dir.display(), e);
            return Ok(());
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("[WARN] Bad directory entry in {}: {}", dir.display(), e);
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[WARN] Can not stat {}: {}", path.display(), e);
                continue;
            }
        };

        // Symlinks: follow links to files, but never descend into linked
        // directories, because a link back to an ancestor would loop forever.
        let (is_dir, is_file) = if file_type.is_symlink() {
            match fs::metadata(&path) {
                Ok(meta) => (false, meta.is_file()),
                Err(_) => (false, false), // broken link
            }
        } else {
            (file_type.is_dir(), file_type.is_file())
        };

        if rules.is_ignored(&relative_path(root, &path), is_dir) {
            continue;
        }

        if is_dir {
            walk(root, &path, rules, f)?;
        } else if is_file && is_supported_file(&path) {
            match read_safe_content(&path) {
                Ok(content) => f(&path, &content),
                Err(e) => eprintln!("[WARN] Can not index file {}: {}", path.display(), e),
            }
        }
    }
    Ok(())
}

fn is_supported_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        matches!(ext, "txt" | "md" | "rs" | "json" | "toml" | "log")
    } else {
        false
    }
}
