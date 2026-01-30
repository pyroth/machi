//! RAG (Retrieval-Augmented Generation) example using Ollama.
//!
//! This example demonstrates how to:
//! 1. Create embeddings for documents using Ollama's embedding model
//! 2. Store embeddings in an in-memory vector store
//! 3. Use dynamic context retrieval with an agent
//!
//! Run with: `cargo run -p machi --example rag_ollama --features derive`

use machi::Embed;
use machi::client::Nothing;
use machi::completion::Prompt;
use machi::embedding::EmbeddingsBuilder;
use machi::prelude::*;
use machi::providers::ollama::{Client, NOMIC_EMBED_TEXT, QWEN3};
use machi::store::in_memory_store::InMemoryVectorStore;
use serde::Serialize;

/// Document structure for RAG.
/// The `#[embed]` attribute marks which field should be embedded for vector search.
#[derive(Embed, Serialize, Clone, Debug, Eq, PartialEq, Default)]
struct WordDefinition {
    id: String,
    word: String,
    #[embed]
    definitions: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .init();

    // Create Ollama client
    let client = Client::from_val(Nothing);
    let embedding_model = client.embedding_model(NOMIC_EMBED_TEXT);

    // Build embeddings for sample word definitions
    let embeddings = EmbeddingsBuilder::new(embedding_model.clone())
        .documents(vec![
            WordDefinition {
                id: "doc0".to_string(),
                word: "flurbo".to_string(),
                definitions: vec![
                    "A flurbo is a green alien that lives on cold planets.".to_string(),
                    "A fictional digital currency from Rick and Morty.".to_string(),
                ],
            },
            WordDefinition {
                id: "doc1".to_string(),
                word: "glarb-glarb".to_string(),
                definitions: vec![
                    "An ancient farming tool from planet Jiro.".to_string(),
                    "A fictional swamp creature from planet Glibbo.".to_string(),
                ],
            },
            WordDefinition {
                id: "doc2".to_string(),
                word: "linglingdong".to_string(),
                definitions: vec![
                    "A lunar term for humans.".to_string(),
                    "A mystical instrument from planet Quarm.".to_string(),
                ],
            },
        ])?
        .build()
        .await?;

    // Create vector store and index
    let vector_store = InMemoryVectorStore::from_documents(embeddings);
    let index = vector_store.index(embedding_model);

    // Build RAG agent with dynamic context retrieval
    // Retrieve top 3 most relevant documents for better context
    let rag_agent = client
        .agent(QWEN3)
        .preamble("You are a dictionary assistant. Answer questions about words using ONLY the definitions provided in the context below. Do not say the word is not defined if you see it in the context.")
        .dynamic_context(3, index)
        .build();

    // Query the agent
    let response = rag_agent.prompt("What does \"glarb-glarb\" mean?").await?;

    println!("{response}");

    Ok(())
}
