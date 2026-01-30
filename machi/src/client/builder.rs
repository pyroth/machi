//! Client builder for constructing provider clients.

use std::sync::Arc;

use http::HeaderMap;

use crate::http::{self as http_client};

use super::{
    auth::ApiKey,
    core::Client,
    traits::{Provider, ProviderBuilder},
};

// Re-export from auth for builder signatures
pub use super::auth::NeedsApiKey;

/// Builder for constructing [`Client`] instances.
///
/// Uses typestate pattern to ensure API key is set before building.
#[derive(Clone)]
pub struct ClientBuilder<Ext, Key = NeedsApiKey, H = reqwest::Client> {
    pub(crate) base_url: String,
    pub(crate) api_key: Key,
    pub(crate) headers: HeaderMap,
    pub(crate) http_client: Option<H>,
    pub(crate) ext: Ext,
}

impl<ExtBuilder, H> Default for ClientBuilder<ExtBuilder, NeedsApiKey, H>
where
    H: Default,
    ExtBuilder: ProviderBuilder + Default,
{
    fn default() -> Self {
        Self {
            api_key: NeedsApiKey,
            headers: Default::default(),
            base_url: ExtBuilder::BASE_URL.into(),
            http_client: None,
            ext: Default::default(),
        }
    }
}

impl<Ext, H> ClientBuilder<Ext, NeedsApiKey, H> {
    /// Sets the API key for this client.
    ///
    /// This must be called before `build()` can be invoked.
    #[inline]
    pub fn api_key<Key>(self, api_key: impl Into<Key>) -> ClientBuilder<Ext, Key, H> {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: self.base_url,
            headers: self.headers,
            http_client: self.http_client,
            ext: self.ext,
        }
    }
}

impl<Ext, Key, H> ClientBuilder<Ext, Key, H>
where
    Ext: Clone,
{
    /// Maps over the extension field, transforming it to a new type.
    pub(crate) fn over_ext<F, NewExt>(self, f: F) -> ClientBuilder<NewExt, Key, H>
    where
        F: FnOnce(Ext) -> NewExt,
    {
        let ClientBuilder {
            base_url,
            api_key,
            headers,
            http_client,
            ext,
        } = self;

        let new_ext = f(ext.clone());

        ClientBuilder {
            base_url,
            api_key,
            headers,
            http_client,
            ext: new_ext,
        }
    }

    /// Sets the base URL for this client.
    pub fn base_url<S>(self, base_url: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            base_url: base_url.as_ref().to_string(),
            ..self
        }
    }

    /// Sets the HTTP backend for this client.
    pub fn http_client<U>(self, http_client: U) -> ClientBuilder<Ext, Key, U> {
        ClientBuilder {
            http_client: Some(http_client),
            base_url: self.base_url,
            api_key: self.api_key,
            headers: self.headers,
            ext: self.ext,
        }
    }

    /// Sets the default HTTP headers for this client.
    pub fn http_headers(self, headers: HeaderMap) -> Self {
        Self { headers, ..self }
    }

    /// Returns a mutable reference to the headers.
    pub(crate) fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Returns a mutable reference to the extension.
    pub(crate) fn ext_mut(&mut self) -> &mut Ext {
        &mut self.ext
    }
}

impl<Ext, Key, H> ClientBuilder<Ext, Key, H> {
    /// Returns a reference to the API key.
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn get_api_key(&self) -> &Key {
        &self.api_key
    }

    /// Returns a reference to the extension.
    #[inline]
    pub fn ext(&self) -> &Ext {
        &self.ext
    }
}

impl<Ext, ExtBuilder, Key, H> ClientBuilder<ExtBuilder, Key, H>
where
    ExtBuilder: Clone + ProviderBuilder<Output = Ext, ApiKey = Key> + Default,
    Ext: Provider<Builder = ExtBuilder>,
    Key: ApiKey,
    H: Default,
{
    /// Builds the client.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider extension fails to build.
    pub fn build(mut self) -> http_client::Result<Client<ExtBuilder::Output, H>> {
        let ext = self.ext.clone();

        self = ext.finish(self)?;
        let ext = Ext::build(&self)?;

        let ClientBuilder {
            http_client,
            base_url,
            mut headers,
            api_key,
            ..
        } = self;

        if let Some((k, v)) = api_key.into_header().transpose()? {
            headers.insert(k, v);
        }

        let http_client = http_client.unwrap_or_default();

        Ok(Client::from_parts(
            Arc::from(base_url.as_str()),
            Arc::new(headers),
            http_client,
            ext,
        ))
    }
}
