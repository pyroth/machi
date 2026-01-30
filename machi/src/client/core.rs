//! Core client structure and HTTP transport implementation.

use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue};

use crate::{
    core::wasm_compat::{WasmCompatSend, WasmCompatSync},
    http::{
        self as http_client, Builder, HttpClientExt, LazyBody, MultipartForm, Request, Response,
    },
};

use super::{
    auth::{ApiKey, Nothing},
    builder::{ClientBuilder, NeedsApiKey},
    error::VerifyError,
    traits::{DebugExt, Provider, ProviderBuilder, VerifyClient},
};

/// HTTP transport type indicator.
pub enum Transport {
    /// Standard HTTP request.
    Http,
    /// Server-Sent Events stream.
    Sse,
    /// Newline-delimited JSON stream.
    NdJson,
}

/// The main client structure for interacting with LLM providers.
///
/// `Client` is generic over:
/// - `Ext`: Provider extension type (e.g., `OpenAI`, Anthropic specifics)
/// - `H`: HTTP client backend (defaults to reqwest)
#[derive(Clone)]
pub struct Client<Ext = Nothing, H = reqwest::Client> {
    base_url: Arc<str>,
    headers: Arc<HeaderMap>,
    http_client: H,
    ext: Ext,
}

impl<Ext, H> Debug for Client<Ext, H>
where
    Ext: DebugExt,
    H: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = &mut f.debug_struct("Client");

        d = d
            .field("base_url", &self.base_url)
            .field(
                "headers",
                &self
                    .headers
                    .iter()
                    .filter_map(|(k, v)| {
                        // Hide sensitive headers in debug output
                        if k == http::header::AUTHORIZATION || k.as_str().contains("api-key") {
                            None
                        } else {
                            Some((k, v))
                        }
                    })
                    .collect::<Vec<(&HeaderName, &HeaderValue)>>(),
            )
            .field("http_client", &self.http_client);

        self.ext
            .fields()
            .fold(d, |d, (name, field)| d.field(name, field))
            .finish()
    }
}

impl<Ext, ExtBuilder, Key, H> Client<Ext, H>
where
    ExtBuilder: Clone + Default + ProviderBuilder<Output = Ext, ApiKey = Key>,
    Ext: Provider<Builder = ExtBuilder>,
    H: Default + HttpClientExt,
    Key: ApiKey,
{
    /// Creates a new client with the given API key.
    pub fn new(api_key: impl Into<Key>) -> http_client::Result<Self> {
        Self::builder().api_key(api_key).build()
    }
}

impl<Ext, H> Client<Ext, H> {
    /// Returns the base URL of this client.
    #[inline]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns the default headers of this client.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns a reference to the provider extension.
    #[inline]
    pub const fn ext(&self) -> &Ext {
        &self.ext
    }

    /// Creates a new client with a different extension type.
    pub fn with_ext<NewExt>(self, new_ext: NewExt) -> Client<NewExt, H> {
        Client {
            base_url: self.base_url,
            headers: self.headers,
            http_client: self.http_client,
            ext: new_ext,
        }
    }

    /// Internal constructor used by `ClientBuilder`.
    pub(crate) const fn from_parts(
        base_url: Arc<str>,
        headers: Arc<HeaderMap>,
        http_client: H,
        ext: Ext,
    ) -> Self {
        Self {
            base_url,
            headers,
            http_client,
            ext,
        }
    }
}

impl<Ext, H> HttpClientExt for Client<Ext, H>
where
    H: HttpClientExt + 'static,
    Ext: WasmCompatSend + WasmCompatSync + 'static,
{
    fn send<T, U>(
        &self,
        mut req: Request<T>,
    ) -> impl Future<Output = http_client::Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        T: Into<Bytes> + WasmCompatSend,
        U: From<Bytes>,
        U: WasmCompatSend + 'static,
    {
        req.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );

        self.http_client.send(req)
    }

    fn send_multipart<U>(
        &self,
        req: Request<MultipartForm>,
    ) -> impl Future<Output = http_client::Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        U: From<Bytes>,
        U: WasmCompatSend + 'static,
    {
        self.http_client.send_multipart(req)
    }

    fn send_streaming<T>(
        &self,
        mut req: Request<T>,
    ) -> impl Future<Output = http_client::Result<http_client::StreamingResponse>> + WasmCompatSend
    where
        T: Into<Bytes>,
    {
        req.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );

        self.http_client.send_streaming(req)
    }
}

impl<Ext, Builder, H> Client<Ext, H>
where
    H: Default + HttpClientExt,
    Ext: Provider<Builder = Builder>,
    Builder: Default + ProviderBuilder,
{
    /// Returns a builder for constructing a new client.
    #[must_use]
    pub fn builder() -> ClientBuilder<Builder, NeedsApiKey, H> {
        ClientBuilder::default()
    }
}

impl<Ext, H> Client<Ext, H>
where
    Ext: Provider,
{
    /// Creates a POST request builder for the given path.
    pub fn post<S>(&self, path: S) -> http_client::Result<Builder>
    where
        S: AsRef<str>,
    {
        let uri = self
            .ext
            .build_uri(&self.base_url, path.as_ref(), Transport::Http);

        let mut req = Request::post(uri);

        if let Some(hs) = req.headers_mut() {
            hs.extend(self.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        self.ext.with_custom(req)
    }

    /// Creates a POST request builder for SSE endpoints.
    pub fn post_sse<S>(&self, path: S) -> http_client::Result<Builder>
    where
        S: AsRef<str>,
    {
        let uri = self
            .ext
            .build_uri(&self.base_url, path.as_ref(), Transport::Sse);

        let mut req = Request::post(uri);

        if let Some(hs) = req.headers_mut() {
            hs.extend(self.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        self.ext.with_custom(req)
    }

    /// Creates a GET request builder for the given path.
    pub fn get<S>(&self, path: S) -> http_client::Result<Builder>
    where
        S: AsRef<str>,
    {
        let uri = self
            .ext
            .build_uri(&self.base_url, path.as_ref(), Transport::Http);

        let mut req = Request::get(uri);

        if let Some(hs) = req.headers_mut() {
            hs.extend(self.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        self.ext.with_custom(req)
    }
}

impl<Ext, H> VerifyClient for Client<Ext, H>
where
    H: HttpClientExt,
    Ext: DebugExt + Provider + WasmCompatSync,
{
    async fn verify(&self) -> Result<(), VerifyError> {
        use http::StatusCode;

        let req = self
            .get(Ext::VERIFY_PATH)?
            .body(http_client::NoBody)
            .map_err(http_client::Error::from)?;

        let response = self.http_client.send(req).await?;

        match response.status() {
            StatusCode::OK => Ok(()),
            StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                Err(VerifyError::InvalidAuthentication)
            }
            StatusCode::INTERNAL_SERVER_ERROR => {
                let text = http_client::text(response).await?;
                Err(VerifyError::ProviderError(text))
            }
            status if status.as_u16() == 529 => {
                let text = http_client::text(response).await?;
                Err(VerifyError::ProviderError(text))
            }
            _ => {
                let status = response.status();

                if status.is_success() {
                    Ok(())
                } else {
                    let text: String = String::from_utf8_lossy(&response.into_body().await?).into();
                    Err(VerifyError::HttpError(http_client::Error::Instance(
                        format!("Failed with '{status}': {text}").into(),
                    )))
                }
            }
        }
    }
}

/// A wrapper type providing runtime checks on a provider's capabilities.
pub struct Capable<M>(PhantomData<M>);

impl<M> super::traits::Capability for Capable<M> {
    const CAPABLE: bool = true;
}
