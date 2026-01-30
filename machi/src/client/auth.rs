//! Authentication types for provider clients.
//!
//! This module provides authentication abstractions for API clients,
//! including bearer token authentication and a placeholder type for
//! clients that don't require authentication.

use http::{HeaderName, HeaderValue};

use crate::http::{self as http_client, make_auth_header};

/// Trait for API key types that can be converted to HTTP headers.
///
/// Implementations determine how the API key is included in requests:
/// - `Some(...)` - Key is inserted into the client's default headers
/// - `None` - Key is handled by the provider extension
pub trait ApiKey: Sized {
    /// Converts this API key into an HTTP header, if applicable.
    fn into_header(self) -> Option<http_client::Result<(HeaderName, HeaderValue)>> {
        None
    }
}

/// Bearer token authentication.
///
/// The API key will be inserted into request headers as a bearer auth token.
pub struct BearerAuth(String);

impl ApiKey for BearerAuth {
    fn into_header(self) -> Option<http_client::Result<(HeaderName, HeaderValue)>> {
        Some(make_auth_header(self.0))
    }
}

impl<S> From<S> for BearerAuth
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
        Self(value.into())
    }
}

/// A placeholder type representing the absence of a value.
///
/// Used for type-level `Option`-like behavior to indicate missing capabilities
/// or fields (e.g., an API key for local models that don't require one).
#[derive(Debug, Default, Clone, Copy)]
pub struct Nothing;

impl ApiKey for Nothing {}

impl super::traits::Capability for Nothing {
    const CAPABLE: bool = false;
}

impl TryFrom<String> for Nothing {
    type Error = &'static str;

    fn try_from(_: String) -> Result<Self, Self::Error> {
        Err(
            "Tried to create a Nothing from a string - this should not happen, please file an issue",
        )
    }
}

/// Typestate marker indicating an API key is required but not yet set.
#[derive(Debug, Default, Clone, Copy)]
pub struct NeedsApiKey;
