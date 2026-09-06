use std::fs;
use std::io;
use std::path::Path;

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
                    match fs::read_to_string(&path) {
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
