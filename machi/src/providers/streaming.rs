//! Streaming infrastructure for model providers.
//!
//! This module provides parsers and utilities for handling streaming
//! responses from various LLM APIs.

use crate::error::AgentError;
use crate::message::ChatMessageStreamDelta;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Stream of model response deltas for streaming generation.
#[cfg(not(target_arch = "wasm32"))]
pub type ModelStream =
    Pin<Box<dyn Stream<Item = Result<ChatMessageStreamDelta, AgentError>> + Send>>;

/// Stream of model response deltas for streaming generation (WASM version, not Send).
#[cfg(target_arch = "wasm32")]
pub type ModelStream = Pin<Box<dyn Stream<Item = Result<ChatMessageStreamDelta, AgentError>>>>;

/// A generic streaming response parser for SSE (Server-Sent Events) format.
///
/// Handles buffering and line parsing for streaming API responses.
#[derive(Debug)]
pub struct SseStreamParser<S> {
    inner: S,
    buffer: String,
}

impl<S> SseStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    /// Create a new SSE stream parser.
    pub const fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buffer: String::new(),
        }
    }

    /// Try to extract the next complete line from the buffer.
    fn next_line(&mut self) -> Option<String> {
        self.buffer.find('\n').map(|pos| {
            let line = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 1..].to_string();
            line
        })
    }

    /// Parse an SSE data line, stripping the "data: " prefix.
    #[must_use]
    pub fn parse_sse_data(line: &str) -> Option<&str> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            return None;
        }
        trimmed.strip_prefix("data: ")
    }

    /// Check if the data indicates stream completion.
    #[must_use]
    pub fn is_done_marker(data: &str) -> bool {
        data.trim() == "[DONE]"
    }
}

impl<S> Stream for SseStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<String, AgentError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to get a complete line from buffer
            if let Some(line) = self.next_line() {
                if let Some(data) = Self::parse_sse_data(&line)
                    && !Self::is_done_marker(data)
                {
                    return Poll::Ready(Some(Ok(data.to_string())));
                }
                continue;
            }

            // Need more data from the inner stream
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        self.buffer.push_str(text);
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(AgentError::from(e))));
                }
                Poll::Ready(None) => {
                    // Process remaining buffer
                    if !self.buffer.is_empty() {
                        let remaining = std::mem::take(&mut self.buffer);
                        for line in remaining.lines() {
                            if let Some(data) = Self::parse_sse_data(line)
                                && !Self::is_done_marker(data)
                            {
                                return Poll::Ready(Some(Ok(data.to_string())));
                            }
                        }
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// A generic streaming response parser for NDJSON (Newline-Delimited JSON) format.
///
/// Used by Ollama and similar providers.
#[derive(Debug)]
pub struct NdjsonStreamParser<S> {
    inner: S,
    buffer: String,
}

impl<S> NdjsonStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    /// Create a new NDJSON stream parser.
    pub const fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buffer: String::new(),
        }
    }

    /// Try to extract the next complete line from the buffer.
    fn next_line(&mut self) -> Option<String> {
        self.buffer.find('\n').map(|pos| {
            let line = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 1..].to_string();
            line
        })
    }
}

impl<S> Stream for NdjsonStreamParser<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<String, AgentError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to get a complete line from buffer
            if let Some(line) = self.next_line() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    return Poll::Ready(Some(Ok(trimmed.to_string())));
                }
                continue;
            }

            // Need more data from the inner stream
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        self.buffer.push_str(text);
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(AgentError::from(e))));
                }
                Poll::Ready(None) => {
                    // Process remaining buffer
                    if !self.buffer.is_empty() {
                        let remaining = std::mem::take(&mut self.buffer);
                        let trimmed = remaining.trim();
                        if !trimmed.is_empty() {
                            return Poll::Ready(Some(Ok(trimmed.to_string())));
                        }
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_parse_data() {
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data("data: hello"),
            Some("hello")
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data("data: [DONE]"),
            Some("[DONE]")
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data(""),
            None
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data(": comment"),
            None
        );
        assert_eq!(
            SseStreamParser::<futures::stream::Empty<_>>::parse_sse_data("event: message"),
            None
        );
    }

    #[test]
    fn test_sse_is_done_marker() {
        assert!(SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("[DONE]"));
        assert!(SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("  [DONE]  "));
        assert!(!SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("done"));
        assert!(!SseStreamParser::<futures::stream::Empty<_>>::is_done_marker("{}"));
    }
}
