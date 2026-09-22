mod config;
mod scanner;
mod storage;

use std::env;
use std::path::Path;
use storage::engine::SearchEngine;
use sysinfo::System;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let cfg = config::Config::default();

    let sys = System::new_all();
    println!(
        "Total RAM (raw value): {} bytes or {} Mb",
        sys.total_memory(),
        sys.total_memory() / 1000 / 1000
    );
    println!(
        "Available RAM (raw value): {} bytes or {} Mb ",
        sys.available_memory(),
        sys.available_memory() / 1000 / 1000
    );
    println!("Search engine initialization ... ");

    let mut current_bytes = 0;
    let mut indexed_count = 0;

    let (num_threads, memory_per_thread) = config::calculate_writer_config();
    let commit_threshold = config::calculate_commit_threshold(memory_per_thread);
    let mut engine = SearchEngine::new(&cfg.storage_path,num_threads,memory_per_thread)?;

    if args.len() > 1 && args[1] == "index" {
        println!("Scanning directory: {}", cfg.scan_path);

        scanner::scan_directory(Path::new(&cfg.scan_path), &mut |path, content| {
            let file_bytes = content.len();
            let path_str = path.to_string_lossy();

            if indexed_count % 1000 == 0{
            println!("\rIndexing [{}] {} ...",indexed_count+1, path_str);
            let _ = io::stdout().flush();
            }
            if file_bytes > commit_threshold {
                if current_bytes > 0 {
                    let _ = engine.commit();
                    current_bytes = 0;
                }

                let chunk_size = commit_threshold / 2;
                let mut chunk_buffer = String::with_capacity(chunk_size);
                let mut chunk_index = 1;

                for line in content.lines() {
                    chunk_buffer.push_str(line);
                    chunk_buffer.push('\n');

                    if chunk_buffer.len() >= chunk_size {
                        let chunk_path = format!("{}#chunk{}", path_str, chunk_index);
                        if let Ok(_) = engine.add_document(&chunk_path, &chunk_buffer) {
                            current_bytes += chunk_buffer.len();
                        }
                        chunk_buffer.clear();
                        chunk_index += 1;
                        if current_bytes >commit_threshold {
                            let _ = engine.commit();
                            current_bytes = 0;
                        }
                    }
                }
                if !chunk_buffer.is_empty() {
                    let chunk_path = format!("{}#chunk{}", path_str, chunk_index);
                    if let Ok(_) = engine.add_document(&chunk_path, &chunk_buffer) {
                        let _ = engine.commit();
                    }
                }
                indexed_count += 1;
            } else {
                if let Err(e) = engine.add_document(&path_str, content) {
                    eprintln!("File Indexation Error {}: {}", path_str, e);
                } else {
                    indexed_count += 1;
                    current_bytes += file_bytes;

                    if current_bytes > commit_threshold {
                        println!(
                            "Memory limit reached ({} Mb). Saving index on disk ...",
                            current_bytes / 1000 / 1000
                        );
                        if let Err(e) = engine.commit() {
                            eprintln!("Commit error: {}", e);
                        }
                        current_bytes = 0;
                    }
                }
            }
        })?;

        if current_bytes > 0 {
            let _ = engine.commit();
        }

        engine.finish_indexing()?;

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
        println!("No matches find.");
    } else {
        println!(r"\Found files (top 10):");
        for (i, path) in results.iter().enumerate() {
            println!("[{}] {}", i + 1, path);
        }
    }
    Ok(())
}
