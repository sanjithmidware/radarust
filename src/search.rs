use crate::azure::DownloadedSplit;
use anyhow::{anyhow, Context};
use log::{info, debug, warn}; // Added warn
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{OwnedValue, Schema, TEXT, STORED};
use tantivy::{doc, Index, IndexWriter, TantivyDocument};

const FIELD_TITLE: &str = "title";
const FIELD_BODY: &str = "body";

pub struct IndexManager {
    pub index: Index,
    pub schema: Schema,
}

impl IndexManager {
    pub fn create(base_path: &Path) -> anyhow::Result<Self> {
        if base_path.exists() {
            info!("Found existing local storage directory. Deleting it to ensure a fresh index.");
            std::fs::remove_dir_all(base_path)
                .with_context(|| format!("Failed to delete old index directory at {:?}", base_path))?;
        }
        info!("Creating new local storage directory at: {:?}", base_path);
        std::fs::create_dir_all(base_path)
            .with_context(|| format!("Failed to create index directory at {:?}", base_path))?;

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field(FIELD_TITLE, TEXT | STORED);
        schema_builder.add_text_field(FIELD_BODY, TEXT | STORED);
        let schema = schema_builder.build();
        let index = Index::create_in_dir(base_path, schema.clone())
            .context("Failed to create Tantivy index")?;
        
        info!("Successfully created a new, empty Tantivy index.");
        Ok(IndexManager {
            index,
            schema,
        })
    }

    pub fn index_splits(&self, splits: Vec<DownloadedSplit>) -> anyhow::Result<usize> {
        info!("Preparing to index {} downloaded splits...", splits.len());
        let mut index_writer: IndexWriter = self.index.writer(100_000_000)
            .context("Failed to create index writer")?;

        let title_field = self.schema.get_field(FIELD_TITLE).unwrap();
        let body_field = self.schema.get_field(FIELD_BODY).unwrap();

        let mut indexed_count = 0;
        for (i, split) in splits.iter().enumerate() {
            info!("\nProcessing split {} of {}: '{}'", i + 1, splits.len(), split.name);
            
            // --- TEMPORARY DEBUGGING CHANGE ---
            // We are SKIPPING the zstd decompression step and assuming the content is plain text.
            warn!("Skipping decompression step for debugging. Treating content as plain text.");
            let body_text = String::from_utf8_lossy(&split.content);

            debug!("\n--- [START] Raw Content of '{}' ---\n{}\n--- [END] Raw Content of '{}' ---\n", split.name, body_text, split.name);
            println!("\n--- Raw Content Preview for '{}' ---\n{}...", split.name, body_text.chars().take(500).collect::<String>());
            
            info!("Adding raw content to the search index...");
            index_writer.add_document(doc!(
                title_field => split.name.clone(),
                body_field => body_text.to_string()
            ))?;
            info!("Successfully added to index writer.");
            indexed_count += 1;
        }

        info!("\nCommitting all new documents to the index. This makes them searchable.");
        index_writer.commit().context("Failed to commit to index")?;
        info!("Commit successful.");
        
        Ok(indexed_count)
    }

    pub fn search(&self, query_str: &str) -> anyhow::Result<()> {
        info!("\nPreparing search for query: '{}'", query_str);
        let reader = self.index.reader().context("Failed to create index reader")?;
        let searcher = reader.searcher();
        
        let title_field = self.schema.get_field(FIELD_TITLE).unwrap();
        let body_field = self.schema.get_field(FIELD_BODY).unwrap();

        let query_parser = QueryParser::for_index(&self.index, vec![title_field, body_field]);
        let query = query_parser.parse_query(query_str)
            .map_err(|e| anyhow!("Failed to parse query: {}", e))?;

        debug!("Executing search...");
        let top_docs = searcher.search(&query, &TopDocs::with_limit(10))
            .context("Search execution failed")?;
        debug!("Search executed. Found {} results.", top_docs.len());

        info!("\n--- Search Results for '{}' ---", query_str);
        if top_docs.is_empty() {
            info!("No documents found.");
        } else {
            for (score, doc_address) in top_docs {
                let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
                
                let title = retrieved_doc
                    .get_first(title_field)
                    .and_then(|v| match v {
                        OwnedValue::Str(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");

                let body_preview = retrieved_doc
                    .get_first(body_field)
                    .and_then(|v| match v {
                        OwnedValue::Str(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                
                debug!("Score: {:.2}, Title: {}", score, title);
                println!("Score: {:.2} | Title: {}", score, title);
                println!("  Body Preview: {}...", body_preview.chars().take(150).collect::<String>());
                println!("---------------------------------");
            }
        }
        Ok(())
    }
}