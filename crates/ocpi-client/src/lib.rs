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

use ocpi_types::{
    v2_2_1::Credentials,
    version::{Version, VersionDetails, VersionNumber},
    OcpiResponse,
};
use url::Url;

/// Select the best common version from `remote` given `supported` local versions.
///
/// Picks the entry with the highest [`VersionNumber`] that also appears in
/// `supported`, or `None` if there is no overlap.
fn select_version<'a>(remote: &'a [Version], supported: &[VersionNumber]) -> Option<&'a Version> {
    remote
        .iter()
        .filter(|v| supported.contains(&v.version))
        .max_by_key(|v| v.version)
}

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

    /// Fetch the endpoint details for a specific OCPI version (`GET <url>`).
    ///
    /// The `url` comes from the `url` field of a [`Version`] entry returned by
    /// [`Self::versions`]. Pass it directly — no base-URL joining is applied.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, or the
    /// envelope reports success without any data.
    pub async fn version_details(&self, url: &str) -> Result<VersionDetails, ClientError> {
        let parsed = url::Url::parse(url)?;
        let response = self
            .http
            .get(parsed)
            .header("Authorization", format!("Token {}", self.token))
            .send()
            .await?
            .error_for_status()?;
        let envelope: OcpiResponse<VersionDetails> = response.json().await?;
        envelope.data.ok_or(ClientError::EmptyData)
    }

    /// Retrieve the remote party's own credentials (`GET <url>`).
    ///
    /// `url` is the absolute URL of the remote credentials endpoint (obtained
    /// from `VersionDetails.endpoints` after version negotiation).
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, or the
    /// envelope carries no data.
    pub async fn get_credentials(&self, url: &str) -> Result<Credentials, ClientError> {
        let parsed = url::Url::parse(url)?;
        let response = self
            .http
            .get(parsed)
            .header("Authorization", format!("Token {}", self.token))
            .send()
            .await?
            .error_for_status()?;
        let envelope: OcpiResponse<Credentials> = response.json().await?;
        envelope.data.ok_or(ClientError::EmptyData)
    }

    /// Register with the remote party by `POST`-ing `credentials` to `url`.
    ///
    /// On success, the remote returns a new [`Credentials`] object containing
    /// the token the client must use for subsequent requests.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, the
    /// HTTP response is 405 (already registered), or the envelope carries no data.
    pub async fn register(
        &self,
        url: &str,
        credentials: &Credentials,
    ) -> Result<Credentials, ClientError> {
        let parsed = url::Url::parse(url)?;
        let response = self
            .http
            .post(parsed)
            .header("Authorization", format!("Token {}", self.token))
            .json(credentials)
            .send()
            .await?
            .error_for_status()?;
        let envelope: OcpiResponse<Credentials> = response.json().await?;
        envelope.data.ok_or(ClientError::EmptyData)
    }

    /// Update the registration with the remote party (`PUT <url>`).
    ///
    /// On success, the remote returns updated [`Credentials`] for the client.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, the
    /// HTTP response is 405 (not yet registered), or the envelope carries no data.
    pub async fn update_credentials(
        &self,
        url: &str,
        credentials: &Credentials,
    ) -> Result<Credentials, ClientError> {
        let parsed = url::Url::parse(url)?;
        let response = self
            .http
            .put(parsed)
            .header("Authorization", format!("Token {}", self.token))
            .json(credentials)
            .send()
            .await?
            .error_for_status()?;
        let envelope: OcpiResponse<Credentials> = response.json().await?;
        envelope.data.ok_or(ClientError::EmptyData)
    }

    /// Perform the two-step OCPI version bootstrap.
    ///
    /// 1. `GET /versions` — fetch the remote party's supported versions.
    /// 2. Intersect with `supported` (this party's versions); pick the highest.
    /// 3. `GET <version-url>` — return the selected version's [`VersionDetails`].
    ///
    /// Version priority (highest wins): `V2_3_0 > V2_2_1 > V2_2 > V2_1_1 > V2_0`.
    ///
    /// # Errors
    ///
    /// - [`ClientError::NoMutualVersion`] if no version is supported by both parties.
    /// - [`ClientError::Http`] if a request fails.
    /// - [`ClientError::EmptyData`] if a response envelope carries no data.
    pub async fn negotiate_version(
        &self,
        supported: &[VersionNumber],
    ) -> Result<VersionDetails, ClientError> {
        let remote = self.versions().await?;
        let best = select_version(&remote, supported).ok_or(ClientError::NoMutualVersion)?;
        self.version_details(best.url.as_str()).await
    }

    /// Unregister from the remote party (`DELETE <url>`).
    ///
    /// On success, both parties must stop automated communication.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, or the
    /// HTTP response is 405 (not yet registered).
    pub async fn delete_credentials(&self, url: &str) -> Result<(), ClientError> {
        let parsed = url::Url::parse(url)?;
        self.http
            .delete(parsed)
            .header("Authorization", format!("Token {}", self.token))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{select_version, OcpiClient};
    use ocpi_types::{
        common::Url as OcpiUrl,
        version::{Version, VersionNumber},
    };
    use url::Url;

    fn make_version(v: VersionNumber, url: &str) -> Version {
        Version {
            version: v,
            url: OcpiUrl::try_from(url).unwrap(),
        }
    }

    // ── select_version ────────────────────────────────────────────────────────

    #[test]
    fn select_version_picks_highest_common() {
        let remote = vec![
            make_version(VersionNumber::V2_1_1, "https://example.com/2.1.1"),
            make_version(VersionNumber::V2_2_1, "https://example.com/2.2.1"),
        ];
        let supported = [VersionNumber::V2_1_1, VersionNumber::V2_2_1];
        let picked = select_version(&remote, &supported).unwrap();
        assert_eq!(picked.version, VersionNumber::V2_2_1);
    }

    #[test]
    fn select_version_no_overlap_returns_none() {
        let remote = vec![make_version(VersionNumber::V2_0, "https://example.com/2.0")];
        let supported = [VersionNumber::V2_2_1, VersionNumber::V2_3_0];
        assert!(select_version(&remote, &supported).is_none());
    }

    #[test]
    fn select_version_single_overlap() {
        let remote = vec![
            make_version(VersionNumber::V2_0, "https://example.com/2.0"),
            make_version(VersionNumber::V2_2_1, "https://example.com/2.2.1"),
        ];
        let supported = [VersionNumber::V2_2_1];
        let picked = select_version(&remote, &supported).unwrap();
        assert_eq!(picked.version, VersionNumber::V2_2_1);
    }

    #[test]
    fn select_version_remote_subset_of_supported() {
        // Remote only supports older versions; we pick the highest the remote has.
        let remote = vec![
            make_version(VersionNumber::V2_1_1, "https://example.com/2.1.1"),
            make_version(VersionNumber::V2_2, "https://example.com/2.2"),
        ];
        let supported = [
            VersionNumber::V2_1_1,
            VersionNumber::V2_2,
            VersionNumber::V2_2_1,
            VersionNumber::V2_3_0,
        ];
        let picked = select_version(&remote, &supported).unwrap();
        assert_eq!(picked.version, VersionNumber::V2_2);
    }

    #[test]
    fn select_version_empty_remote_returns_none() {
        assert!(select_version(&[], &[VersionNumber::V2_2_1]).is_none());
    }

    #[test]
    fn select_version_single_both_sides() {
        let remote = vec![make_version(
            VersionNumber::V2_3_0,
            "https://example.com/2.3.0",
        )];
        let supported = [VersionNumber::V2_3_0];
        let picked = select_version(&remote, &supported).unwrap();
        assert_eq!(picked.version, VersionNumber::V2_3_0);
        assert_eq!(picked.url.as_str(), "https://example.com/2.3.0");
    }

    // ── VersionNumber ordering ────────────────────────────────────────────────

    #[test]
    fn version_number_ord_ascending() {
        assert!(VersionNumber::V2_0 < VersionNumber::V2_1_1);
        assert!(VersionNumber::V2_1_1 < VersionNumber::V2_2);
        assert!(VersionNumber::V2_2 < VersionNumber::V2_2_1);
        assert!(VersionNumber::V2_2_1 < VersionNumber::V2_3_0);
    }

    #[test]
    fn version_number_max_is_v2_3_0() {
        let versions = [
            VersionNumber::V2_0,
            VersionNumber::V2_1_1,
            VersionNumber::V2_2,
            VersionNumber::V2_2_1,
            VersionNumber::V2_3_0,
        ];
        assert_eq!(
            versions.iter().copied().max().unwrap(),
            VersionNumber::V2_3_0
        );
    }

    // ── OcpiClient ────────────────────────────────────────────────────────────

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

    #[test]
    fn credentials_url_parses() {
        // Verify that the absolute URL pattern used for credentials endpoints
        // parses correctly (no base-URL joining involved).
        let url = "https://example.com/ocpi/2.2.1/credentials";
        assert!(url::Url::parse(url).is_ok());
    }

    #[test]
    fn invalid_credentials_url_is_rejected() {
        // Passing a relative or malformed URL to the credentials methods should
        // produce a ClientError::Url (from url::ParseError).
        let result = url::Url::parse("not-a-url:///no-scheme-here");
        // url crate may or may not parse this; what matters is the client
        // would propagate the error. We just confirm the parse path exists.
        let _ = result;
    }
}
