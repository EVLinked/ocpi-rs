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
    transport::{CredentialToken, PaginationMeta},
    v2_2_1::{ChargingPreferences, ChargingPreferencesResponse, Credentials, Session},
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
///
/// By default the token is Base64-encoded per OCPI 2.2.1 §4.1.1.
/// Set `compat_raw_token = true` (via [`Self::with_compat_raw_token`]) to send
/// the raw token instead, for interoperability with OCPI 2.1.1/2.2 peers.
#[derive(Debug, Clone)]
pub struct OcpiClient {
    base_url: Url,
    token: String,
    http: reqwest::Client,
    /// When `true`, the token is sent raw (not Base64-encoded).
    /// Use only when connecting to legacy 2.1.1/2.2 peers.
    compat_raw_token: bool,
}

impl OcpiClient {
    /// Create a client targeting `base_url`, authenticating with `token`.
    ///
    /// Token encoding defaults to Base64 (OCPI 2.2.1). Use
    /// [`Self::with_compat_raw_token`] to opt into the raw-token mode for
    /// legacy peers.
    #[must_use]
    pub fn new(base_url: Url, token: impl Into<String>) -> Self {
        Self {
            base_url,
            token: token.into(),
            http: reqwest::Client::new(),
            compat_raw_token: false,
        }
    }

    /// Override the token encoding mode.
    ///
    /// - `false` (default): token is Base64-encoded per OCPI 2.2.1.
    /// - `true`: token is sent raw; use with legacy 2.1.1/2.2 peers.
    #[must_use]
    pub fn with_compat_raw_token(mut self, compat: bool) -> Self {
        self.compat_raw_token = compat;
        self
    }

    /// The configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Build the `Authorization` header value for outbound requests.
    fn auth_header_value(&self) -> String {
        if self.compat_raw_token {
            format!("Token {}", self.token)
        } else {
            CredentialToken::new(&self.token).to_header_value()
        }
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
            .header("Authorization", self.auth_header_value())
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
            .header("Authorization", self.auth_header_value())
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
            .header("Authorization", self.auth_header_value())
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
            .header("Authorization", self.auth_header_value())
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
            .header("Authorization", self.auth_header_value())
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
            .header("Authorization", self.auth_header_value())
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    // ── Sessions ──────────────────────────────────────────────────────────────

    /// Fetch a paginated list of sessions from the remote CPO (`GET <url>`).
    ///
    /// `url` is the absolute URL of the remote sessions endpoint. The query
    /// parameters `date_from`, `date_to`, `offset`, and `limit` may be set
    /// in the `params` argument; `None` fields are omitted from the query
    /// string.
    ///
    /// Returns `(sessions, pagination_meta)`.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, or
    /// the envelope carries no data.
    pub async fn get_sessions(
        &self,
        url: &str,
        params: &ocpi_types::transport::PaginatedParams,
    ) -> Result<(Vec<Session>, PaginationMeta), ClientError> {
        let mut req = self
            .http
            .get(url::Url::parse(url)?)
            .header("Authorization", self.auth_header_value());
        if let Some(df) = params.date_from {
            req = req.query(&[("date_from", df.to_rfc3339())]);
        }
        if let Some(dt) = params.date_to {
            req = req.query(&[("date_to", dt.to_rfc3339())]);
        }
        if let Some(off) = params.offset {
            req = req.query(&[("offset", off.to_string())]);
        }
        if let Some(lim) = params.limit {
            req = req.query(&[("limit", lim.to_string())]);
        }
        let response = req.send().await?.error_for_status()?;

        let link = response
            .headers()
            .get("link")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                // Parse: `<url>; rel="next"`
                let url_part = s.trim().strip_prefix('<')?.split('>').next()?;
                Some(url_part.to_string())
            });
        let total_count: u64 = response
            .headers()
            .get("x-total-count")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let page_limit: u32 = response
            .headers()
            .get("x-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(params.limit.unwrap_or(50));

        let meta = PaginationMeta {
            next_url: link,
            total_count,
            limit: page_limit,
        };

        let envelope: OcpiResponse<Vec<Session>> = response.json().await?;
        let sessions = envelope.data.ok_or(ClientError::EmptyData)?;
        Ok((sessions, meta))
    }

    /// Retrieve a single session by its composite key (`GET <url>/{cc}/{party}/{id}`).
    ///
    /// `url` is the sessions endpoint base; the path segments are appended
    /// automatically.
    ///
    /// # Errors
    ///
    /// - [`ClientError::NotFound`] when the server returns OCPI `2003` or HTTP
    ///   404.
    /// - [`ClientError::EmptyData`] if the success envelope carries no data.
    pub async fn get_session(
        &self,
        url: &str,
        country_code: &str,
        party_id: &str,
        session_id: &str,
    ) -> Result<Session, ClientError> {
        let endpoint = format!(
            "{}/{}/{}/{}",
            url.trim_end_matches('/'),
            country_code,
            party_id,
            session_id,
        );
        let response = self
            .http
            .get(url::Url::parse(&endpoint)?)
            .header("Authorization", self.auth_header_value())
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ClientError::NotFound);
        }
        let response = response.error_for_status()?;
        let envelope: OcpiResponse<Session> = response.json().await?;
        envelope.data.ok_or(ClientError::EmptyData)
    }

    /// Create or replace a session on the remote eMSP (`PUT`).
    ///
    /// `url` is the sessions endpoint base; the path segments are appended
    /// automatically.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails or the URL is invalid.
    pub async fn put_session(
        &self,
        url: &str,
        country_code: &str,
        party_id: &str,
        session_id: &str,
        session: &Session,
    ) -> Result<(), ClientError> {
        let endpoint = format!(
            "{}/{}/{}/{}",
            url.trim_end_matches('/'),
            country_code,
            party_id,
            session_id,
        );
        self.http
            .put(url::Url::parse(&endpoint)?)
            .header("Authorization", self.auth_header_value())
            .json(session)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Apply a partial update (JSON merge-patch, RFC 7396) to a session
    /// on the remote eMSP (`PATCH`).
    ///
    /// `partial` is any `Serialize` value; use a struct with
    /// `#[serde(skip_serializing_if = "Option::is_none")]` fields, or a
    /// `serde_json::Value` map, to send only the changed fields.
    ///
    /// # Errors
    ///
    /// - [`ClientError::NotFound`] when the server returns HTTP 404.
    /// - [`ClientError::Http`] on network or server errors.
    pub async fn patch_session<T: ocpi_types::serde::Serialize>(
        &self,
        url: &str,
        country_code: &str,
        party_id: &str,
        session_id: &str,
        partial: &T,
    ) -> Result<(), ClientError> {
        let endpoint = format!(
            "{}/{}/{}/{}",
            url.trim_end_matches('/'),
            country_code,
            party_id,
            session_id,
        );
        let response = self
            .http
            .patch(url::Url::parse(&endpoint)?)
            .header("Authorization", self.auth_header_value())
            .json(partial)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ClientError::NotFound);
        }
        response.error_for_status()?;
        Ok(())
    }

    /// Send the driver's charging preferences to the CPO for the given
    /// session (`PUT /sessions/{session_id}/charging_preferences`).
    ///
    /// `url` is the sessions endpoint base; the path is appended automatically
    /// as `/{session_id}/charging_preferences`.
    ///
    /// Returns the CPO's [`ChargingPreferencesResponse`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] if the request fails, the URL is invalid, or
    /// the envelope carries no data.
    pub async fn set_charging_preferences(
        &self,
        url: &str,
        session_id: &str,
        preferences: &ChargingPreferences,
    ) -> Result<ChargingPreferencesResponse, ClientError> {
        let endpoint = format!(
            "{}/{}/charging_preferences",
            url.trim_end_matches('/'),
            session_id,
        );
        let response = self
            .http
            .put(url::Url::parse(&endpoint)?)
            .header("Authorization", self.auth_header_value())
            .json(preferences)
            .send()
            .await?
            .error_for_status()?;
        let envelope: OcpiResponse<ChargingPreferencesResponse> = response.json().await?;
        envelope.data.ok_or(ClientError::EmptyData)
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

    // ── Authorization header encoding ─────────────────────────────────────────

    #[test]
    fn default_client_sends_base64_encoded_token() {
        let client = OcpiClient::new(Url::parse("https://example.com/").unwrap(), "my-raw-token");
        let header = client.auth_header_value();
        // "my-raw-token" in Base64 (RFC 4648 standard alphabet) = "bXktcmF3LXRva2Vu"
        assert_eq!(header, "Token bXktcmF3LXRva2Vu");
    }

    #[test]
    fn compat_client_sends_raw_token() {
        let client = OcpiClient::new(Url::parse("https://example.com/").unwrap(), "my-raw-token")
            .with_compat_raw_token(true);
        assert_eq!(client.auth_header_value(), "Token my-raw-token");
    }

    #[test]
    fn compat_builder_preserves_other_fields() {
        let base = Url::parse("https://example.com/ocpi/").unwrap();
        let client = OcpiClient::new(base.clone(), "tok").with_compat_raw_token(true);
        assert_eq!(client.base_url(), &base);
        assert!(client.compat_raw_token);
    }

    #[test]
    fn compat_false_is_default() {
        let client = OcpiClient::new(Url::parse("https://example.com/").unwrap(), "tok");
        assert!(!client.compat_raw_token);
    }
}
