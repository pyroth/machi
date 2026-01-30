use machi::client::Nothing;
use machi::prelude::*;
use machi::providers::ollama::{self, QWEN3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A record representing a person
#[derive(Debug, Deserialize, JsonSchema, Serialize)]
struct Person {
    /// The person's first name, if provided (null otherwise)
    #[schemars(required)]
    pub first_name: Option<String>,
    /// The person's last name, if provided (null otherwise)
    #[schemars(required)]
    pub last_name: Option<String>,
    /// The person's job, if provided (null otherwise)
    #[schemars(required)]
    pub job: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create Ollama client
    let client = ollama::Client::from_val(Nothing);

    // Create extractor
    let data_extractor = client.extractor::<Person>(QWEN3).build();
    let person = data_extractor
        .extract("Hello my name is John Doe! I am a software engineer.")
        .await?;

    println!("Ollama: {}", serde_json::to_string_pretty(&person).unwrap());

    Ok(())
}
