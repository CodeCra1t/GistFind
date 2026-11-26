mod config;
mod ignore;
mod scanner;
mod storage;

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use storage::engine::SearchEngine;

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Feeds documents into the engine and commits to disk when enough data has
/// accumulated. Errors are counted and logged, never silently dropped.
struct Indexer<'a> {
    engine: &'a mut SearchEngine,
    commit_threshold: usize,
    pending_bytes: usize,
    files: usize,
    documents: usize,
    errors: usize,
}

impl<'a> Indexer<'a> {
    fn new(engine: &'a mut SearchEngine, commit_threshold: usize) -> Self {
        Self {
            engine,
            commit_threshold,
            pending_bytes: 0,
            files: 0,
            documents: 0,
            errors: 0,
        }
    }

    fn add(&mut self, path: &str, content: &str) {
        match self.engine.add_document(path, content) {
            Ok(()) => {
                self.documents += 1;
                self.pending_bytes += content.len();
            }
            Err(e) => {
                self.errors += 1;
                eprintln!("[ERROR] Failed to index {}: {}", path, e);
            }
        }
        if self.pending_bytes >= self.commit_threshold {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.pending_bytes == 0 {
            return;
        }
        if let Err(e) = self.engine.commit() {
            self.errors += 1;
            eprintln!("[ERROR] Commit failed: {}", e);
        }
        // Reset even on failure, so one bad commit does not turn into an
        // error for every following file.
        self.pending_bytes = 0;
    }

    fn index_file(&mut self, path: &std::path::Path, content: &str) {
        let path_str = path.to_string_lossy();
        self.files += 1;
        if self.files % 1000 == 1 {
            println!("Indexing [{}] {} ...", self.files, path_str);
        }

        if content.len() <= self.commit_threshold {
            self.add(&path_str, content);
            return;
        }

        // Large file: split into line-aligned chunks so that one file never
        // exceeds the writer's memory budget.
        let chunk_size = (self.commit_threshold / 2).max(1);
        let mut buffer = String::with_capacity(chunk_size);
        let mut chunk_index = 1;

        for line in content.lines() {
            buffer.push_str(line);
            buffer.push('\n');
            if buffer.len() >= chunk_size {
                let chunk_path = format!("{}#chunk{}", path_str, chunk_index);
                self.add(&chunk_path, &buffer);
                buffer.clear();
                chunk_index += 1;
            }
        }
        if !buffer.is_empty() {
            let chunk_path = format!("{}#chunk{}", path_str, chunk_index);
            self.add(&chunk_path, &buffer);
        }
    }
}

/// Holds the current folder to scan and the index location. The scan folder
/// can be changed at runtime with `scan`.
struct Session {
    scan_path: PathBuf,
    storage_path: PathBuf,
}

impl Session {
    fn new() -> Self {
        let cfg = config::Config::default();
        Self {
            scan_path: PathBuf::from(cfg.scan_path),
            storage_path: PathBuf::from(cfg.storage_path),
        }
    }

    /// Temporary location the new index is built in before it replaces the old one.
    fn building_path(&self) -> PathBuf {
        let mut name = self.storage_path.clone().into_os_string();
        name.push(".building");
        PathBuf::from(name)
    }

    fn index(&self) -> AppResult<()> {
        if !self.scan_path.is_dir() {
            return Err(format!("not a folder: {}", self.scan_path.display()).into());
        }
        println!("Scanning: {}", self.scan_path.display());

        let (num_threads, memory_per_thread) = config::calculate_writer_config();
        let commit_threshold = config::calculate_commit_threshold(memory_per_thread);

        // Build into a separate folder and swap it in only after success, so a
        // failed or empty scan never destroys the existing index.
        let building = self.building_path();
        if building.exists() {
            fs::remove_dir_all(&building)?;
        }

        let mut engine = SearchEngine::new(&building, num_threads, memory_per_thread)?;
        let (files, documents, errors) = {
            let mut indexer = Indexer::new(&mut engine, commit_threshold);
            scanner::scan_directory(&self.scan_path, &mut |path, content| {
                indexer.index_file(path, content);
            })?;
            indexer.flush();
            (indexer.files, indexer.documents, indexer.errors)
        };
        engine.finish_indexing()?;
        drop(engine);

        if files == 0 {
            fs::remove_dir_all(&building)?;
            return Err(format!(
                "no supported files found in {}; existing index left unchanged",
                self.scan_path.display()
            )
            .into());
        }

        if self.storage_path.exists() {
            fs::remove_dir_all(&self.storage_path)?;
        }
        fs::rename(&building, &self.storage_path)?;

        println!(
            "Indexed {} files ({} documents), {} errors",
            files, documents, errors
        );
        Ok(())
    }

    fn search(&self, query: &str) -> AppResult<()> {
        if !self.storage_path.exists() {
            return Err("no index yet, run `index` first".into());
        }
        let engine = SearchEngine::open_for_search(&self.storage_path)?;
        let results = engine.search(query, 10)?;

        if results.is_empty() {
            println!("No matches found.");
        } else {
            println!("Found files (top {}):", results.len());
            for (i, path) in results.iter().enumerate() {
                println!("[{}] {}", i + 1, path);
            }
        }
        Ok(())
    }

    fn set_scan_path(&mut self, arg: &str) -> AppResult<()> {
        let arg = arg.trim().trim_matches('"');
        if arg.is_empty() {
            println!("Scan folder: {}", self.scan_path.display());
            return Ok(());
        }
        let path = PathBuf::from(arg);
        if !path.is_dir() {
            return Err(format!("not a folder: {}", arg).into());
        }
        self.scan_path = path;
        println!(
            "Scan folder set to {}. Run `index` to rebuild.",
            self.scan_path.display()
        );
        Ok(())
    }
}

fn print_help() {
    println!("Type a query to search the index.");
    println!("Commands:");
    println!("  index             rebuild the index from the scan folder");
    println!("  scan <folder>     change the scan folder (quotes allowed)");
    println!("  scan              show the current scan folder");
    println!("  find <query>      search for a query even if it equals a command name");
    println!("  help              show this help");
    println!("  quit              exit");
}

fn repl(session: &mut Session) -> AppResult<()> {
    print_help();
    println!("Scan folder: {}", session.scan_path.display());

    let stdin = io::stdin();
    loop {
        print!("gistfind> ");
        io::stdout().flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            // End of input (Ctrl+D / Ctrl+Z).
            println!();
            break;
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        // The first word decides whether this is a command. Anything that is not
        // a command is a search query, so `find <query>` is available for
        // queries that happen to equal a command name.
        let (word, arg) = input
            .split_once(char::is_whitespace)
            .map(|(w, a)| (w, a.trim()))
            .unwrap_or((input, ""));

        let outcome = match word {
            "q" | "quit" | "exit" => break,
            "h" | "help" | "?" => {
                print_help();
                Ok(())
            }
            "i" | "index" => session.index(),
            "s" | "scan" => session.set_scan_path(arg),
            "find" if !arg.is_empty() => session.search(arg),
            _ => session.search(input),
        };

        if let Err(e) = outcome {
            eprintln!("Error: {}", e);
        }
    }
    Ok(())
}

fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().collect();
    let mut session = Session::new();

    // No arguments: interactive mode.
    if args.len() < 2 {
        return repl(&mut session);
    }

    // One-shot mode: `gistfind index` or `gistfind <query>`.
    match args[1].as_str() {
        "index" => session.index(),
        _ => session.search(&args[1..].join(" ")),
    }
}
