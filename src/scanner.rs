use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::collections::VecDeque;

// Умный распределитель: решает, как читать файл
fn read_safe_content(path: &Path) -> io::Result<String> {
    let is_log = path.extension().and_then(|s| s.to_str()) == Some("log");

    if is_log {
        read_head_and_tail(path, 5000, 5000)
    } else {
        read_head_only(path, 20_000)
    }
}

fn read_head_only(path: &Path, limit: usize) -> io::Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut content = String::new();

    for (i, line_result) in reader.lines().enumerate() {
        if i >= limit {
            content.push_str("\n... [GistFind: File truncated to save memory] ...\n");
            break;
        }
        content.push_str(&line_result?);
        content.push('\n');
    }
    Ok(content)
}

fn read_head_and_tail(path: &Path, head_limit: usize, tail_limit: usize) -> io::Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut head = Vec::new();
    let mut tail_ring = VecDeque::with_capacity(tail_limit);
    let mut total_lines = 0;

    for line_result in reader.lines() {
        let line = line_result?;
        total_lines += 1;

        if total_lines <= head_limit {
            head.push(line);
        } else {
            if tail_ring.len() == tail_limit {
                tail_ring.pop_front(); 
            }
            tail_ring.push_back(line); 
        }
    }

    if total_lines <= head_limit {
        return Ok(head.join("\n"));
    }

    if total_lines <= head_limit + tail_limit {
        let mut full_content = head;
        let skip_count = head_limit.saturating_sub(total_lines - tail_ring.len());
        full_content.extend(tail_ring.into_iter().skip(skip_count));
        return Ok(full_content.join("\n"));
    }

    let mut result = head.join("\n");
    result.push_str(&format!(
        "\n\n--- [GistFind: Skipped {} lines in middle] ---\n\n",
        total_lines - head_limit - tail_ring.len()
    ));
    
    let tail_vec: Vec<String> = tail_ring.into();
    result.push_str(&tail_vec.join("\n"));

    Ok(result)
}

pub fn scan_directory<F>(dir: &Path, f: &mut F) -> io::Result<()>
where
    F: FnMut(&Path, &str),
{
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                scan_directory(&path, f)?;
            } else if path.is_file() {
                if is_supported_file(&path) {
                    match read_safe_content(&path) {
                        Ok(content) => f(&path,&content),
                        Err(e) => eprintln!("[WARN] Can not file {}: {}", path.display(), e),
                    }
                }else {
                    eprintln!("[SKIP] Unsupported file type: {}", path.display());
                }
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
