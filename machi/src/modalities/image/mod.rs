//! Image processing capabilities.
//!
//! This module provides image generation functionality.

pub mod errors;
pub mod generation;

pub use errors::ImageGenerationError;
pub use generation::*;
