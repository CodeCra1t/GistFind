# GistFind

Local, privacy-first file search engine with semantic query understanding — written in Rust.

Find files by what's *inside* them, not just by their name. Fully offline, no cloud, no telemetry — your files never leave your machine.

> **Status: Alpha (v0.2.4)** — core indexing and search pipeline is functional and stable, but many planned features (PDF/DOCX support, background file watching, fuzzy search) are not implemented yet. Not ready for production use.

## Why GistFind

Ever had a file somewhere on your disk and couldn't remember its name or where you saved it — only that it *mentioned* something specific? Standard tools like Windows Search or `find` only match filenames. GistFind indexes the actual content of your files and lets you search by meaning, all locally.

## Current Features

- Full-text indexing powered by [tantivy](https://github.com/quickwit-oss/tantivy) (a Rust-native search engine library)
- Recursive directory scanning
- Adaptive resource configuration — automatically detects available RAM and CPU threads to configure the indexing engine, so it runs efficiently on both low-end laptops and powerful desktops without manual tuning
- Incremental commits during indexing to avoid data loss on large batches
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
cd gistfind
cargo build --release
```

### Usage

Run without arguments to start the interactive mode:

```bash
cargo run --release
```

```text
gistfind> index                  rebuild the index from the scan folder
gistfind> scan "D:\Thesis"       change the scan folder (quotes allowed)
gistfind> scan                   show the current scan folder
gistfind> neural network         search the index
gistfind> find index             search for the word "index" itself
gistfind> quit
```

The first word decides whether a line is a command (`index`, `scan`, `find`, `help`, `quit`).
Anything else is a search query. The scan folder changed with `scan` lasts only for the current session.

### Ignoring files: `.gistignore`

Put a file named `.gistignore` in the root of the folder you scan. Its rules work like `.gitignore`:

```text
# comments and empty lines are skipped
node_modules/        a folder with this name, anywhere below the root
*.log                any file ending in .log
/drafts              only the drafts folder in the root
docs/private/**      everything under docs/private
!keep.log            re-include a file excluded by an earlier rule
```

Ignored folders are not entered at all, so they cost no reading time. Only the `.gistignore` in the scan root is read; nested ones are not supported yet.

One-shot commands also work:

```bash
cargo run --release -- index
cargo run --release -- <your search query>
```

By default, GistFind scans `./test_docs` and stores its index in `./storage_data`. These paths are configured in `src/config.rs`.
A new index is built next to the old one and replaces it only after a successful scan, so a failed scan never deletes the existing index.

## Roadmap

- [x] Fix silent error swallowing (errors are now logged and counted)
- [ ] Byte-based adaptive commit thresholds (instead of a fixed file count)
- [ ] Parallel directory scanning (`jwalk` + `rayon`)
- [ ] PDF and DOCX text extraction
- [ ] Background file watcher for incremental re-indexing (`notify` crate)
- [ ] Fuzzy search for typo tolerance
- [ ] Stemming and stop-word filtering
- [ ] Local LLM-based query normalization for natural-language search

## License

Licensed under the [MIT License](LICENSE).

## Author

Built solo by CodeCra1t, as a personal tool
