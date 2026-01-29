//! Audio processing capabilities.
//!
//! This module provides audio generation and transcription functionality.

pub mod errors;
pub mod generation;
pub mod transcription;

pub use errors::{AudioGenerationError, TranscriptionError};
pub use generation::*;
pub use transcription::*;
