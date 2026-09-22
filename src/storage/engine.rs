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

        let query = query_parser.parse_query(query_str)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();

        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;

            if let Some(OwnedValue::Str(path_str)) = retrieved_doc.get_first(self.fields.path) {
                results.push(path_str.clone());
            }
        }

        Ok(results)
    }
}
