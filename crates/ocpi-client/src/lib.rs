//! # ocpi-client
//!
//! An async OCPI HTTP client for the **sender** role — the side that issues
//! requests to a remote party's endpoints (e.g. an eMSP pulling Locations from
//! a CPO, or either party performing the credentials handshake).
//!
//! The client is transport-only; all wire types come from [`ocpi_types`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;

pub use error::ClientError;

use ocpi_types::{version::Version, OcpiResponse};
use url::Url;

/// A configured OCPI client pointed at one remote party's API base URL.
///
/// The `base_url` should be the versioned module base (it is joined with
/// relative paths like `versions`), and `token` is the OCPI authorization
/// token presented as `Authorization: Token <token>`.
#[derive(Debug, Clone)]
pub struct OcpiClient {
    base_url: Url,
    token: String,
    http: reqwest::Client,
}

impl OcpiClient {
    /// Create a client targeting `base_url`, authenticating with `token`.
    #[must_use]
    pub fn new(base_url: Url, token: impl Into<String>) -> Self {
        Self {
            base_url,
            token: token.into(),
            http: reqwest::Client::new(),
        }
    }

    /// The configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Fetch the remote party's supported versions (`GET /versions`).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, or the
    /// envelope reports success without any data.
    pub async fn versions(&self) -> Result<Vec<Version>, ClientError> {
        let url = self.base_url.join("versions")?;
        let response = self
            .http
            .get(url)
            .header("Authorization", format!("Token {}", self.token))
            .send()
            .await?
            .error_for_status()?;
        let envelope: OcpiResponse<Vec<Version>> = response.json().await?;
        envelope.data.ok_or(ClientError::EmptyData)
    }
}

#[cfg(test)]
mod tests {
    use super::OcpiClient;
    use url::Url;

    #[test]
    fn builds_client_with_base_url() {
        let client = OcpiClient::new(
            Url::parse("https://example.com/ocpi/cpo/2.2.1/").unwrap(),
            "secret",
        );
        assert_eq!(
            client.base_url().as_str(),
            "https://example.com/ocpi/cpo/2.2.1/"
        );
    }
}
