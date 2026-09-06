use std::path::Path;
use tantivy::doc;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use super::scheme::{build_schema, IndexFields};

pub struct SearchEngine {
    pub index: Index,
    pub fields: IndexFields,
    pub writer: IndexWriter,
}

impl SearchEngine {
    pub fn new<P: AsRef<Path>>(index_path: P) -> tantivy::Result<Self> {
        let fields = build_schema();

        std::fs::create_dir_all(&index_path)?;

        let dir = tantivy::directory::MmapDirectory::open(index_path)?;
        let index = Index::open_or_create(dir, fields.schema.clone())?;
        let (num_threads, memory_per_thread)= crate::config::calculate_writer_config();
        let writer = index.writer_with_num_threads(num_threads, memory_per_thread)?;

        Ok(Self {
            index,
            fields,
            writer,
        })
    }

    pub fn add_document(&mut self, file_path: &str, content: &str) -> tantivy::Result<()> {
        self.writer.add_document(doc!(
            self.fields.path => file_path,
            self.fields.content => content,
        ))?;
        Ok(())
    }

    pub fn commit(&mut self) -> tantivy::Result<()> {
        self.writer.commit()?;
        Ok(())
    }

    pub fn search(&self, query_str: &str, limit: usize) -> tantivy::Result<Vec<String>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.fields.content]);
        let query = query_parser.parse_query(query_str)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();

        for(_score, doc_adress) in top_docs{
            let retrieved_doc: TantivyDocument = searcher.doc(doc_adress)?;

             if let Some(OwnedValue::Str(path_str)) = retrieved_doc.get_first(self.fields.path) {
                results.push(path_str.clone());
            }
        }
        Ok(results)
    }
}
