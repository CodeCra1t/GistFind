use super::scheme::{IndexFields, build_schema};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::doc;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

pub struct SearchEngine {
    pub index: Index,
    pub fields: IndexFields,
    pub writer: Option<IndexWriter>,
    pub reader: IndexReader,
    pub num_threads: usize,
    pub memory_per_thread: usize,
}

impl SearchEngine {
    /// Opens an existing index for searching. No writer is created, so the
    /// writer settings do not matter here.
    pub fn open_for_search<P: AsRef<Path>>(index_path: P) -> tantivy::Result<Self> {
        Self::new(index_path, 1, 15_000_000)
    }

    pub fn new<P: AsRef<Path>>(index_path: P, num_threads: usize, memory_per_thread: usize,) -> tantivy::Result<Self> {
        let fields = build_schema();

        std::fs::create_dir_all(&index_path)?;

        let dir = tantivy::directory::MmapDirectory::open(index_path)?;
        let index = Index::open_or_create(dir, fields.schema.clone())?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            fields,
            writer: None,
            reader,
            num_threads,
            memory_per_thread,
        })
    }

    fn ensure_writer(&mut self) -> tantivy::Result<&mut IndexWriter> {
        if self.writer.is_none() {
            let writer = self
                .index
                .writer_with_num_threads(self.num_threads, self.memory_per_thread)?;
            self.writer = Some(writer);
        }
        Ok(self.writer.as_mut().unwrap())
    }

    pub fn add_document(&mut self, file_path: &str, content: &str) -> tantivy::Result<()> {
        let path_field = self.fields.path;
        let content_field = self.fields.content;

        let writer = self.ensure_writer()?;

        writer.add_document(doc!(
            path_field => file_path,
            content_field => content,
        ))?;

        Ok(())
    }

    pub fn commit(&mut self) -> tantivy::Result<()> {
        if let Some(writer) = self.writer.as_mut() {
            writer.commit()?;
        }
        Ok(())
    }

    pub fn finish_indexing(&mut self) -> tantivy::Result<()> {
        if let Some(writer) = self.writer.take() {
            let _ = writer.wait_merging_threads()?;
        }
        Ok(())
    }

    pub fn search(&self, query_str: &str, limit: usize) -> tantivy::Result<Vec<String>> {
        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.fields.content]);

        // Lenient parsing: stray quotes, brackets or colons in a user query
        // must not turn into a hard error.
        let (query, _parse_errors) = query_parser.parse_query_lenient(query_str);

        // Several chunks of one file can match, so over-fetch generously and
        // de-duplicate. Fetching only a few times `limit` could return fewer
        // distinct files than requested.
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit * 20))?;

        let mut results: Vec<String> = Vec::new();

        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;

            if let Some(OwnedValue::Str(path_str)) = retrieved_doc.get_first(self.fields.path) {
                let file_path = strip_chunk_suffix(path_str).to_string();
                if !results.contains(&file_path) {
                    results.push(file_path);
                    if results.len() == limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }
}

/// `dir/file.txt#chunk3` -> `dir/file.txt`. A name that merely ends in
/// `#chunk` (no digits after it) is returned unchanged.
fn strip_chunk_suffix(path: &str) -> &str {
    match path.rfind("#chunk") {
        Some(pos) => {
            let digits = &path[pos + "#chunk".len()..];
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                &path[..pos]
            } else {
                path
            }
        }
        None => path,
    }
}
