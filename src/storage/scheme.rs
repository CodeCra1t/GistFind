use tantivy::schema::*;

pub struct IndexFields {
    pub schema: Schema,
    pub path: Field,
    pub content: Field,
}

pub fn build_schema() -> IndexFields {
    let mut schema_builder = Schema::builder();
    let path = schema_builder.add_text_field("path", STRING | STORED);
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default().set_indexing_options(text_indexing);
    let content = schema_builder.add_text_field("content", text_options);

    IndexFields {
        schema: schema_builder.build(),
        path,
        content,
    }
}
