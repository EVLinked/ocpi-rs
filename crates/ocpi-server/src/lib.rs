//! # ocpi-server
//!
//! Server-side building blocks for the **receiver** role — the side that
//! exposes OCPI endpoints and is called by remote parties.
//!
//! The core is framework-agnostic: you implement handler traits such as
//! [`VersionsHandler`] or [`CredentialsHandler`]. Enable the `axum` feature
//! for ready-made routers (see the `http` module).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use ocpi_types::{
    v2_2_1::{
        AuthorizationInfo, Cdr, Credentials, LocationReferences, Session, Tariff, Token, TokenType,
    },
    version::{Version, VersionDetails, VersionNumber},
    DateTime, OcpiStatusCode, Utc,
};

// ── ServerError ───────────────────────────────────────────────────────────────

/// An error raised while handling an inbound OCPI request.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// A wrapped error originating from the type layer.
    #[error(transparent)]
    Ocpi(#[from] ocpi_types::OcpiError),

    /// The caller's token was missing or not recognised.
    #[error("unauthorized")]
    Unauthorized,

    /// The requested operation is not yet implemented.
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),

    /// A `POST /credentials` was received from a party that is already
    /// registered. The axum layer should respond with HTTP 405.
    #[error("already registered")]
    AlreadyRegistered,

    /// A `PUT` or `DELETE /credentials` was received from a party that has
    /// not yet registered. The axum layer should respond with HTTP 405.
    #[error("not registered")]
    NotRegistered,

    /// The requested resource was not found (unknown ID in the store).
    ///
    /// Maps to OCPI status code `2003` (Unknown Location).
    #[error("not found")]
    NotFound,

    /// A real-time authorization was requested for a token the eMSP does not know.
    ///
    /// Maps to OCPI status code `2004` (Unknown Token).
    #[error("unknown token")]
    UnknownToken,
}

impl ServerError {
    /// Map this error to the OCPI status code that should be returned in the
    /// response envelope.
    #[must_use]
    pub fn status_code(&self) -> OcpiStatusCode {
        match self {
            Self::Ocpi(ocpi_types::OcpiError::Status(code)) => *code,
            Self::Unauthorized => OcpiStatusCode::ClientError,
            Self::AlreadyRegistered | Self::NotRegistered => OcpiStatusCode::ClientError,
            Self::NotFound => OcpiStatusCode::UnknownLocation,
            Self::UnknownToken => OcpiStatusCode::UnknownToken,
            Self::Ocpi(_) | Self::NotImplemented(_) => OcpiStatusCode::ServerError,
        }
    }
}

// ── VersionsHandler ───────────────────────────────────────────────────────────

/// Handles the OCPI versions / version-details endpoints (receiver role).
///
/// Implementors return the list of supported OCPI versions and the endpoint
/// catalogue for each version.
///
/// The axum integration in [`http::versions_router`] accepts any [`VersionsConfig`]
/// directly. This trait is provided for custom, dynamic, or async-backed
/// implementations.
#[allow(async_fn_in_trait)]
pub trait VersionsHandler {
    /// Return all supported OCPI versions (`GET /versions`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if the versions cannot be retrieved.
    async fn list_versions(&self) -> Result<Vec<Version>, ServerError>;

    /// Return the endpoint catalogue for a specific OCPI version
    /// (`GET /versions/{version}`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Ocpi`] with
    /// [`OcpiStatusCode::UnsupportedVersion`] when the version is not
    /// supported.
    async fn version_details(&self, version: VersionNumber) -> Result<VersionDetails, ServerError>;
}

// ── VersionsConfig ────────────────────────────────────────────────────────────

/// A static in-memory version registry for use with [`http::versions_router`].
///
/// Populate this at server startup with the versions and endpoint URLs your
/// OCPI node exposes.
#[derive(Debug, Clone)]
pub struct VersionsConfig {
    /// Ordered list of supported versions (returned by `GET /versions`).
    pub versions: Vec<Version>,
    /// Endpoint catalogue keyed by version number.
    pub details: std::collections::HashMap<VersionNumber, VersionDetails>,
}

impl VersionsConfig {
    /// Create an empty registry; add entries with
    /// [`add_version`](Self::add_version).
    #[must_use]
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
            details: std::collections::HashMap::new(),
        }
    }

    /// Register a version and its endpoint catalogue.
    pub fn add_version(&mut self, entry: Version, details: VersionDetails) {
        self.versions.push(entry);
        self.details.insert(details.version, details);
    }
}

impl Default for VersionsConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(async_fn_in_trait)]
impl VersionsHandler for VersionsConfig {
    async fn list_versions(&self) -> Result<Vec<Version>, ServerError> {
        Ok(self.versions.clone())
    }

    async fn version_details(&self, version: VersionNumber) -> Result<VersionDetails, ServerError> {
        self.details
            .get(&version)
            .cloned()
            .ok_or(ServerError::Ocpi(ocpi_types::OcpiError::Status(
                OcpiStatusCode::UnsupportedVersion,
            )))
    }
}

// ── CredentialsHandler ────────────────────────────────────────────────────────

/// Handles the OCPI credentials / registration handshake (receiver role).
///
/// All four spec methods are required. Implementors are responsible for:
/// - Persisting/revoking credentials tokens.
/// - Returning [`ServerError::AlreadyRegistered`] from `register` when the
///   caller is already known (the axum layer turns this into HTTP 405).
/// - Returning [`ServerError::NotRegistered`] from `update_credentials` and
///   `delete_credentials` when the caller is not yet registered (HTTP 405).
/// - Calling [`Credentials::check_single_role`] if multi-role is not yet
///   supported, and returning [`ServerError::Ocpi`] wrapping
///   [`OcpiStatusCode::ServerError`].
///
/// Spec: `specs/ocpi/2.2.1/credentials.asciidoc`
#[allow(async_fn_in_trait)]
pub trait CredentialsHandler {
    /// Return this server's own [`Credentials`] (`GET /credentials`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Unauthorized`] when `token` is not recognised.
    async fn get_credentials(&self, token: &str) -> Result<Credentials, ServerError>;

    /// Register a new party and return the server's credentials for them
    /// (`POST /credentials`).
    ///
    /// # Errors
    ///
    /// - [`ServerError::Unauthorized`] — `token` not recognised.
    /// - [`ServerError::AlreadyRegistered`] — caller already registered (→ HTTP 405).
    async fn register(
        &self,
        token: &str,
        credentials: Credentials,
    ) -> Result<Credentials, ServerError>;

    /// Update an existing registration and return the refreshed server
    /// credentials (`PUT /credentials`).
    ///
    /// # Errors
    ///
    /// - [`ServerError::Unauthorized`] — `token` not recognised.
    /// - [`ServerError::NotRegistered`] — caller not yet registered (→ HTTP 405).
    async fn update_credentials(
        &self,
        token: &str,
        credentials: Credentials,
    ) -> Result<Credentials, ServerError>;

    /// Revoke a registration (`DELETE /credentials`).
    ///
    /// # Errors
    ///
    /// - [`ServerError::Unauthorized`] — `token` not recognised.
    /// - [`ServerError::NotRegistered`] — caller not yet registered (→ HTTP 405).
    async fn delete_credentials(&self, token: &str) -> Result<(), ServerError>;
}

// ── SessionsHandler ───────────────────────────────────────────────────────────

/// Handles the OCPI Sessions module endpoints.
///
/// Implements both the **sender** interface (CPO exposes `GET /sessions`) and
/// the **receiver** interface (eMSP exposes `GET/PUT/PATCH
/// /sessions/{country_code}/{party_id}/{session_id}`).
///
/// Spec: `specs/ocpi/2.2.1/mod_sessions.asciidoc`
#[allow(async_fn_in_trait)]
pub trait SessionsHandler {
    /// Paginated list of sessions whose `last_updated` is in
    /// `[date_from, date_to)` — sender interface (`GET /sessions`).
    ///
    /// Returns `(page_items, total_count)`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if the query cannot be executed.
    async fn get_sessions(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Session>, u32), ServerError>;

    /// Fetch a single session by its composite key — receiver interface
    /// (`GET /sessions/{country_code}/{party_id}/{session_id}`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the session does not exist.
    async fn get_session(
        &self,
        country_code: &str,
        party_id: &str,
        session_id: &str,
    ) -> Result<Session, ServerError>;

    /// Create or replace a session — receiver interface (`PUT`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] on storage failure.
    async fn put_session(
        &self,
        country_code: &str,
        party_id: &str,
        session_id: &str,
        session: Session,
    ) -> Result<(), ServerError>;

    /// Apply a JSON merge-patch (RFC 7396) to an existing session — receiver
    /// interface (`PATCH`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the session does not exist, or
    /// [`ServerError::NotImplemented`] if serialization fails.
    async fn patch_session(
        &self,
        country_code: &str,
        party_id: &str,
        session_id: &str,
        partial: ocpi_types::serde_json::Value,
    ) -> Result<(), ServerError>;
}

// ── SessionsConfig ────────────────────────────────────────────────────────────

/// Thread-safe in-memory sessions store for use with [`http::sessions_router`].
///
/// Sessions are keyed by `"{country_code}/{party_id}/{session_id}"`. Wrap in
/// `Arc` to share across axum handlers or multiple threads.
pub struct SessionsConfig {
    sessions: std::sync::RwLock<std::collections::HashMap<String, Session>>,
}

impl std::fmt::Debug for SessionsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionsConfig")
            .field(
                "session_count",
                &self.sessions.read().map(|m| m.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl SessionsConfig {
    /// Create an empty sessions store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    fn composite_key(country_code: &str, party_id: &str, session_id: &str) -> String {
        format!("{country_code}/{party_id}/{session_id}")
    }

    /// Insert or replace a session.
    pub fn put(&self, country_code: &str, party_id: &str, session_id: &str, session: Session) {
        let key = Self::composite_key(country_code, party_id, session_id);
        self.sessions
            .write()
            .expect("lock not poisoned")
            .insert(key, session);
    }

    /// Retrieve a session by its composite key.
    #[must_use]
    pub fn get(&self, country_code: &str, party_id: &str, session_id: &str) -> Option<Session> {
        let key = Self::composite_key(country_code, party_id, session_id);
        self.sessions
            .read()
            .expect("lock not poisoned")
            .get(&key)
            .cloned()
    }

    /// Apply a JSON merge-patch to an existing session.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] if no session matches the key.
    pub fn patch_json(
        &self,
        country_code: &str,
        party_id: &str,
        session_id: &str,
        partial: ocpi_types::serde_json::Value,
    ) -> Result<(), ServerError> {
        let key = Self::composite_key(country_code, party_id, session_id);
        let mut map = self.sessions.write().expect("lock not poisoned");
        let session = map.get(&key).ok_or(ServerError::NotFound)?;
        let mut base = ocpi_types::serde_json::to_value(session.clone())
            .map_err(|_| ServerError::NotImplemented("patch serialize"))?;
        json_merge(&mut base, partial);
        let updated: Session = ocpi_types::serde_json::from_value(base)
            .map_err(|_| ServerError::NotImplemented("patch deserialize"))?;
        map.insert(key, updated);
        Ok(())
    }

    /// Return a filtered and paginated slice of sessions.
    ///
    /// Filters by `last_updated >= date_from` and (if provided)
    /// `last_updated < date_to`. Results are sorted by `last_updated`.
    ///
    /// Returns `(page_items, total_matching_count)`.
    #[must_use]
    pub fn list(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> (Vec<Session>, u32) {
        let map = self.sessions.read().expect("lock not poisoned");
        let mut filtered: Vec<&Session> = map
            .values()
            .filter(|s| s.last_updated >= date_from && date_to.is_none_or(|dt| s.last_updated < dt))
            .collect();
        filtered.sort_by_key(|s| s.last_updated);
        let total = filtered.len() as u32;
        let page: Vec<Session> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        (page, total)
    }
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(async_fn_in_trait)]
impl SessionsHandler for SessionsConfig {
    async fn get_sessions(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Session>, u32), ServerError> {
        Ok(self.list(date_from, date_to, offset, limit))
    }

    async fn get_session(
        &self,
        country_code: &str,
        party_id: &str,
        session_id: &str,
    ) -> Result<Session, ServerError> {
        self.get(country_code, party_id, session_id)
            .ok_or(ServerError::NotFound)
    }

    async fn put_session(
        &self,
        country_code: &str,
        party_id: &str,
        session_id: &str,
        session: Session,
    ) -> Result<(), ServerError> {
        self.put(country_code, party_id, session_id, session);
        Ok(())
    }

    async fn patch_session(
        &self,
        country_code: &str,
        party_id: &str,
        session_id: &str,
        partial: ocpi_types::serde_json::Value,
    ) -> Result<(), ServerError> {
        self.patch_json(country_code, party_id, session_id, partial)
    }
}

// ── CdrsHandler ───────────────────────────────────────────────────────────────

/// Handles the OCPI CDRs module endpoints.
///
/// Implements both the **sender** interface (CPO exposes `GET /cdrs`) and the
/// **receiver** interface (eMSP exposes `POST /cdrs`).
///
/// Spec: `specs/ocpi/2.2.1/mod_cdrs.asciidoc`
#[allow(async_fn_in_trait)]
pub trait CdrsHandler {
    /// Paginated list of CDRs whose `last_updated` is in `[date_from, date_to)`
    /// — sender interface (`GET /cdrs`).
    ///
    /// Returns `(page_items, total_count)`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if the query cannot be executed.
    async fn get_cdrs(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Cdr>, u32), ServerError>;

    /// Fetch a single CDR by its ID — sender interface (`GET /cdrs/{cdr_id}`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the CDR does not exist.
    async fn get_cdr(&self, cdr_id: &str) -> Result<Cdr, ServerError>;

    /// Store a new CDR and return its URL — receiver interface (`POST /cdrs`).
    ///
    /// The returned `String` is the absolute URL at which the stored CDR can be
    /// retrieved (used for the HTTP `Location` response header).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] on storage failure.
    async fn post_cdr(&self, cdr: Cdr) -> Result<String, ServerError>;
}

// ── CdrsConfig ────────────────────────────────────────────────────────────────

/// Thread-safe in-memory CDR store for use with [`http::cdrs_router`].
///
/// CDRs are keyed by their `id`. The `base_url` (e.g.
/// `"https://example.com/ocpi/2.2.1"`) is prepended to construct the
/// `Location` header returned by `POST /cdrs`.
pub struct CdrsConfig {
    base_url: String,
    cdrs: std::sync::RwLock<std::collections::HashMap<String, Cdr>>,
}

impl std::fmt::Debug for CdrsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdrsConfig")
            .field("cdr_count", &self.cdrs.read().map(|m| m.len()).unwrap_or(0))
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl CdrsConfig {
    /// Create an empty CDR store.
    ///
    /// `base_url` is used to build the `Location` header on `POST /cdrs`
    /// (e.g. `"https://example.com/ocpi/2.2.1"`).
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            cdrs: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Construct the URL for a CDR by its ID.
    fn cdr_url(&self, cdr_id: &str) -> String {
        format!("{}/cdrs/{cdr_id}", self.base_url.trim_end_matches('/'))
    }

    /// Store a CDR and return its URL.
    pub fn store(&self, cdr: Cdr) -> String {
        let id = cdr.id.as_str().to_string();
        let url = self.cdr_url(&id);
        self.cdrs
            .write()
            .expect("lock not poisoned")
            .insert(id, cdr);
        url
    }

    /// Retrieve a CDR by its ID.
    #[must_use]
    pub fn get(&self, cdr_id: &str) -> Option<Cdr> {
        self.cdrs
            .read()
            .expect("lock not poisoned")
            .get(cdr_id)
            .cloned()
    }

    /// Return a filtered and paginated slice of CDRs.
    ///
    /// Filters by `last_updated >= date_from` and (if provided)
    /// `last_updated < date_to`. Results are sorted by `last_updated`.
    ///
    /// Returns `(page_items, total_matching_count)`.
    #[must_use]
    pub fn list(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> (Vec<Cdr>, u32) {
        let map = self.cdrs.read().expect("lock not poisoned");
        let mut filtered: Vec<&Cdr> = map
            .values()
            .filter(|c| c.last_updated >= date_from && date_to.is_none_or(|dt| c.last_updated < dt))
            .collect();
        filtered.sort_by_key(|c| c.last_updated);
        let total = filtered.len() as u32;
        let page: Vec<Cdr> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        (page, total)
    }
}

impl Default for CdrsConfig {
    fn default() -> Self {
        Self::new("")
    }
}

#[allow(async_fn_in_trait)]
impl CdrsHandler for CdrsConfig {
    async fn get_cdrs(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Cdr>, u32), ServerError> {
        Ok(self.list(date_from, date_to, offset, limit))
    }

    async fn get_cdr(&self, cdr_id: &str) -> Result<Cdr, ServerError> {
        self.get(cdr_id).ok_or(ServerError::NotFound)
    }

    async fn post_cdr(&self, cdr: Cdr) -> Result<String, ServerError> {
        Ok(self.store(cdr))
    }
}

// ── TariffsHandler ────────────────────────────────────────────────────────────

/// Handles the OCPI Tariffs module endpoints.
///
/// Implements the **sender** interface (CPO exposes `GET /tariffs`) and the
/// **receiver** interface (eMSP exposes `GET/PUT/DELETE
/// /tariffs/{country_code}/{party_id}/{tariff_id}`).
///
/// Spec: `specs/ocpi/2.2.1/mod_tariffs.asciidoc`
#[allow(async_fn_in_trait)]
pub trait TariffsHandler {
    /// Paginated list of tariffs whose `last_updated` is in
    /// `[date_from, date_to)` — sender interface (`GET /tariffs`).
    ///
    /// Returns `(page_items, total_count)`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if the query cannot be executed.
    async fn get_tariffs(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Tariff>, u32), ServerError>;

    /// Fetch a single tariff by its composite key — receiver interface
    /// (`GET /tariffs/{country_code}/{party_id}/{tariff_id}`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the tariff does not exist.
    async fn get_tariff(
        &self,
        country_code: &str,
        party_id: &str,
        tariff_id: &str,
    ) -> Result<Tariff, ServerError>;

    /// Create or replace a tariff — receiver interface (`PUT`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] on storage failure.
    async fn put_tariff(
        &self,
        country_code: &str,
        party_id: &str,
        tariff_id: &str,
        tariff: Tariff,
    ) -> Result<(), ServerError>;

    /// Delete a tariff — receiver interface (`DELETE`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the tariff does not exist.
    async fn delete_tariff(
        &self,
        country_code: &str,
        party_id: &str,
        tariff_id: &str,
    ) -> Result<(), ServerError>;
}

// ── TariffsConfig ─────────────────────────────────────────────────────────────

/// Thread-safe in-memory tariffs store for use with [`http::tariffs_router`].
///
/// Tariffs are keyed by `"{country_code}/{party_id}/{tariff_id}"`. Wrap in
/// `Arc` to share across axum handlers or multiple threads.
pub struct TariffsConfig {
    tariffs: std::sync::RwLock<std::collections::HashMap<String, Tariff>>,
}

impl std::fmt::Debug for TariffsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TariffsConfig")
            .field(
                "tariff_count",
                &self.tariffs.read().map(|m| m.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl TariffsConfig {
    /// Create an empty tariffs store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tariffs: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    fn composite_key(country_code: &str, party_id: &str, tariff_id: &str) -> String {
        format!("{country_code}/{party_id}/{tariff_id}")
    }

    /// Insert or replace a tariff.
    pub fn put(&self, country_code: &str, party_id: &str, tariff_id: &str, tariff: Tariff) {
        let key = Self::composite_key(country_code, party_id, tariff_id);
        self.tariffs
            .write()
            .expect("lock not poisoned")
            .insert(key, tariff);
    }

    /// Retrieve a tariff by its composite key.
    #[must_use]
    pub fn get(&self, country_code: &str, party_id: &str, tariff_id: &str) -> Option<Tariff> {
        let key = Self::composite_key(country_code, party_id, tariff_id);
        self.tariffs
            .read()
            .expect("lock not poisoned")
            .get(&key)
            .cloned()
    }

    /// Remove a tariff by its composite key.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] if no tariff matches the key.
    pub fn delete(
        &self,
        country_code: &str,
        party_id: &str,
        tariff_id: &str,
    ) -> Result<(), ServerError> {
        let key = Self::composite_key(country_code, party_id, tariff_id);
        let mut map = self.tariffs.write().expect("lock not poisoned");
        if map.remove(&key).is_some() {
            Ok(())
        } else {
            Err(ServerError::NotFound)
        }
    }

    /// Return a filtered and paginated slice of tariffs.
    ///
    /// Filters by `last_updated >= date_from` and (if provided)
    /// `last_updated < date_to`. Results are sorted by `last_updated`.
    ///
    /// Returns `(page_items, total_matching_count)`.
    #[must_use]
    pub fn list(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> (Vec<Tariff>, u32) {
        let map = self.tariffs.read().expect("lock not poisoned");
        let mut filtered: Vec<&Tariff> = map
            .values()
            .filter(|t| t.last_updated >= date_from && date_to.is_none_or(|dt| t.last_updated < dt))
            .collect();
        filtered.sort_by_key(|t| t.last_updated);
        let total = filtered.len() as u32;
        let page: Vec<Tariff> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        (page, total)
    }
}

impl Default for TariffsConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(async_fn_in_trait)]
impl TariffsHandler for TariffsConfig {
    async fn get_tariffs(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Tariff>, u32), ServerError> {
        Ok(self.list(date_from, date_to, offset, limit))
    }

    async fn get_tariff(
        &self,
        country_code: &str,
        party_id: &str,
        tariff_id: &str,
    ) -> Result<Tariff, ServerError> {
        self.get(country_code, party_id, tariff_id)
            .ok_or(ServerError::NotFound)
    }

    async fn put_tariff(
        &self,
        country_code: &str,
        party_id: &str,
        tariff_id: &str,
        tariff: Tariff,
    ) -> Result<(), ServerError> {
        self.put(country_code, party_id, tariff_id, tariff);
        Ok(())
    }

    async fn delete_tariff(
        &self,
        country_code: &str,
        party_id: &str,
        tariff_id: &str,
    ) -> Result<(), ServerError> {
        self.delete(country_code, party_id, tariff_id)
    }
}

// ── TokensHandler ─────────────────────────────────────────────────────────────

/// Handles the OCPI Tokens module endpoints.
///
/// Implements the **receiver** interface (CPO receives token updates from eMSP),
/// the **sender** interface (eMSP exposes `GET /tokens` list for CPO pull), and
/// the real-time **authorize** endpoint (eMSP receiver, CPO sender).
///
/// Spec: `specs/ocpi/2.2.1/mod_tokens.asciidoc`
#[allow(async_fn_in_trait)]
pub trait TokensHandler {
    /// Paginated list of tokens — sender interface (`GET /tokens`).
    ///
    /// Returns `(page_items, total_count)`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if the query cannot be executed.
    async fn get_tokens(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Token>, u32), ServerError>;

    /// Fetch a single token by its composite key — receiver interface
    /// (`GET /tokens/{country_code}/{party_id}/{token_uid}?type=`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the token does not exist.
    async fn get_token(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
    ) -> Result<Token, ServerError>;

    /// Create or replace a token — receiver interface (`PUT`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] on storage failure.
    async fn put_token(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
        token: Token,
    ) -> Result<(), ServerError>;

    /// Apply a JSON merge-patch (RFC 7396) to an existing token — receiver
    /// interface (`PATCH`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the token does not exist.
    async fn patch_token(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
        partial: ocpi_types::serde_json::Value,
    ) -> Result<(), ServerError>;

    /// Real-time authorization — sender interface
    /// (`POST /tokens/{token_uid}/authorize?type=`).
    ///
    /// Returns [`AuthorizationInfo`] when the token is known.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::UnknownToken`] (OCPI 2004) when the token is
    /// not found in this eMSP's system.
    async fn authorize(
        &self,
        token_uid: &str,
        token_type: TokenType,
        location: Option<LocationReferences>,
    ) -> Result<AuthorizationInfo, ServerError>;
}

// ── TokensConfig ──────────────────────────────────────────────────────────────

/// Thread-safe in-memory tokens store for use with [`http::tokens_router`].
///
/// Tokens are keyed by `"{country_code}/{party_id}/{token_uid}/{token_type}"`.
/// Wrap in `Arc` to share across axum handlers or multiple threads.
pub struct TokensConfig {
    tokens: std::sync::RwLock<std::collections::HashMap<String, Token>>,
}

impl std::fmt::Debug for TokensConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokensConfig")
            .field(
                "token_count",
                &self.tokens.read().map(|m| m.len()).unwrap_or(0),
            )
            .finish()
    }
}

fn token_type_str(t: TokenType) -> &'static str {
    match t {
        TokenType::AdHocUser => "AD_HOC_USER",
        TokenType::AppUser => "APP_USER",
        TokenType::Other => "OTHER",
        TokenType::Rfid => "RFID",
    }
}

impl TokensConfig {
    /// Create an empty tokens store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    fn composite_key(
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
    ) -> String {
        format!(
            "{country_code}/{party_id}/{token_uid}/{}",
            token_type_str(token_type)
        )
    }

    /// Insert or replace a token.
    pub fn put(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
        token: Token,
    ) {
        let key = Self::composite_key(country_code, party_id, token_uid, token_type);
        self.tokens
            .write()
            .expect("lock not poisoned")
            .insert(key, token);
    }

    /// Retrieve a token by its composite key.
    #[must_use]
    pub fn get(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
    ) -> Option<Token> {
        let key = Self::composite_key(country_code, party_id, token_uid, token_type);
        self.tokens
            .read()
            .expect("lock not poisoned")
            .get(&key)
            .cloned()
    }

    /// Apply a JSON merge-patch to an existing token.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] if no token matches the key.
    pub fn patch_json(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
        partial: ocpi_types::serde_json::Value,
    ) -> Result<(), ServerError> {
        let key = Self::composite_key(country_code, party_id, token_uid, token_type);
        let mut map = self.tokens.write().expect("lock not poisoned");
        let token = map.get(&key).ok_or(ServerError::NotFound)?;
        let mut base = ocpi_types::serde_json::to_value(token.clone())
            .map_err(|_| ServerError::NotImplemented("patch serialize"))?;
        json_merge(&mut base, partial);
        let updated: Token = ocpi_types::serde_json::from_value(base)
            .map_err(|_| ServerError::NotImplemented("patch deserialize"))?;
        map.insert(key, updated);
        Ok(())
    }

    /// Return a filtered and paginated slice of tokens.
    ///
    /// Filters by `last_updated >= date_from` and (if provided)
    /// `last_updated < date_to`. Results are sorted by `last_updated`.
    ///
    /// Returns `(page_items, total_matching_count)`.
    #[must_use]
    pub fn list(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> (Vec<Token>, u32) {
        let map = self.tokens.read().expect("lock not poisoned");
        let mut filtered: Vec<&Token> = map
            .values()
            .filter(|t| t.last_updated >= date_from && date_to.is_none_or(|dt| t.last_updated < dt))
            .collect();
        filtered.sort_by_key(|t| t.last_updated);
        let total = filtered.len() as u32;
        let page: Vec<Token> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        (page, total)
    }

    /// Perform a real-time authorization lookup by `uid` and `token_type`.
    ///
    /// Searches all stored tokens (regardless of owner party) for a match.
    /// Returns [`ServerError::UnknownToken`] (OCPI 2004) when not found.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::UnknownToken`] if no token with the given uid and
    /// type is known to this store.
    pub fn authorize(
        &self,
        token_uid: &str,
        token_type: TokenType,
        location: Option<LocationReferences>,
    ) -> Result<AuthorizationInfo, ServerError> {
        use ocpi_types::v2_2_1::AllowedType;
        let map = self.tokens.read().expect("lock not poisoned");
        let token = map
            .values()
            .find(|t| t.uid.as_str() == token_uid && t.token_type == token_type)
            .cloned()
            .ok_or(ServerError::UnknownToken)?;
        let allowed = if token.valid {
            AllowedType::Allowed
        } else {
            AllowedType::Blocked
        };
        let location = if matches!(allowed, AllowedType::Allowed) {
            location
        } else {
            None
        };
        Ok(AuthorizationInfo {
            allowed,
            token,
            location,
            authorization_reference: None,
            info: None,
        })
    }
}

impl Default for TokensConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(async_fn_in_trait)]
impl TokensHandler for TokensConfig {
    async fn get_tokens(
        &self,
        date_from: DateTime<Utc>,
        date_to: Option<DateTime<Utc>>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Token>, u32), ServerError> {
        Ok(self.list(date_from, date_to, offset, limit))
    }

    async fn get_token(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
    ) -> Result<Token, ServerError> {
        self.get(country_code, party_id, token_uid, token_type)
            .ok_or(ServerError::NotFound)
    }

    async fn put_token(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
        token: Token,
    ) -> Result<(), ServerError> {
        self.put(country_code, party_id, token_uid, token_type, token);
        Ok(())
    }

    async fn patch_token(
        &self,
        country_code: &str,
        party_id: &str,
        token_uid: &str,
        token_type: TokenType,
        partial: ocpi_types::serde_json::Value,
    ) -> Result<(), ServerError> {
        self.patch_json(country_code, party_id, token_uid, token_type, partial)
    }

    async fn authorize(
        &self,
        token_uid: &str,
        token_type: TokenType,
        location: Option<LocationReferences>,
    ) -> Result<AuthorizationInfo, ServerError> {
        self.authorize(token_uid, token_type, location)
    }
}

/// RFC 7396 JSON merge-patch: recursively apply `patch` onto `base`.
fn json_merge(base: &mut ocpi_types::serde_json::Value, patch: ocpi_types::serde_json::Value) {
    match patch {
        ocpi_types::serde_json::Value::Object(patch_map) => {
            if let ocpi_types::serde_json::Value::Object(base_map) = base {
                for (key, val) in patch_map {
                    if val.is_null() {
                        base_map.remove(&key);
                    } else {
                        json_merge(
                            base_map
                                .entry(key)
                                .or_insert(ocpi_types::serde_json::Value::Null),
                            val,
                        );
                    }
                }
            }
        }
        _ => *base = patch,
    }
}

// ── axum integration ──────────────────────────────────────────────────────────

#[cfg(feature = "axum")]
pub mod http {
    //! axum integration: ready-made routers for OCPI receiver endpoints.

    use std::sync::Arc;

    use axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::{get, post},
        Json, Router,
    };
    use ocpi_types::{
        envelope::{OcpiPaged, OcpiResponse},
        transport::PaginatedParams,
        v2_2_1::{AuthorizationInfo, Cdr, LocationReferences, Session, Tariff, Token, TokenType},
        version::{VersionDetails, VersionNumber},
        OcpiStatusCode,
    };

    use crate::{
        token_type_str, CdrsConfig, ServerError, SessionsConfig, TariffsConfig, TokensConfig,
        VersionsConfig,
    };

    // ── Versions ──────────────────────────────────────────────────────────────

    /// Build an axum router exposing `GET /versions` and `GET /versions/{version}`.
    ///
    /// Pass a [`VersionsConfig`] populated with the versions and endpoint URLs
    /// your OCPI node supports.
    pub fn versions_router(config: VersionsConfig) -> Router {
        Router::new()
            .route("/versions", get(list_versions))
            .route("/versions/{version}", get(version_details))
            .with_state(Arc::new(config))
    }

    async fn list_versions(State(cfg): State<Arc<VersionsConfig>>) -> Response {
        Json(OcpiResponse::success(cfg.versions.clone())).into_response()
    }

    async fn version_details(
        State(cfg): State<Arc<VersionsConfig>>,
        Path(version_str): Path<String>,
    ) -> Response {
        let version = match version_str.parse::<VersionNumber>() {
            Ok(v) => v,
            Err(_) => {
                return Json(OcpiResponse::<VersionDetails>::error(
                    OcpiStatusCode::InvalidParameters,
                    format!("unknown version: {version_str}"),
                ))
                .into_response();
            }
        };
        match cfg.details.get(&version).cloned() {
            Some(details) => Json(OcpiResponse::success(details)).into_response(),
            None => Json(OcpiResponse::<VersionDetails>::error(
                OcpiStatusCode::UnsupportedVersion,
                format!("version {version_str} not supported"),
            ))
            .into_response(),
        }
    }

    // ── Sessions ──────────────────────────────────────────────────────────────

    const DEFAULT_LIMIT: u32 = 50;

    /// Build an axum router for the OCPI Sessions module.
    ///
    /// Exposes:
    /// - `GET  /sessions` — paginated list (sender interface, CPO)
    /// - `GET  /sessions/{country_code}/{party_id}/{session_id}` — single
    /// - `PUT  /sessions/{country_code}/{party_id}/{session_id}` — upsert
    /// - `PATCH /sessions/{country_code}/{party_id}/{session_id}` — merge-patch
    ///
    /// OCPI routing headers (`OCPI-from/to-party-id/country-code`) are accepted
    /// on all routes; they are not enforced at this layer and can be validated
    /// by middleware in production deployments.
    pub fn sessions_router(config: Arc<SessionsConfig>) -> Router {
        Router::new()
            .route("/sessions", get(sessions_list))
            .route(
                "/sessions/{country_code}/{party_id}/{session_id}",
                get(sessions_get).put(sessions_put).patch(sessions_patch),
            )
            .with_state(config)
    }

    async fn sessions_list(
        State(cfg): State<Arc<SessionsConfig>>,
        Query(params): Query<PaginatedParams>,
    ) -> Response {
        use ocpi_types::chrono::TimeZone as _;
        let date_from = params.date_from.unwrap_or_else(|| {
            ocpi_types::Utc
                .with_ymd_and_hms(1970, 1, 1, 0, 0, 0)
                .single()
                .expect("epoch is valid")
        });
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);

        let (items, total) = cfg.list(date_from, params.date_to, offset, limit);
        let page = OcpiPaged::new(items, offset, limit, total);
        let next_offset = page.next_offset();
        let body = page.into_response();

        let mut response = Json(body).into_response();
        let hdrs = response.headers_mut();
        if let Ok(v) = total.to_string().parse() {
            hdrs.insert("x-total-count", v);
        }
        if let Ok(v) = limit.to_string().parse() {
            hdrs.insert("x-limit", v);
        }
        if let Some(next_off) = next_offset {
            let link = format!("</sessions?offset={next_off}&limit={limit}>; rel=\"next\"");
            if let Ok(v) = link.parse() {
                hdrs.insert("link", v);
            }
        }

        response
    }

    async fn sessions_get(
        State(cfg): State<Arc<SessionsConfig>>,
        Path((country_code, party_id, session_id)): Path<(String, String, String)>,
    ) -> Response {
        match cfg.get(&country_code, &party_id, &session_id) {
            Some(session) => Json(OcpiResponse::success(session)).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<Session>::error(
                    OcpiStatusCode::UnknownLocation,
                    format!("session {session_id} not found"),
                )),
            )
                .into_response(),
        }
    }

    async fn sessions_put(
        State(cfg): State<Arc<SessionsConfig>>,
        Path((country_code, party_id, session_id)): Path<(String, String, String)>,
        Json(session): Json<Session>,
    ) -> Response {
        cfg.put(&country_code, &party_id, &session_id, session);
        Json(OcpiResponse::<Session>::success_empty()).into_response()
    }

    async fn sessions_patch(
        State(cfg): State<Arc<SessionsConfig>>,
        Path((country_code, party_id, session_id)): Path<(String, String, String)>,
        Json(partial): Json<ocpi_types::serde_json::Value>,
    ) -> Response {
        match cfg.patch_json(&country_code, &party_id, &session_id, partial) {
            Ok(()) => Json(OcpiResponse::<Session>::success_empty()).into_response(),
            Err(ServerError::NotFound) => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<Session>::error(
                    OcpiStatusCode::UnknownLocation,
                    format!("session {session_id} not found"),
                )),
            )
                .into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OcpiResponse::<Session>::error(
                    OcpiStatusCode::ServerError,
                    "internal error",
                )),
            )
                .into_response(),
        }
    }

    // ── CDRs ──────────────────────────────────────────────────────────────────

    /// Build an axum router for the OCPI CDRs module.
    ///
    /// Exposes:
    /// - `GET  /cdrs` — paginated list (sender interface, CPO)
    /// - `GET  /cdrs/{cdr_id}` — single CDR (sender interface, CPO)
    /// - `POST /cdrs` — store a new CDR (receiver interface, eMSP); responds
    ///   `201 Created` with a `Location` header pointing to the stored CDR.
    ///
    /// OCPI routing headers (`OCPI-from/to-party-id/country-code`) are accepted
    /// on all routes; they are not enforced at this layer.
    pub fn cdrs_router(config: Arc<CdrsConfig>) -> Router {
        Router::new()
            .route("/cdrs", get(cdrs_list).post(cdrs_post))
            .route("/cdrs/{cdr_id}", get(cdrs_get))
            .with_state(config)
    }

    async fn cdrs_list(
        State(cfg): State<Arc<CdrsConfig>>,
        Query(params): Query<PaginatedParams>,
    ) -> Response {
        use ocpi_types::chrono::TimeZone as _;
        let date_from = params.date_from.unwrap_or_else(|| {
            ocpi_types::Utc
                .with_ymd_and_hms(1970, 1, 1, 0, 0, 0)
                .single()
                .expect("epoch is valid")
        });
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);

        let (items, total) = cfg.list(date_from, params.date_to, offset, limit);
        let page = OcpiPaged::new(items, offset, limit, total);
        let next_offset = page.next_offset();
        let body = page.into_response();

        let mut response = Json(body).into_response();
        let hdrs = response.headers_mut();
        if let Ok(v) = total.to_string().parse() {
            hdrs.insert("x-total-count", v);
        }
        if let Ok(v) = limit.to_string().parse() {
            hdrs.insert("x-limit", v);
        }
        if let Some(next_off) = next_offset {
            let link = format!("</cdrs?offset={next_off}&limit={limit}>; rel=\"next\"");
            if let Ok(v) = link.parse() {
                hdrs.insert("link", v);
            }
        }

        response
    }

    async fn cdrs_get(State(cfg): State<Arc<CdrsConfig>>, Path(cdr_id): Path<String>) -> Response {
        match cfg.get(&cdr_id) {
            Some(cdr) => Json(OcpiResponse::success(cdr)).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<Cdr>::error(
                    OcpiStatusCode::UnknownLocation,
                    format!("CDR {cdr_id} not found"),
                )),
            )
                .into_response(),
        }
    }

    async fn cdrs_post(State(cfg): State<Arc<CdrsConfig>>, Json(cdr): Json<Cdr>) -> Response {
        let location_url = cfg.store(cdr);
        let mut response = (
            StatusCode::CREATED,
            Json(OcpiResponse::<Cdr>::success_empty()),
        )
            .into_response();
        if let Ok(v) = location_url.parse() {
            response.headers_mut().insert("location", v);
        }
        response
    }

    // ── Tariffs ───────────────────────────────────────────────────────────────

    /// Build an axum router for the OCPI Tariffs module.
    ///
    /// Exposes:
    /// - `GET  /tariffs` — paginated list (sender interface, CPO)
    /// - `GET  /tariffs/{country_code}/{party_id}/{tariff_id}` — single tariff
    /// - `PUT  /tariffs/{country_code}/{party_id}/{tariff_id}` — upsert
    /// - `DELETE /tariffs/{country_code}/{party_id}/{tariff_id}` — remove
    ///
    /// OCPI routing headers (`OCPI-from/to-party-id/country-code`) are accepted
    /// on all routes; they are not enforced at this layer.
    pub fn tariffs_router(config: Arc<TariffsConfig>) -> Router {
        Router::new()
            .route("/tariffs", get(tariffs_list))
            .route(
                "/tariffs/{country_code}/{party_id}/{tariff_id}",
                get(tariffs_get).put(tariffs_put).delete(tariffs_delete),
            )
            .with_state(config)
    }

    async fn tariffs_list(
        State(cfg): State<Arc<TariffsConfig>>,
        Query(params): Query<PaginatedParams>,
    ) -> Response {
        use ocpi_types::chrono::TimeZone as _;
        let date_from = params.date_from.unwrap_or_else(|| {
            ocpi_types::Utc
                .with_ymd_and_hms(1970, 1, 1, 0, 0, 0)
                .single()
                .expect("epoch is valid")
        });
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);

        let (items, total) = cfg.list(date_from, params.date_to, offset, limit);
        let page = OcpiPaged::new(items, offset, limit, total);
        let next_offset = page.next_offset();
        let body = page.into_response();

        let mut response = Json(body).into_response();
        let hdrs = response.headers_mut();
        if let Ok(v) = total.to_string().parse() {
            hdrs.insert("x-total-count", v);
        }
        if let Ok(v) = limit.to_string().parse() {
            hdrs.insert("x-limit", v);
        }
        if let Some(next_off) = next_offset {
            let link = format!("</tariffs?offset={next_off}&limit={limit}>; rel=\"next\"");
            if let Ok(v) = link.parse() {
                hdrs.insert("link", v);
            }
        }

        response
    }

    async fn tariffs_get(
        State(cfg): State<Arc<TariffsConfig>>,
        Path((country_code, party_id, tariff_id)): Path<(String, String, String)>,
    ) -> Response {
        match cfg.get(&country_code, &party_id, &tariff_id) {
            Some(tariff) => Json(OcpiResponse::success(tariff)).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<Tariff>::error(
                    OcpiStatusCode::UnknownLocation,
                    format!("tariff {tariff_id} not found"),
                )),
            )
                .into_response(),
        }
    }

    async fn tariffs_put(
        State(cfg): State<Arc<TariffsConfig>>,
        Path((country_code, party_id, tariff_id)): Path<(String, String, String)>,
        Json(tariff): Json<Tariff>,
    ) -> Response {
        cfg.put(&country_code, &party_id, &tariff_id, tariff);
        Json(OcpiResponse::<Tariff>::success_empty()).into_response()
    }

    async fn tariffs_delete(
        State(cfg): State<Arc<TariffsConfig>>,
        Path((country_code, party_id, tariff_id)): Path<(String, String, String)>,
    ) -> Response {
        match cfg.delete(&country_code, &party_id, &tariff_id) {
            Ok(()) => Json(OcpiResponse::<Tariff>::success_empty()).into_response(),
            Err(ServerError::NotFound) => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<Tariff>::error(
                    OcpiStatusCode::UnknownLocation,
                    format!("tariff {tariff_id} not found"),
                )),
            )
                .into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OcpiResponse::<Tariff>::error(
                    OcpiStatusCode::ServerError,
                    "internal error",
                )),
            )
                .into_response(),
        }
    }

    // ── Tokens ────────────────────────────────────────────────────────────────

    /// Build an axum router for the OCPI Tokens module.
    ///
    /// Exposes:
    /// - `GET  /tokens` — paginated list (sender interface, eMSP)
    /// - `GET  /tokens/{country_code}/{party_id}/{token_uid}?type=` — single token
    /// - `PUT  /tokens/{country_code}/{party_id}/{token_uid}?type=` — upsert
    /// - `PATCH /tokens/{country_code}/{party_id}/{token_uid}?type=` — merge-patch
    /// - `POST /tokens/{token_uid}/authorize?type=` — real-time authorization
    ///
    /// OCPI routing headers (`OCPI-from/to-party-id/country-code`) are accepted
    /// on all routes; they are not enforced at this layer.
    pub fn tokens_router(config: Arc<TokensConfig>) -> Router {
        Router::new()
            .route("/tokens", get(tokens_list))
            .route(
                "/tokens/{country_code}/{party_id}/{token_uid}",
                get(tokens_get).put(tokens_put).patch(tokens_patch),
            )
            .route("/tokens/{token_uid}/authorize", post(tokens_authorize))
            .with_state(config)
    }

    #[derive(ocpi_types::serde::Deserialize)]
    #[serde(crate = "ocpi_types::serde")]
    struct TypeQuery {
        #[serde(rename = "type", default = "default_token_type")]
        token_type: TokenType,
    }

    fn default_token_type() -> TokenType {
        TokenType::Rfid
    }

    async fn tokens_list(
        State(cfg): State<Arc<TokensConfig>>,
        Query(params): Query<PaginatedParams>,
    ) -> Response {
        use ocpi_types::chrono::TimeZone as _;
        let date_from = params.date_from.unwrap_or_else(|| {
            ocpi_types::Utc
                .with_ymd_and_hms(1970, 1, 1, 0, 0, 0)
                .single()
                .expect("epoch is valid")
        });
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);

        let (items, total) = cfg.list(date_from, params.date_to, offset, limit);
        let page = OcpiPaged::new(items, offset, limit, total);
        let next_offset = page.next_offset();
        let body = page.into_response();

        let mut response = Json(body).into_response();
        let hdrs = response.headers_mut();
        if let Ok(v) = total.to_string().parse() {
            hdrs.insert("x-total-count", v);
        }
        if let Ok(v) = limit.to_string().parse() {
            hdrs.insert("x-limit", v);
        }
        if let Some(next_off) = next_offset {
            let link = format!("</tokens?offset={next_off}&limit={limit}>; rel=\"next\"");
            if let Ok(v) = link.parse() {
                hdrs.insert("link", v);
            }
        }

        response
    }

    async fn tokens_get(
        State(cfg): State<Arc<TokensConfig>>,
        Path((country_code, party_id, token_uid)): Path<(String, String, String)>,
        Query(q): Query<TypeQuery>,
    ) -> Response {
        match cfg.get(&country_code, &party_id, &token_uid, q.token_type) {
            Some(token) => Json(OcpiResponse::success(token)).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<Token>::error(
                    OcpiStatusCode::UnknownLocation,
                    format!(
                        "token {token_uid}?type={} not found",
                        token_type_str(q.token_type)
                    ),
                )),
            )
                .into_response(),
        }
    }

    async fn tokens_put(
        State(cfg): State<Arc<TokensConfig>>,
        Path((country_code, party_id, token_uid)): Path<(String, String, String)>,
        Query(q): Query<TypeQuery>,
        Json(token): Json<Token>,
    ) -> Response {
        cfg.put(&country_code, &party_id, &token_uid, q.token_type, token);
        Json(OcpiResponse::<Token>::success_empty()).into_response()
    }

    async fn tokens_patch(
        State(cfg): State<Arc<TokensConfig>>,
        Path((country_code, party_id, token_uid)): Path<(String, String, String)>,
        Query(q): Query<TypeQuery>,
        Json(partial): Json<ocpi_types::serde_json::Value>,
    ) -> Response {
        match cfg.patch_json(&country_code, &party_id, &token_uid, q.token_type, partial) {
            Ok(()) => Json(OcpiResponse::<Token>::success_empty()).into_response(),
            Err(ServerError::NotFound) => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<Token>::error(
                    OcpiStatusCode::UnknownLocation,
                    format!(
                        "token {token_uid}?type={} not found",
                        token_type_str(q.token_type)
                    ),
                )),
            )
                .into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OcpiResponse::<Token>::error(
                    OcpiStatusCode::ServerError,
                    "internal error",
                )),
            )
                .into_response(),
        }
    }

    async fn tokens_authorize(
        State(cfg): State<Arc<TokensConfig>>,
        Path(token_uid): Path<String>,
        Query(q): Query<TypeQuery>,
        body: Option<Json<LocationReferences>>,
    ) -> Response {
        let location = body.map(|Json(loc)| loc);
        match cfg.authorize(&token_uid, q.token_type, location) {
            Ok(auth_info) => Json(OcpiResponse::success(auth_info)).into_response(),
            Err(ServerError::UnknownToken) => (
                StatusCode::NOT_FOUND,
                Json(OcpiResponse::<AuthorizationInfo>::error(
                    OcpiStatusCode::UnknownToken,
                    format!("token {token_uid} not known"),
                )),
            )
                .into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(OcpiResponse::<AuthorizationInfo>::error(
                    OcpiStatusCode::ServerError,
                    "internal error",
                )),
            )
                .into_response(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ocpi_types::chrono::TimeZone as _;
    use ocpi_types::{
        v2_2_1::{
            AllowedType, AuthMethod, Cdr, CdrDimension, CdrDimensionType, CdrLocation, CdrToken,
            ChargingPeriod, ConnectorFormat, ConnectorType, PowerType, PriceComponent, Session,
            SessionStatus, Tariff, TariffDimensionType, TariffElement, Token, TokenType,
            WhitelistType,
        },
        OcpiStatusCode,
    };

    fn make_session(id: &str, ts: DateTime<Utc>) -> Session {
        use ocpi_types::common::{CiString2, CiString3, CiString36};
        Session {
            country_code: CiString2::try_from("NL").unwrap(),
            party_id: CiString3::try_from("CPO").unwrap(),
            id: CiString36::try_from(id).unwrap(),
            start_date_time: ts,
            end_date_time: None,
            kwh: 0.0,
            cdr_token: CdrToken {
                country_code: CiString2::try_from("NL").unwrap(),
                party_id: CiString3::try_from("MSP").unwrap(),
                uid: CiString36::try_from("RFID001").unwrap(),
                token_type: TokenType::Rfid,
                contract_id: CiString36::try_from("NL-MSP-0001").unwrap(),
            },
            auth_method: AuthMethod::Whitelist,
            authorization_reference: None,
            location_id: CiString36::try_from("LOC1").unwrap(),
            evse_uid: CiString36::try_from("EVSE1").unwrap(),
            connector_id: CiString36::try_from("1").unwrap(),
            meter_id: None,
            currency: "EUR".to_string(),
            charging_periods: vec![],
            total_cost: None,
            status: SessionStatus::Active,
            last_updated: ts,
        }
    }

    #[test]
    fn sessions_config_put_and_get_roundtrip() {
        let cfg = SessionsConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let s = make_session("S001", ts);
        cfg.put("NL", "CPO", "S001", s.clone());
        let got = cfg.get("NL", "CPO", "S001").unwrap();
        assert_eq!(got.id.as_str(), "S001");
        assert_eq!(got.kwh, 0.0);
    }

    #[test]
    fn sessions_config_get_missing_returns_none() {
        let cfg = SessionsConfig::new();
        assert!(cfg.get("NL", "CPO", "MISSING").is_none());
    }

    #[test]
    fn sessions_config_list_filters_by_date_from() {
        let cfg = SessionsConfig::new();
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put("NL", "CPO", "S001", make_session("S001", t1));
        cfg.put("NL", "CPO", "S002", make_session("S002", t2));

        let cutoff = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
        let (items, total) = cfg.list(cutoff, None, 0, 50);
        assert_eq!(total, 1);
        assert_eq!(items[0].id.as_str(), "S002");
    }

    #[test]
    fn sessions_config_list_filters_by_date_to() {
        let cfg = SessionsConfig::new();
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put("NL", "CPO", "S001", make_session("S001", t1));
        cfg.put("NL", "CPO", "S002", make_session("S002", t2));

        let from = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
        let (items, total) = cfg.list(from, Some(to), 0, 50);
        assert_eq!(total, 1);
        assert_eq!(items[0].id.as_str(), "S001");
    }

    #[test]
    fn sessions_config_list_pagination() {
        let cfg = SessionsConfig::new();
        let epoch = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
        for i in 0u32..5 {
            let ts = epoch + ocpi_types::chrono::Duration::seconds(i64::from(i));
            cfg.put(
                "NL",
                "CPO",
                &format!("S{i:03}"),
                make_session(&format!("S{i:03}"), ts),
            );
        }

        let (page, total) = cfg.list(epoch, None, 2, 2);
        assert_eq!(total, 5);
        assert_eq!(page.len(), 2);
        // sorted by last_updated, so offset=2 picks the 3rd & 4th
        assert_eq!(page[0].id.as_str(), "S002");
        assert_eq!(page[1].id.as_str(), "S003");
    }

    #[test]
    fn sessions_config_patch_updates_kwh() {
        let cfg = SessionsConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put("NL", "CPO", "S001", make_session("S001", ts));
        let patch = ocpi_types::serde_json::json!({"kwh": 12.5});
        cfg.patch_json("NL", "CPO", "S001", patch).unwrap();
        let updated = cfg.get("NL", "CPO", "S001").unwrap();
        assert_eq!(updated.kwh, 12.5);
    }

    #[test]
    fn sessions_config_patch_missing_returns_not_found() {
        let cfg = SessionsConfig::new();
        let patch = ocpi_types::serde_json::json!({"kwh": 5.0});
        let err = cfg.patch_json("NL", "CPO", "MISSING", patch).unwrap_err();
        assert!(matches!(err, ServerError::NotFound));
        assert_eq!(err.status_code(), OcpiStatusCode::UnknownLocation);
    }

    #[test]
    fn not_found_maps_to_unknown_location() {
        assert_eq!(
            ServerError::NotFound.status_code(),
            OcpiStatusCode::UnknownLocation
        );
    }

    #[test]
    fn unauthorized_maps_to_client_error() {
        assert_eq!(
            ServerError::Unauthorized.status_code(),
            OcpiStatusCode::ClientError
        );
    }

    #[test]
    fn not_implemented_maps_to_server_error() {
        assert_eq!(
            ServerError::NotImplemented("credentials").status_code(),
            OcpiStatusCode::ServerError
        );
    }

    #[test]
    fn already_registered_maps_to_client_error() {
        assert_eq!(
            ServerError::AlreadyRegistered.status_code(),
            OcpiStatusCode::ClientError
        );
    }

    #[test]
    fn not_registered_maps_to_client_error() {
        assert_eq!(
            ServerError::NotRegistered.status_code(),
            OcpiStatusCode::ClientError
        );
    }

    #[test]
    fn versions_config_default_is_empty() {
        let cfg = VersionsConfig::default();
        assert!(cfg.versions.is_empty());
        assert!(cfg.details.is_empty());
    }

    #[test]
    fn versions_config_add_and_lookup() {
        use ocpi_types::{
            version::{Endpoint, InterfaceRole, ModuleID, Version, VersionDetails, VersionNumber},
            Url,
        };

        let mut cfg = VersionsConfig::new();
        let details = VersionDetails {
            version: VersionNumber::V2_2_1,
            endpoints: vec![Endpoint {
                identifier: ModuleID::Credentials,
                role: InterfaceRole::Sender,
                url: Url::try_from("https://example.com/ocpi/2.2.1/credentials").unwrap(),
            }],
        };
        cfg.add_version(
            Version {
                version: VersionNumber::V2_2_1,
                url: Url::try_from("https://example.com/ocpi/2.2.1").unwrap(),
            },
            details.clone(),
        );
        assert_eq!(cfg.versions.len(), 1);
        assert_eq!(cfg.versions[0].version, VersionNumber::V2_2_1);
        assert_eq!(cfg.details.get(&VersionNumber::V2_2_1).unwrap(), &details);
    }

    #[test]
    fn versions_config_missing_version_is_unsupported() {
        // The ServerError returned for a missing version maps to UnsupportedVersion (3002).
        let cfg = VersionsConfig::new();
        assert!(!cfg.details.contains_key(&VersionNumber::V2_2_1));
        // Verify the error code that would be returned
        let err = ServerError::Ocpi(ocpi_types::OcpiError::Status(
            OcpiStatusCode::UnsupportedVersion,
        ));
        assert_eq!(err.status_code(), OcpiStatusCode::UnsupportedVersion);
    }

    // ── CdrsConfig tests ──────────────────────────────────────────────────────

    fn make_cdr(id: &str, ts: DateTime<Utc>) -> Cdr {
        use ocpi_types::common::{CiString2, CiString3, CiString36, CiString39, CiString48};
        Cdr {
            country_code: CiString2::try_from("NL").unwrap(),
            party_id: CiString3::try_from("CPO").unwrap(),
            id: CiString39::try_from(id).unwrap(),
            start_date_time: ts,
            end_date_time: ts,
            session_id: None,
            cdr_token: CdrToken {
                country_code: CiString2::try_from("NL").unwrap(),
                party_id: CiString3::try_from("MSP").unwrap(),
                uid: CiString36::try_from("RFID001").unwrap(),
                token_type: TokenType::Rfid,
                contract_id: CiString36::try_from("NL-MSP-0001").unwrap(),
            },
            auth_method: AuthMethod::Whitelist,
            authorization_reference: None,
            cdr_location: CdrLocation {
                id: CiString36::try_from("LOC1").unwrap(),
                name: None,
                address: "Test St 1".into(),
                city: "Amsterdam".into(),
                postal_code: None,
                state: None,
                country: "NLD".into(),
                coordinates: ocpi_types::common::GeoLocation {
                    latitude: "52.370216".into(),
                    longitude: "4.895168".into(),
                },
                evse_uid: CiString36::try_from("EVSE1").unwrap(),
                evse_id: CiString48::try_from("NL*CPO*E001").unwrap(),
                connector_id: CiString36::try_from("1").unwrap(),
                connector_standard: ConnectorType::Iec62196T2,
                connector_format: ConnectorFormat::Socket,
                connector_power_type: PowerType::Ac3Phase,
            },
            meter_id: None,
            currency: "EUR".into(),
            tariffs: vec![],
            charging_periods: vec![ChargingPeriod {
                start_date_time: ts,
                dimensions: vec![CdrDimension {
                    dimension_type: CdrDimensionType::Energy,
                    volume: 10.0,
                }],
                tariff_id: None,
            }],
            signed_data: None,
            total_cost: ocpi_types::common::Price {
                excl_vat: 2.50,
                incl_vat: None,
            },
            total_fixed_cost: None,
            total_energy: 10.0,
            total_energy_cost: None,
            total_time: 0.5,
            total_time_cost: None,
            total_parking_time: None,
            total_parking_cost: None,
            total_reservation_cost: None,
            remark: None,
            invoice_reference_id: None,
            credit: None,
            credit_reference_id: None,
            home_charging_compensation: None,
            last_updated: ts,
        }
    }

    #[test]
    fn cdrs_config_store_and_get_roundtrip() {
        let cfg = CdrsConfig::new("https://example.com/ocpi/2.2.1");
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let cdr = make_cdr("CDR001", ts);
        let url = cfg.store(cdr.clone());
        assert_eq!(url, "https://example.com/ocpi/2.2.1/cdrs/CDR001");
        let got = cfg.get("CDR001").unwrap();
        assert_eq!(got.id.as_str(), "CDR001");
    }

    #[test]
    fn cdrs_config_get_missing_returns_none() {
        let cfg = CdrsConfig::new("https://example.com/ocpi/2.2.1");
        assert!(cfg.get("MISSING").is_none());
    }

    #[test]
    fn cdrs_config_list_filters_by_date_from() {
        let cfg = CdrsConfig::new("https://example.com/ocpi/2.2.1");
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.store(make_cdr("CDR001", t1));
        cfg.store(make_cdr("CDR002", t2));

        let cutoff = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
        let (items, total) = cfg.list(cutoff, None, 0, 50);
        assert_eq!(total, 1);
        assert_eq!(items[0].id.as_str(), "CDR002");
    }

    #[test]
    fn cdrs_config_list_filters_by_date_to() {
        let cfg = CdrsConfig::new("https://example.com/ocpi/2.2.1");
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.store(make_cdr("CDR001", t1));
        cfg.store(make_cdr("CDR002", t2));

        let from = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
        let (items, total) = cfg.list(from, Some(to), 0, 50);
        assert_eq!(total, 1);
        assert_eq!(items[0].id.as_str(), "CDR001");
    }

    #[test]
    fn cdrs_config_list_pagination() {
        let cfg = CdrsConfig::new("https://example.com/ocpi/2.2.1");
        let epoch = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
        for i in 0u32..5 {
            let ts = epoch + ocpi_types::chrono::Duration::seconds(i64::from(i));
            cfg.store(make_cdr(&format!("CDR{i:03}"), ts));
        }

        let (page, total) = cfg.list(epoch, None, 2, 2);
        assert_eq!(total, 5);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id.as_str(), "CDR002");
        assert_eq!(page[1].id.as_str(), "CDR003");
    }

    #[test]
    fn cdrs_config_url_trailing_slash_normalised() {
        let cfg = CdrsConfig::new("https://example.com/ocpi/2.2.1/");
        let ts = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let url = cfg.store(make_cdr("CDR001", ts));
        assert_eq!(url, "https://example.com/ocpi/2.2.1/cdrs/CDR001");
    }

    // ── TariffsConfig tests ───────────────────────────────────────────────────

    fn make_tariff(id: &str, ts: DateTime<Utc>) -> Tariff {
        use ocpi_types::common::{CiString2, CiString3, CiString36};
        Tariff {
            country_code: CiString2::try_from("NL").unwrap(),
            party_id: CiString3::try_from("CPO").unwrap(),
            id: CiString36::try_from(id).unwrap(),
            currency: "EUR".to_string(),
            tariff_type: None,
            tariff_alt_text: vec![],
            tariff_alt_url: None,
            min_price: None,
            max_price: None,
            elements: vec![TariffElement {
                price_components: vec![PriceComponent {
                    component_type: TariffDimensionType::Energy,
                    price: 0.25,
                    vat: None,
                    step_size: 1,
                }],
                restrictions: None,
            }],
            start_date_time: None,
            end_date_time: None,
            energy_mix: None,
            last_updated: ts,
        }
    }

    #[test]
    fn tariffs_config_put_and_get_roundtrip() {
        let cfg = TariffsConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let t = make_tariff("TARIFF001", ts);
        cfg.put("NL", "CPO", "TARIFF001", t);
        let got = cfg.get("NL", "CPO", "TARIFF001").unwrap();
        assert_eq!(got.id.as_str(), "TARIFF001");
        assert_eq!(got.currency, "EUR");
    }

    #[test]
    fn tariffs_config_get_missing_returns_none() {
        let cfg = TariffsConfig::new();
        assert!(cfg.get("NL", "CPO", "MISSING").is_none());
    }

    #[test]
    fn tariffs_config_delete_removes_tariff() {
        let cfg = TariffsConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put("NL", "CPO", "T001", make_tariff("T001", ts));
        cfg.delete("NL", "CPO", "T001").unwrap();
        assert!(cfg.get("NL", "CPO", "T001").is_none());
    }

    #[test]
    fn tariffs_config_delete_unknown_returns_not_found() {
        let cfg = TariffsConfig::new();
        let err = cfg.delete("NL", "CPO", "MISSING").unwrap_err();
        assert!(matches!(err, ServerError::NotFound));
    }

    #[test]
    fn tariffs_config_list_filters_by_date_from() {
        let cfg = TariffsConfig::new();
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put("NL", "CPO", "T001", make_tariff("T001", t1));
        cfg.put("NL", "CPO", "T002", make_tariff("T002", t2));

        let cutoff = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
        let (items, total) = cfg.list(cutoff, None, 0, 50);
        assert_eq!(total, 1);
        assert_eq!(items[0].id.as_str(), "T002");
    }

    #[test]
    fn tariffs_config_list_filters_by_date_to() {
        let cfg = TariffsConfig::new();
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put("NL", "CPO", "T001", make_tariff("T001", t1));
        cfg.put("NL", "CPO", "T002", make_tariff("T002", t2));

        let from = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
        let (items, total) = cfg.list(from, Some(to), 0, 50);
        assert_eq!(total, 1);
        assert_eq!(items[0].id.as_str(), "T001");
    }

    #[test]
    fn tariffs_config_list_pagination() {
        let cfg = TariffsConfig::new();
        let epoch = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
        for i in 0u32..5 {
            let ts = epoch + ocpi_types::chrono::Duration::seconds(i64::from(i));
            cfg.put(
                "NL",
                "CPO",
                &format!("T{i:03}"),
                make_tariff(&format!("T{i:03}"), ts),
            );
        }

        let (page, total) = cfg.list(epoch, None, 2, 2);
        assert_eq!(total, 5);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id.as_str(), "T002");
        assert_eq!(page[1].id.as_str(), "T003");
    }

    // ── TokensConfig tests ────────────────────────────────────────────────────

    fn make_token(uid: &str, ts: DateTime<Utc>, valid: bool) -> Token {
        use ocpi_types::common::{CiString2, CiString3, CiString36};
        Token {
            country_code: CiString2::try_from("NL").unwrap(),
            party_id: CiString3::try_from("MSP").unwrap(),
            uid: CiString36::try_from(uid).unwrap(),
            token_type: TokenType::Rfid,
            contract_id: CiString36::try_from("NL-MSP-0001").unwrap(),
            visual_number: None,
            issuer: "TestIssuer".to_string(),
            group_id: None,
            valid,
            whitelist: WhitelistType::Always,
            language: None,
            default_profile_type: None,
            energy_contract: None,
            last_updated: ts,
        }
    }

    #[test]
    fn tokens_config_put_and_get_roundtrip() {
        let cfg = TokensConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        let token = make_token("TOKEN001", ts, true);
        cfg.put("NL", "MSP", "TOKEN001", TokenType::Rfid, token);
        let got = cfg.get("NL", "MSP", "TOKEN001", TokenType::Rfid).unwrap();
        assert_eq!(got.uid.as_str(), "TOKEN001");
        assert!(got.valid);
    }

    #[test]
    fn tokens_config_get_missing_returns_none() {
        let cfg = TokensConfig::new();
        assert!(cfg.get("NL", "MSP", "MISSING", TokenType::Rfid).is_none());
    }

    #[test]
    fn tokens_config_get_wrong_type_returns_none() {
        let cfg = TokensConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put(
            "NL",
            "MSP",
            "TOKEN001",
            TokenType::Rfid,
            make_token("TOKEN001", ts, true),
        );
        assert!(cfg
            .get("NL", "MSP", "TOKEN001", TokenType::AppUser)
            .is_none());
    }

    #[test]
    fn tokens_config_patch_updates_valid_field() {
        let cfg = TokensConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put(
            "NL",
            "MSP",
            "TOKEN001",
            TokenType::Rfid,
            make_token("TOKEN001", ts, true),
        );
        let patch =
            ocpi_types::serde_json::json!({"valid": false, "last_updated": "2024-06-02T00:00:00Z"});
        cfg.patch_json("NL", "MSP", "TOKEN001", TokenType::Rfid, patch)
            .unwrap();
        let updated = cfg.get("NL", "MSP", "TOKEN001", TokenType::Rfid).unwrap();
        assert!(!updated.valid);
    }

    #[test]
    fn tokens_config_patch_missing_returns_not_found() {
        let cfg = TokensConfig::new();
        let patch = ocpi_types::serde_json::json!({"valid": false});
        let err = cfg
            .patch_json("NL", "MSP", "MISSING", TokenType::Rfid, patch)
            .unwrap_err();
        assert!(matches!(err, ServerError::NotFound));
    }

    #[test]
    fn tokens_config_list_filters_by_date_from() {
        let cfg = TokensConfig::new();
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put(
            "NL",
            "MSP",
            "T001",
            TokenType::Rfid,
            make_token("T001", t1, true),
        );
        cfg.put(
            "NL",
            "MSP",
            "T002",
            TokenType::Rfid,
            make_token("T002", t2, true),
        );

        let cutoff = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
        let (items, total) = cfg.list(cutoff, None, 0, 50);
        assert_eq!(total, 1);
        assert_eq!(items[0].uid.as_str(), "T002");
    }

    #[test]
    fn tokens_config_list_pagination() {
        let cfg = TokensConfig::new();
        let epoch = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
        for i in 0u32..5 {
            let ts = epoch + ocpi_types::chrono::Duration::seconds(i64::from(i));
            let uid = format!("T{i:03}");
            cfg.put(
                "NL",
                "MSP",
                &uid,
                TokenType::Rfid,
                make_token(&uid, ts, true),
            );
        }

        let (page, total) = cfg.list(epoch, None, 2, 2);
        assert_eq!(total, 5);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].uid.as_str(), "T002");
        assert_eq!(page[1].uid.as_str(), "T003");
    }

    #[test]
    fn tokens_config_authorize_valid_token_returns_allowed() {
        let cfg = TokensConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put(
            "NL",
            "MSP",
            "RFID001",
            TokenType::Rfid,
            make_token("RFID001", ts, true),
        );
        let result = cfg.authorize("RFID001", TokenType::Rfid, None).unwrap();
        assert_eq!(result.allowed, AllowedType::Allowed);
        assert_eq!(result.token.uid.as_str(), "RFID001");
        assert!(result.location.is_none());
    }

    #[test]
    fn tokens_config_authorize_invalid_token_returns_blocked() {
        let cfg = TokensConfig::new();
        let ts = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
        cfg.put(
            "NL",
            "MSP",
            "RFID002",
            TokenType::Rfid,
            make_token("RFID002", ts, false),
        );
        let result = cfg.authorize("RFID002", TokenType::Rfid, None).unwrap();
        assert_eq!(result.allowed, AllowedType::Blocked);
    }

    #[test]
    fn tokens_config_authorize_unknown_token_returns_unknown_token_error() {
        let cfg = TokensConfig::new();
        let err = cfg.authorize("UNKNOWN", TokenType::Rfid, None).unwrap_err();
        assert!(matches!(err, ServerError::UnknownToken));
        assert_eq!(err.status_code(), OcpiStatusCode::UnknownToken);
    }
}
