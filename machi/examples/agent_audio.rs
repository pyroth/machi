//! Audio loading example demonstrating AgentAudio API.
//!
//! ```bash
//! cargo run --example agent_audio
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use machi::prelude::*;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/data")
        .join("en-us-natural-speech.mp3");

    let audio = AgentAudio::load_from_path(&path, 16000).await?;

    println!("Format: {:?}", audio.format());
    println!("Sample rate: {} Hz", audio.sample_rate());
    println!(
        "Base64 length: {} chars",
        audio.to_base64().map_or(0, |b| b.len())
    );

    Ok(())
}
