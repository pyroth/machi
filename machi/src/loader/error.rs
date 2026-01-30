//! Error types for the loader module.

use std::string::FromUtf8Error;

use thiserror::Error;

/// Errors that can occur during file loading operations.
#[derive(Error, Debug)]
pub enum FileLoaderError {
    #[error("Invalid glob pattern: {0}")]
    InvalidGlobPattern(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Pattern error: {0}")]
    PatternError(#[from] glob::PatternError),

    #[error("Glob error: {0}")]
    GlobError(#[from] glob::GlobError),

    #[error("String conversion error: {0}")]
    StringUtf8Error(#[from] FromUtf8Error),
}

#[cfg(feature = "pdf")]
/// Errors that can occur during PDF loading operations.
#[derive(Error, Debug)]
pub enum PdfLoaderError {
    #[error("{0}")]
    FileLoaderError(#[from] FileLoaderError),

    #[error("UTF-8 conversion error: {0}")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error("IO error: {0}")]
    PdfError(#[from] lopdf::Error),
}
