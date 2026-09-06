mod config;
mod scanner;
mod storage;

use std::env;
use std::path::Path;
use storage::engine::SearchEngine;
use sysinfo::System;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let cfg = config::Config::default();

    let sys = System::new_all();
    println!("Total RAM (raw value): {}", sys.total_memory());
    println!("Available RAM (raw value): {} ", sys.available_memory());
    println!("Search engine initialization ... ");

    let mut engine = SearchEngine::new(&cfg.storage_path)?;

    if args.len() > 1 && args[1] == "index" {
        println!("Scanning directory: {}", cfg.scan_path);

        let mut indexed_count = 0;
        scanner::scan_directory(Path::new(&cfg.scan_path), &mut |path, content| {
            let path_str = path.to_string_lossy();
            if let Err(e) = engine.add_document(&path_str, content) {
                eprintln!("File Indexation Error {}: {}", path_str, e);
            } else {
                indexed_count += 1;
                if indexed_count % 500 == 0 {
                    println!("Saving {} index on disk ...", indexed_count);
                    if let Err(e) = engine.commit() {
                        eprintln!("Commit error at {}: {}", indexed_count, e);
                    }
                }
            }
        })?;

        println!("Successfully indexed files: {}", indexed_count);
        return Ok(());
    }

    let query_str = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        println!("Usage.");
        println!(" cargo run -- index      -File Indexation");
        println!(" cargo run -- <request>  -Finding file by index");
        return Ok(());
    };
    println!("\nFinding by request: \"{}\"", query_str);
    let results = engine.search(&query_str, 10)?;

    if results.is_empty() {
        println!("No mathces find.");
    } else {
        println!(r"\Found files (top 10):");
        for (i, path) in results.iter().enumerate() {
            println!("[{}] {}", i + 1, path);
        }
    }
    Ok(())
}
