//! Core provider traits for API abstraction.

use std::fmt::Debug;

use http::{HeaderName, HeaderValue};

use crate::http::{self, Builder};

use super::super::{builder::ClientBuilder, core::Transport};

/// Trait for instantiating provider clients.
pub trait ProviderClient {
    /// Input type for creating a client from a value.
    type Input;

    /// Creates a client from the process's environment.
    ///
    /// # Panics
    ///
    /// Panics if the environment is improperly configured.
    fn from_env() -> Self;

    /// Creates a client from the given input value.
    fn from_val(input: Self::Input) -> Self;
}

/// Trait for API keys that can be converted to HTTP headers.
///
/// - `Some(...)` - Key is inserted into the client's default headers
/// - `None` - Key is handled by the provider extension
pub trait ApiKey: Sized {
    /// Converts this API key into an HTTP header.
    fn into_header(self) -> Option<http::Result<(HeaderName, HeaderValue)>> {
        None
    }
}

/// Extension trait for debug formatting of provider types.
pub trait DebugExt: Debug {
    /// Returns additional fields to include in debug output.
    fn fields(&self) -> impl Iterator<Item = (&'static str, &dyn Debug)> {
        std::iter::empty()
    }
}

/// Provider extension trait for defining provider-specific behavior.
///
/// This abstracts over extensions used with `Client<Ext, H>` to define
/// networking, authentication, and model instantiation behavior.
pub trait Provider: Sized {
    /// Path used for verifying client configuration.
    const VERIFY_PATH: &'static str;

    /// The builder type for this provider.
    type Builder: ProviderBuilder;

    /// Builds the provider extension from a client builder.
    fn build<H>(
        builder: &ClientBuilder<Self::Builder, <Self::Builder as ProviderBuilder>::ApiKey, H>,
    ) -> http::Result<Self>;

    /// Builds a URI for the given path and transport type.
    fn build_uri(&self, base_url: &str, path: &str, _transport: Transport) -> String {
        let base_url = if base_url.is_empty() {
            base_url.to_string()
        } else {
            base_url.to_string() + "/"
        };

        base_url + path.trim_start_matches('/')
    }

    /// Applies custom modifications to a request builder.
    fn with_custom(&self, req: Builder) -> http::Result<Builder> {
        Ok(req)
    }
}

/// Marker trait for capability checks.
pub trait Capability {
    /// Whether the capability is available.
    const CAPABLE: bool;
}

/// Trait defining the capabilities of a provider.
pub trait Capabilities<H = reqwest::Client> {
    /// Completion capability marker.
    type Completion: Capability;
    /// Embeddings capability marker.
    type Embeddings: Capability;
    /// Transcription capability marker.
    type Transcription: Capability;
    /// Image generation capability marker.
    #[cfg(feature = "image")]
    type ImageGeneration: Capability;
    /// Audio generation capability marker.
    #[cfg(feature = "audio")]
    type AudioGeneration: Capability;
}

/// Builder trait for provider extensions.
///
/// Abstracts over provider-specific builders that configure and produce
/// a provider's extension type.
pub trait ProviderBuilder: Sized {
    /// The output extension type.
    type Output: Provider;
    /// The API key type.
    type ApiKey;

    /// The default base URL for this provider.
    const BASE_URL: &'static str;

    /// Customizes the builder before client creation.
    ///
    /// Can be used to add default headers or other configuration.
    fn finish<H>(
        &self,
        builder: ClientBuilder<Self, Self::ApiKey, H>,
    ) -> http::Result<ClientBuilder<Self, Self::ApiKey, H>> {
        Ok(builder)
    }
}
