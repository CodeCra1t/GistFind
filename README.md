# GistFind

Local, privacy-first file search engine with semantic query understanding — written in Rust.

Find files by what's *inside* them, not just by their name. Fully offline, no cloud, no telemetry — your files never leave your machine.

> **Status: Alpha (v0.2.2)** — core indexing and search pipeline is functional and stable, but many planned features (PDF/DOCX support, background file watching, fuzzy search) are not implemented yet. Not ready for production use.

## Why GistFind

Ever had a file somewhere on your disk and couldn't remember its name or where you saved it — only that it *mentioned* something specific? Standard tools like Windows Search or `find` only match filenames. GistFind indexes the actual content of your files and lets you search by meaning, all locally.

## Current Features

- Full-text indexing powered by [tantivy](https://github.com/quickwit-oss/tantivy) (a Rust-native search engine library)
- Recursive directory scanning
- Adaptive resource configuration — automatically detects available RAM and CPU threads to configure the indexing engine, so it runs efficiently on both low-end laptops and powerful desktops without manual tuning
- Memory-safe indexing — the engine tracks how much data has been buffered since the last commit and automatically flushes to disk before hitting memory limits, adapting to available RAM in real time
- Automatic chunking for oversized files — very large files are split instead of risking a memory spike or crash
- Storage-efficient schema: file content is indexed with positions (enabling phrase search) but not stored raw, keeping the index size well below the size of the indexed text
- Supported file types: `.txt`, `.md`, `.rs`, `.json`, `.toml`, `.log`

## How It Works

1. **Scan** — recursively walks a target directory
2. **Extract** — reads supported text files
3. **Index** — tokenizes content and builds a persistent tantivy index on disk
4. **Search** — queries the index and returns matching file paths, ranked by relevance

## Tech Stack

- **Rust** (edition 2024)
- [`tantivy`](https://crates.io/crates/tantivy) — full-text search engine
- [`sysinfo`](https://crates.io/crates/sysinfo) — system resource detection for adaptive configuration

## Getting Started

### Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))

### Build

```bash
git clone https://github.com/CodeCra1t/GistFind
cd GistFind
cargo build --release
```

### Usage

Index a directory:

```bash
cargo run -- index
```

Search the index:

```bash
cargo run -- <your search query>
```

By default, GistFind scans `./test_docs` and stores its index in `./storage_data`. These paths are configured in `src/config.rs`.

## Roadmap

- [x] Fix silent error swallowing
- [x] Byte-based adaptive commit thresholds (instead of a fixed file count)
- [x] Automatic chunking for oversized files
- [ ] Parallel directory scanning (`jwalk` + `rayon`)
- [ ] PDF and DOCX text extraction
- [ ] Background file watcher for incremental re-indexing (`notify` crate)
- [ ] Fuzzy search for typo tolerance
- [ ] Stemming and stop-word filtering
- [ ] Local LLM-based query normalization for natural-language search

## License

Licensed under the [MIT License](LICENSE).

## Author

Built solo by CodeCra1t, as a personal tool. Feedback and issues are welcome.
