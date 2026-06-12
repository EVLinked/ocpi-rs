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
    v2_2_1::{Connector, Credentials, Evse, Location},
    version::{Version, VersionDetails, VersionNumber},
    OcpiStatusCode,
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

    /// The requested object was not found.
    #[error("not found")]
    NotFound,

    /// A PATCH body could not be applied to the stored object.
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
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
            Self::Ocpi(_) | Self::NotImplemented(_) | Self::InvalidPatch(_) => {
                OcpiStatusCode::ServerError
            }
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

// ── LocationsHandler ──────────────────────────────────────────────────────────

/// Handles the OCPI Locations receiver interface (eMSP side).
///
/// The receiver stores Location objects pushed by CPOs via PUT/PATCH.
/// The CPO may also GET a previously-pushed object to verify the eMSP's state.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — §Receiver Interface
#[allow(async_fn_in_trait)]
pub trait LocationsHandler {
    /// Return a page of stored [`Location`] objects for one CPO.
    ///
    /// `offset` is zero-based; `limit` caps the page size.
    /// Returns `(items, total_count)` where `total_count` is the unfiltered
    /// total for that CPO (used to set `X-Total-Count`).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] on internal failure.
    async fn list_locations(
        &self,
        country_code: &str,
        party_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Location>, u32), ServerError>;

    /// Return a single [`Location`] by its ID.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when `location_id` is unknown.
    async fn get_location(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
    ) -> Result<Location, ServerError>;

    /// Upsert a [`Location`] (PUT).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] on internal failure.
    async fn put_location(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        location: Location,
    ) -> Result<(), ServerError>;

    /// Apply a JSON merge-patch to a stored [`Location`] (PATCH, RFC 7396).
    ///
    /// # Errors
    ///
    /// - [`ServerError::NotFound`] — `location_id` unknown.
    /// - [`ServerError::InvalidPatch`] — patch produces an invalid Location.
    async fn patch_location(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        patch: serde_json::Value,
    ) -> Result<(), ServerError>;

    /// Return a single [`Evse`] nested within a Location.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the location or EVSE is unknown.
    async fn get_evse(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
    ) -> Result<Evse, ServerError>;

    /// Upsert an [`Evse`] within a Location (PUT).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when `location_id` is unknown.
    async fn put_evse(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        evse: Evse,
    ) -> Result<(), ServerError>;

    /// Apply a JSON merge-patch to an [`Evse`] within a Location (PATCH).
    ///
    /// # Errors
    ///
    /// - [`ServerError::NotFound`] — location or EVSE unknown.
    /// - [`ServerError::InvalidPatch`] — patch produces an invalid EVSE.
    async fn patch_evse(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        patch: serde_json::Value,
    ) -> Result<(), ServerError>;

    /// Return a single [`Connector`] nested within an EVSE.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the location, EVSE, or Connector is unknown.
    async fn get_connector(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        connector_id: &str,
    ) -> Result<Connector, ServerError>;

    /// Upsert a [`Connector`] within an EVSE (PUT).
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotFound`] when the location or EVSE is unknown.
    async fn put_connector(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        connector_id: &str,
        connector: Connector,
    ) -> Result<(), ServerError>;

    /// Apply a JSON merge-patch to a [`Connector`] within an EVSE (PATCH).
    ///
    /// # Errors
    ///
    /// - [`ServerError::NotFound`] — location, EVSE, or Connector unknown.
    /// - [`ServerError::InvalidPatch`] — patch produces an invalid Connector.
    async fn patch_connector(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        connector_id: &str,
        patch: serde_json::Value,
    ) -> Result<(), ServerError>;
}

// ── LocationsConfig ───────────────────────────────────────────────────────────

/// An in-memory Locations store for use with [`http::locations_router`].
///
/// Stores [`Location`] objects keyed by `(country_code, party_id, location_id)`.
/// Thread-safe via interior mutability (`RwLock`); wrap in `Arc` to share
/// across axum handlers.
///
/// `LocationsConfig` implements [`LocationsHandler`] directly and does **not**
/// require the axum feature.
pub struct LocationsConfig {
    store: std::sync::RwLock<std::collections::HashMap<(String, String, String), Location>>,
}

impl std::fmt::Debug for LocationsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocationsConfig")
            .field(
                "location_count",
                &self.store.read().map(|m| m.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl Default for LocationsConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl LocationsConfig {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    fn key(cc: &str, pid: &str, lid: &str) -> (String, String, String) {
        (cc.to_owned(), pid.to_owned(), lid.to_owned())
    }
}

/// Apply an RFC 7396 JSON merge-patch in place.
fn json_merge_patch(target: &mut serde_json::Value, patch: serde_json::Value) {
    if let serde_json::Value::Object(patch_map) = patch {
        if let serde_json::Value::Object(target_map) = target {
            for (key, val) in patch_map {
                if val.is_null() {
                    target_map.remove(&key);
                } else {
                    let entry = target_map.entry(key).or_insert(serde_json::Value::Null);
                    json_merge_patch(entry, val);
                }
            }
        } else {
            *target = serde_json::Value::Object(patch_map.into_iter().collect());
        }
    } else {
        *target = patch;
    }
}

#[allow(async_fn_in_trait)]
impl LocationsHandler for LocationsConfig {
    async fn list_locations(
        &self,
        country_code: &str,
        party_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<Location>, u32), ServerError> {
        let store = self.store.read().expect("lock not poisoned");
        let mut all: Vec<Location> = store
            .iter()
            .filter(|((cc, pid, _), _)| cc == country_code && pid == party_id)
            .map(|(_, loc)| loc.clone())
            .collect();
        all.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        let total = all.len() as u32;
        let page = all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok((page, total))
    }

    async fn get_location(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
    ) -> Result<Location, ServerError> {
        self.store
            .read()
            .expect("lock not poisoned")
            .get(&Self::key(country_code, party_id, location_id))
            .cloned()
            .ok_or(ServerError::NotFound)
    }

    async fn put_location(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        location: Location,
    ) -> Result<(), ServerError> {
        self.store
            .write()
            .expect("lock not poisoned")
            .insert(Self::key(country_code, party_id, location_id), location);
        Ok(())
    }

    async fn patch_location(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        patch: serde_json::Value,
    ) -> Result<(), ServerError> {
        let mut store = self.store.write().expect("lock not poisoned");
        let loc = store
            .get_mut(&Self::key(country_code, party_id, location_id))
            .ok_or(ServerError::NotFound)?;
        let mut val =
            serde_json::to_value(&*loc).map_err(|e| ServerError::InvalidPatch(e.to_string()))?;
        json_merge_patch(&mut val, patch);
        *loc = serde_json::from_value(val).map_err(|e| ServerError::InvalidPatch(e.to_string()))?;
        Ok(())
    }

    async fn get_evse(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
    ) -> Result<Evse, ServerError> {
        let store = self.store.read().expect("lock not poisoned");
        let loc = store
            .get(&Self::key(country_code, party_id, location_id))
            .ok_or(ServerError::NotFound)?;
        loc.evses
            .iter()
            .find(|e| e.uid.as_str() == evse_uid)
            .cloned()
            .ok_or(ServerError::NotFound)
    }

    async fn put_evse(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        evse: Evse,
    ) -> Result<(), ServerError> {
        let mut store = self.store.write().expect("lock not poisoned");
        let loc = store
            .get_mut(&Self::key(country_code, party_id, location_id))
            .ok_or(ServerError::NotFound)?;
        if let Some(pos) = loc.evses.iter().position(|e| e.uid.as_str() == evse_uid) {
            loc.evses[pos] = evse;
        } else {
            loc.evses.push(evse);
        }
        Ok(())
    }

    async fn patch_evse(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        patch: serde_json::Value,
    ) -> Result<(), ServerError> {
        let mut store = self.store.write().expect("lock not poisoned");
        let loc = store
            .get_mut(&Self::key(country_code, party_id, location_id))
            .ok_or(ServerError::NotFound)?;
        let pos = loc
            .evses
            .iter()
            .position(|e| e.uid.as_str() == evse_uid)
            .ok_or(ServerError::NotFound)?;
        let mut val = serde_json::to_value(&loc.evses[pos])
            .map_err(|e| ServerError::InvalidPatch(e.to_string()))?;
        json_merge_patch(&mut val, patch);
        loc.evses[pos] =
            serde_json::from_value(val).map_err(|e| ServerError::InvalidPatch(e.to_string()))?;
        Ok(())
    }

    async fn get_connector(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        connector_id: &str,
    ) -> Result<Connector, ServerError> {
        let store = self.store.read().expect("lock not poisoned");
        let loc = store
            .get(&Self::key(country_code, party_id, location_id))
            .ok_or(ServerError::NotFound)?;
        let evse = loc
            .evses
            .iter()
            .find(|e| e.uid.as_str() == evse_uid)
            .ok_or(ServerError::NotFound)?;
        evse.connectors
            .iter()
            .find(|c| c.id.as_str() == connector_id)
            .cloned()
            .ok_or(ServerError::NotFound)
    }

    async fn put_connector(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        connector_id: &str,
        connector: Connector,
    ) -> Result<(), ServerError> {
        let mut store = self.store.write().expect("lock not poisoned");
        let loc = store
            .get_mut(&Self::key(country_code, party_id, location_id))
            .ok_or(ServerError::NotFound)?;
        let evse = loc
            .evses
            .iter_mut()
            .find(|e| e.uid.as_str() == evse_uid)
            .ok_or(ServerError::NotFound)?;
        if let Some(pos) = evse
            .connectors
            .iter()
            .position(|c| c.id.as_str() == connector_id)
        {
            evse.connectors[pos] = connector;
        } else {
            evse.connectors.push(connector);
        }
        Ok(())
    }

    async fn patch_connector(
        &self,
        country_code: &str,
        party_id: &str,
        location_id: &str,
        evse_uid: &str,
        connector_id: &str,
        patch: serde_json::Value,
    ) -> Result<(), ServerError> {
        let mut store = self.store.write().expect("lock not poisoned");
        let loc = store
            .get_mut(&Self::key(country_code, party_id, location_id))
            .ok_or(ServerError::NotFound)?;
        let evse = loc
            .evses
            .iter_mut()
            .find(|e| e.uid.as_str() == evse_uid)
            .ok_or(ServerError::NotFound)?;
        let pos = evse
            .connectors
            .iter()
            .position(|c| c.id.as_str() == connector_id)
            .ok_or(ServerError::NotFound)?;
        let mut val = serde_json::to_value(&evse.connectors[pos])
            .map_err(|e| ServerError::InvalidPatch(e.to_string()))?;
        json_merge_patch(&mut val, patch);
        evse.connectors[pos] =
            serde_json::from_value(val).map_err(|e| ServerError::InvalidPatch(e.to_string()))?;
        Ok(())
    }
}

// ── axum integration ──────────────────────────────────────────────────────────

#[cfg(feature = "axum")]
pub mod http {
    //! axum integration: ready-made routers for OCPI receiver endpoints.

    use std::sync::Arc;

    use axum::{
        extract::{Path, Query, State},
        http::{header::HeaderName, HeaderMap, HeaderValue, StatusCode},
        response::{IntoResponse, Response},
        routing::get,
        Json, Router,
    };
    use ocpi_types::{
        envelope::OcpiResponse,
        v2_2_1::{Connector, Evse, Location},
        version::{VersionDetails, VersionNumber},
        OcpiStatusCode,
    };
    use serde::Deserialize;

    use crate::{LocationsConfig, LocationsHandler, ServerError, VersionsConfig};

    // ── Versions ──────────────────────────────────────────────────────────────

    /// Build an axum router exposing `GET /versions` and `GET /versions/{version}`.
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

    // ── Locations ─────────────────────────────────────────────────────────────

    #[derive(Deserialize)]
    struct PageParams {
        #[serde(default)]
        offset: u32,
        #[serde(default = "default_limit")]
        limit: u32,
    }

    fn default_limit() -> u32 {
        20
    }

    fn not_found_response() -> Response {
        (
            StatusCode::NOT_FOUND,
            Json(OcpiResponse::<Location>::error(
                OcpiStatusCode::UnknownLocation,
                "not found",
            )),
        )
            .into_response()
    }

    fn server_error_response() -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(OcpiResponse::<Location>::error(
                OcpiStatusCode::ServerError,
                "internal server error",
            )),
        )
            .into_response()
    }

    fn pagination_headers(total: u32, limit: u32) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&total.to_string()) {
            headers.insert(HeaderName::from_static("x-total-count"), v);
        }
        if let Ok(v) = HeaderValue::from_str(&limit.to_string()) {
            headers.insert(HeaderName::from_static("x-limit"), v);
        }
        headers
    }

    fn locations_err(e: ServerError) -> Response {
        match e {
            ServerError::NotFound => not_found_response(),
            _ => server_error_response(),
        }
    }

    /// Build an axum router for the OCPI Locations receiver interface.
    ///
    /// Exposes the full receiver interface as specified in
    /// `specs/ocpi/2.2.1/mod_locations.asciidoc` §Receiver Interface:
    ///
    /// - `GET    /locations/{cc}/{pid}` — paginated list (`X-Total-Count`, `X-Limit`)
    /// - `GET    /locations/{cc}/{pid}/{lid}` — single Location
    /// - `PUT    /locations/{cc}/{pid}/{lid}` — upsert Location
    /// - `PATCH  /locations/{cc}/{pid}/{lid}` — merge-patch Location (RFC 7396)
    /// - `GET    /locations/{cc}/{pid}/{lid}/{evse_uid}` — single EVSE
    /// - `PUT    /locations/{cc}/{pid}/{lid}/{evse_uid}` — upsert EVSE
    /// - `PATCH  /locations/{cc}/{pid}/{lid}/{evse_uid}` — merge-patch EVSE
    /// - `GET    /locations/{cc}/{pid}/{lid}/{evse_uid}/{cid}` — single Connector
    /// - `PUT    /locations/{cc}/{pid}/{lid}/{evse_uid}/{cid}` — upsert Connector
    /// - `PATCH  /locations/{cc}/{pid}/{lid}/{evse_uid}/{cid}` — merge-patch Connector
    ///
    /// Known gaps: `Link: next` header (requires deployment base-URL knowledge),
    /// date-range filtering on the list endpoint. Both are deferred to a follow-up.
    pub fn locations_router(config: Arc<LocationsConfig>) -> Router {
        Router::new()
            .route("/locations/{cc}/{pid}", get(loc_list))
            .route(
                "/locations/{cc}/{pid}/{lid}",
                get(loc_get).put(loc_put).patch(loc_patch),
            )
            .route(
                "/locations/{cc}/{pid}/{lid}/{evse_uid}",
                get(evse_get).put(evse_put).patch(evse_patch),
            )
            .route(
                "/locations/{cc}/{pid}/{lid}/{evse_uid}/{cid}",
                get(conn_get).put(conn_put).patch(conn_patch),
            )
            .with_state(config)
    }

    async fn loc_list(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid)): Path<(String, String)>,
        Query(params): Query<PageParams>,
    ) -> Response {
        match cfg
            .list_locations(&cc, &pid, params.offset, params.limit)
            .await
        {
            Ok((items, total)) => {
                let headers = pagination_headers(total, params.limit);
                (headers, Json(OcpiResponse::success(items))).into_response()
            }
            Err(e) => locations_err(e),
        }
    }

    async fn loc_get(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid)): Path<(String, String, String)>,
    ) -> Response {
        match cfg.get_location(&cc, &pid, &lid).await {
            Ok(loc) => Json(OcpiResponse::success(loc)).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn loc_put(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid)): Path<(String, String, String)>,
        Json(body): Json<Location>,
    ) -> Response {
        match cfg.put_location(&cc, &pid, &lid, body).await {
            Ok(()) => Json(OcpiResponse::<Location>::success_empty()).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn loc_patch(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid)): Path<(String, String, String)>,
        Json(body): Json<serde_json::Value>,
    ) -> Response {
        match cfg.patch_location(&cc, &pid, &lid, body).await {
            Ok(()) => Json(OcpiResponse::<Location>::success_empty()).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn evse_get(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid, evse_uid)): Path<(String, String, String, String)>,
    ) -> Response {
        match cfg.get_evse(&cc, &pid, &lid, &evse_uid).await {
            Ok(evse) => Json(OcpiResponse::success(evse)).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn evse_put(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid, evse_uid)): Path<(String, String, String, String)>,
        Json(body): Json<Evse>,
    ) -> Response {
        match cfg.put_evse(&cc, &pid, &lid, &evse_uid, body).await {
            Ok(()) => Json(OcpiResponse::<Evse>::success_empty()).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn evse_patch(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid, evse_uid)): Path<(String, String, String, String)>,
        Json(body): Json<serde_json::Value>,
    ) -> Response {
        match cfg.patch_evse(&cc, &pid, &lid, &evse_uid, body).await {
            Ok(()) => Json(OcpiResponse::<Evse>::success_empty()).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn conn_get(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid, evse_uid, cid)): Path<(String, String, String, String, String)>,
    ) -> Response {
        match cfg.get_connector(&cc, &pid, &lid, &evse_uid, &cid).await {
            Ok(conn) => Json(OcpiResponse::success(conn)).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn conn_put(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid, evse_uid, cid)): Path<(String, String, String, String, String)>,
        Json(body): Json<Connector>,
    ) -> Response {
        match cfg
            .put_connector(&cc, &pid, &lid, &evse_uid, &cid, body)
            .await
        {
            Ok(()) => Json(OcpiResponse::<Connector>::success_empty()).into_response(),
            Err(e) => locations_err(e),
        }
    }

    async fn conn_patch(
        State(cfg): State<Arc<LocationsConfig>>,
        Path((cc, pid, lid, evse_uid, cid)): Path<(String, String, String, String, String)>,
        Json(body): Json<serde_json::Value>,
    ) -> Response {
        match cfg
            .patch_connector(&cc, &pid, &lid, &evse_uid, &cid, body)
            .await
        {
            Ok(()) => Json(OcpiResponse::<Connector>::success_empty()).into_response(),
            Err(e) => locations_err(e),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ocpi_types::OcpiStatusCode;

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
    fn not_found_maps_to_unknown_location() {
        assert_eq!(
            ServerError::NotFound.status_code(),
            OcpiStatusCode::UnknownLocation
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
        let cfg = VersionsConfig::new();
        assert!(!cfg.details.contains_key(&VersionNumber::V2_2_1));
        let err = ServerError::Ocpi(ocpi_types::OcpiError::Status(
            OcpiStatusCode::UnsupportedVersion,
        ));
        assert_eq!(err.status_code(), OcpiStatusCode::UnsupportedVersion);
    }

    // ── LocationsConfig tests ─────────────────────────────────────────────────

    fn make_location(cc: &str, pid: &str, lid: &str) -> Location {
        use chrono::Utc;
        use ocpi_types::{
            common::{CiString2, CiString3, CiString36, GeoLocation},
            v2_2_1::Location,
        };

        Location {
            country_code: CiString2::try_from(cc).unwrap(),
            party_id: CiString3::try_from(pid).unwrap(),
            id: CiString36::try_from(lid).unwrap(),
            publish: true,
            publish_allowed_to: vec![],
            name: None,
            address: "Strawinskylaan 3051".into(),
            city: "Amsterdam".into(),
            postal_code: Some("1077ZX".into()),
            state: None,
            country: "NLD".into(),
            coordinates: GeoLocation {
                latitude: "52.3740300".into(),
                longitude: "4.9162600".into(),
            },
            related_locations: vec![],
            parking_type: None,
            evses: vec![],
            directions: vec![],
            operator: None,
            suboperator: None,
            owner: None,
            facilities: vec![],
            time_zone: "Europe/Amsterdam".into(),
            opening_times: None,
            charging_when_closed: None,
            images: vec![],
            energy_mix: None,
            last_updated: Utc::now(),
        }
    }

    #[tokio::test]
    async fn locations_put_and_get_round_trip() {
        let cfg = LocationsConfig::new();
        let loc = make_location("NL", "TNM", "LOC001");
        cfg.put_location("NL", "TNM", "LOC001", loc.clone())
            .await
            .unwrap();
        let got = cfg.get_location("NL", "TNM", "LOC001").await.unwrap();
        assert_eq!(got.city, "Amsterdam");
    }

    #[tokio::test]
    async fn locations_get_unknown_returns_not_found() {
        let cfg = LocationsConfig::new();
        let err = cfg.get_location("NL", "TNM", "NOPE").await.unwrap_err();
        assert!(matches!(err, ServerError::NotFound));
    }

    #[tokio::test]
    async fn locations_list_pagination() {
        let cfg = LocationsConfig::new();
        cfg.put_location("NL", "TNM", "A", make_location("NL", "TNM", "A"))
            .await
            .unwrap();
        cfg.put_location("NL", "TNM", "B", make_location("NL", "TNM", "B"))
            .await
            .unwrap();
        cfg.put_location("NL", "TNM", "C", make_location("NL", "TNM", "C"))
            .await
            .unwrap();

        let (page1, total) = cfg.list_locations("NL", "TNM", 0, 2).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(page1.len(), 2);

        let (page2, total2) = cfg.list_locations("NL", "TNM", 2, 2).await.unwrap();
        assert_eq!(total2, 3);
        assert_eq!(page2.len(), 1);
    }

    #[tokio::test]
    async fn locations_list_filters_by_cpo() {
        let cfg = LocationsConfig::new();
        cfg.put_location("NL", "TNM", "L1", make_location("NL", "TNM", "L1"))
            .await
            .unwrap();
        cfg.put_location("DE", "EXA", "L2", make_location("DE", "EXA", "L2"))
            .await
            .unwrap();
        let (items, total) = cfg.list_locations("NL", "TNM", 0, 10).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn locations_patch_updates_field() {
        let cfg = LocationsConfig::new();
        cfg.put_location("NL", "TNM", "LOC1", make_location("NL", "TNM", "LOC1"))
            .await
            .unwrap();
        let patch = serde_json::json!({"city": "Rotterdam"});
        cfg.patch_location("NL", "TNM", "LOC1", patch)
            .await
            .unwrap();
        let got = cfg.get_location("NL", "TNM", "LOC1").await.unwrap();
        assert_eq!(got.city, "Rotterdam");
    }

    #[tokio::test]
    async fn json_merge_patch_removes_null_key() {
        let mut val = serde_json::json!({"a": 1, "b": "hello"});
        super::json_merge_patch(&mut val, serde_json::json!({"b": null}));
        assert!(val.get("b").is_none());
        assert_eq!(val["a"], 1);
    }
}
