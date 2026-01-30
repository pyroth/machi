//! Image processing example using Ollama with vision model.
//!
//! Run with: `cargo run -p machi --example agent_with_image`
//!
//! Note: You can use any vision model by passing a custom model name string.
//! For example: `client.agent("llava:13b")` or `client.agent("bakllava")`

use std::path::Path;

use base64::{Engine, prelude::BASE64_STANDARD};
use machi::client::Nothing;
use machi::completion::message::{DocumentSourceKind, ImageMediaType};
use machi::completion::{Prompt, message::Image};
use machi::prelude::*;
use machi::providers::ollama;

/// Local image path relative to examples directory
const IMAGE_PATH: &str = "machi/examples/assets/camponotus_flavomarginatus_ant.jpg";

/// Vision model name - can be customized to any Ollama vision model
const VISION_MODEL: &str = "qwen3-vl";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create Ollama client with vision model
    let client = ollama::Client::from_val(Nothing);

    // Create agent with vision model
    let agent = client
        .agent(VISION_MODEL)
        .preamble("You are an image describer. Describe the image in detail.")
        .build();

    // Read local image and convert to base64
    let image_bytes = std::fs::read(Path::new(IMAGE_PATH))?;
    let image_base64 = BASE64_STANDARD.encode(&image_bytes);

    // Compose `Image` for prompt
    let image = Image {
        data: DocumentSourceKind::base64(&image_base64),
        media_type: Some(ImageMediaType::JPEG),
        ..Default::default()
    };

    // Prompt the agent and print the response
    let response = agent.prompt(image).await?;
    println!("{response}");

    Ok(())
}
