//! PDF document processing example using Ollama with RAG.
//!
//! Run with: `cargo run -p machi --example agent_with_pdf --features "pdf,derive"`
//!
//! This example demonstrates how to:
//! 1. Load and extract text from a PDF file using PdfFileLoader
//! 2. Create embeddings for the PDF content
//! 3. Use RAG (Retrieval-Augmented Generation) to answer questions about the PDF

use anyhow::Context;
use machi::Embed;
use machi::client::Nothing;
use machi::completion::Prompt;
use machi::embedding::EmbeddingsBuilder;
use machi::loader::PdfFileLoader;
use machi::prelude::*;
use machi::providers::ollama::{self, NOMIC_EMBED_TEXT};
use machi::store::in_memory_store::InMemoryVectorStore;
use serde::Serialize;
use std::path::PathBuf;

/// Local PDF path relative to examples directory
const PDF_PATH: &str = "machi/examples/assets/deepseek_r1.pdf";

/// Chunk size for splitting PDF content
const CHUNK_SIZE: usize = 2000;

/// Document structure for RAG
#[derive(Embed, Serialize, Clone, Debug, Eq, PartialEq, Default)]
struct PdfChunk {
    id: String,
    #[embed]
    content: String,
}

/// Load PDF and split into chunks
fn load_pdf(path: PathBuf) -> anyhow::Result<Vec<String>> {
    let content_chunks = PdfFileLoader::with_glob(path.to_str().context("Invalid path")?)?
        .read()
        .into_iter()
        .filter_map(|result| {
            result
                .map_err(|e| {
                    eprintln!("Error reading PDF content: {e}");
                    e
                })
                .ok()
        })
        .flat_map(|content| {
            let mut chunks = Vec::new();
            let mut current = String::new();
            for word in content.split_whitespace() {
                if current.len() + word.len() + 1 > CHUNK_SIZE && !current.is_empty() {
                    chunks.push(std::mem::take(&mut current).trim().to_string());
                }
                current.push_str(word);
                current.push(' ');
            }
            if !current.is_empty() {
                chunks.push(current.trim().to_string());
            }
            chunks
        })
        .collect::<Vec<_>>();

    if content_chunks.is_empty() {
        anyhow::bail!("No content found in PDF file: {}", path.display());
    }
    Ok(content_chunks)
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create Ollama client
    let client = ollama::Client::from_val(Nothing);

    // Load PDF and extract text chunks
    let pdf_path = PathBuf::from(PDF_PATH);
    let pdf_chunks = load_pdf(pdf_path).context("Failed to load PDF")?;
    println!("Loaded {} chunks from PDF", pdf_chunks.len());

    // Create embedding model
    let embedding_model = client.embedding_model(NOMIC_EMBED_TEXT);

    // Build embeddings for PDF chunks
    let documents: Vec<PdfChunk> = pdf_chunks
        .into_iter()
        .enumerate()
        .map(|(i, content)| PdfChunk {
            id: format!("chunk_{i}"),
            content,
        })
        .collect();

    let embeddings = EmbeddingsBuilder::new(embedding_model.clone())
        .documents(documents)?
        .build()
        .await?;
    println!("Generated embeddings for PDF content");

    // Create vector store and index
    let vector_store = InMemoryVectorStore::from_documents(embeddings);
    let index = vector_store.index(embedding_model);

    // Build RAG agent with dynamic context retrieval
    let rag_agent = client
        .agent("qwen3")
        .preamble("You are a helpful assistant that answers questions based on the provided PDF document context. Synthesize information from multiple chunks if needed.")
        .dynamic_context(3, index)
        .build();

    // Query the agent about the PDF content
    let response = rag_agent
        .prompt("What is the main topic of this document? Summarize the key points.")
        .await?;

    println!("\nResponse:\n{response}");

    Ok(())
}
