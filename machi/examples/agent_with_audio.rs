//! Audio transcription example using OpenAI Whisper.
//!
//! Run with: `cargo run -p machi --example agent_with_audio`
//!
//! Requires: OPENAI_API_KEY environment variable to be set.
//!
//! This example demonstrates how to transcribe audio files using OpenAI's Whisper model.

use machi::modalities::audio::TranscriptionModel;
use machi::prelude::*;
use machi::providers::openai;

/// Local audio path relative to examples directory
const AUDIO_PATH: &str = "machi/examples/assets/en-us-natural-speech.mp3";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create OpenAI client from environment variable
    let client = openai::Client::from_env();

    // Create Whisper transcription model
    let whisper = client.transcription_model(openai::WHISPER_1);

    // Transcribe the audio file
    let response = whisper
        .transcription_request()
        .load_file(AUDIO_PATH)
        .send()
        .await?;

    println!("Transcription: {}", response.text);

    Ok(())
}
