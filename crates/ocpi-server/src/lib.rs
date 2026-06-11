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
    v2_2_1::Credentials,
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

// ── CredentialsConfig ─────────────────────────────────────────────────────────

/// An in-memory credentials store for use with [`http::credentials_router`].
///
/// Holds the server's own [`Credentials`] and a token-keyed registry of
/// registered parties. Thread-safe via interior mutability (`RwLock`); wrap
/// in `Arc` to share across axum handlers.
///
/// `CredentialsConfig` intentionally does **not** implement
/// [`CredentialsHandler`] — wiring that trait generically through axum runs
/// into `async_fn_in_trait` / `Send` bound issues. Use this concrete type with
/// [`http::credentials_router`] instead, and keep the trait for custom
/// out-of-process implementations.
pub struct CredentialsConfig {
    /// The credentials this server returns on every successful request.
    pub own_credentials: Credentials,
    registered: std::sync::RwLock<std::collections::HashMap<String, Credentials>>,
}

impl std::fmt::Debug for CredentialsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialsConfig")
            .field("own_credentials", &self.own_credentials)
            .field(
                "registered_count",
                &self.registered.read().map(|m| m.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl CredentialsConfig {
    /// Create a new registry with the given server credentials.
    ///
    /// No parties are registered initially. Call
    /// [`register`](Self::register) or let parties register via the axum
    /// router.
    #[must_use]
    pub fn new(own_credentials: Credentials) -> Self {
        Self {
            own_credentials,
            registered: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Returns `true` if `token` belongs to a registered party.
    #[must_use]
    pub fn is_registered(&self, token: &str) -> bool {
        self.registered
            .read()
            .expect("lock not poisoned")
            .contains_key(token)
    }

    /// Register a new party under `token`, storing their [`Credentials`].
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::AlreadyRegistered`] if `token` is already known.
    pub fn register(&self, token: &str, credentials: Credentials) -> Result<(), ServerError> {
        let mut map = self.registered.write().expect("lock not poisoned");
        if map.contains_key(token) {
            return Err(ServerError::AlreadyRegistered);
        }
        map.insert(token.to_owned(), credentials);
        Ok(())
    }

    /// Update the stored credentials for an already-registered party.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotRegistered`] if `token` is not in the
    /// registry.
    pub fn update(&self, token: &str, credentials: Credentials) -> Result<(), ServerError> {
        let mut map = self.registered.write().expect("lock not poisoned");
        if !map.contains_key(token) {
            return Err(ServerError::NotRegistered);
        }
        map.insert(token.to_owned(), credentials);
        Ok(())
    }

    /// Remove the registration for `token`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::NotRegistered`] if `token` is not in the
    /// registry.
    pub fn delete(&self, token: &str) -> Result<(), ServerError> {
        let mut map = self.registered.write().expect("lock not poisoned");
        if !map.contains_key(token) {
            return Err(ServerError::NotRegistered);
        }
        map.remove(token);
        Ok(())
    }
}

// ── axum integration ──────────────────────────────────────────────────────────

#[cfg(feature = "axum")]
pub mod http {
    //! axum integration: ready-made routers for OCPI receiver endpoints.

    use std::sync::Arc;

    use axum::{
        extract::{Path, State},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Response},
        routing::get,
        Json, Router,
    };
    use ocpi_types::{
        envelope::OcpiResponse,
        transport::CredentialToken,
        v2_2_1::Credentials,
        version::{VersionDetails, VersionNumber},
        OcpiStatusCode,
    };

    use crate::{CredentialsConfig, ServerError, VersionsConfig};

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

    // ── Credentials ───────────────────────────────────────────────────────────

    /// Build an axum router for the OCPI credentials endpoints.
    ///
    /// Exposes:
    /// - `GET    /credentials` — return this server's own credentials
    /// - `POST   /credentials` — register a new party; HTTP 405 if already registered
    /// - `PUT    /credentials` — update an existing registration; HTTP 405 if not registered
    /// - `DELETE /credentials` — revoke a registration; HTTP 405 if not registered
    ///
    /// All routes validate the `Authorization: Token <base64>` header.
    /// Pass an `Arc<`[`CredentialsConfig`]`>` so the same store can be shared
    /// with other handlers or inspected by the host application.
    pub fn credentials_router(config: Arc<CredentialsConfig>) -> Router {
        Router::new()
            .route(
                "/credentials",
                get(handle_get)
                    .post(handle_post)
                    .put(handle_put)
                    .delete(handle_delete),
            )
            .with_state(config)
    }

    /// Extract and decode the Bearer token from `Authorization: Token <base64>`.
    fn extract_token(headers: &HeaderMap) -> Option<String> {
        let value = headers.get("Authorization")?.to_str().ok()?;
        CredentialToken::from_header_value(value).map(|t| t.as_str().to_owned())
    }

    fn unauthorized_response() -> Response {
        (
            StatusCode::UNAUTHORIZED,
            Json(OcpiResponse::<Credentials>::error(
                OcpiStatusCode::ClientError,
                "unauthorized",
            )),
        )
            .into_response()
    }

    fn method_not_allowed_response(msg: &'static str) -> Response {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            Json(OcpiResponse::<Credentials>::error(
                OcpiStatusCode::ClientError,
                msg,
            )),
        )
            .into_response()
    }

    fn server_error_response() -> Response {
        Json(OcpiResponse::<Credentials>::error(
            OcpiStatusCode::ServerError,
            "internal server error",
        ))
        .into_response()
    }

    async fn handle_get(State(cfg): State<Arc<CredentialsConfig>>, headers: HeaderMap) -> Response {
        let token = match extract_token(&headers) {
            Some(t) => t,
            None => return unauthorized_response(),
        };
        if !cfg.is_registered(token.as_str()) {
            return unauthorized_response();
        }
        Json(OcpiResponse::success(cfg.own_credentials.clone())).into_response()
    }

    async fn handle_post(
        State(cfg): State<Arc<CredentialsConfig>>,
        headers: HeaderMap,
        Json(body): Json<Credentials>,
    ) -> Response {
        let token = match extract_token(&headers) {
            Some(t) => t,
            None => return unauthorized_response(),
        };
        match cfg.register(token.as_str(), body) {
            Ok(()) => Json(OcpiResponse::success(cfg.own_credentials.clone())).into_response(),
            Err(ServerError::AlreadyRegistered) => {
                method_not_allowed_response("already registered")
            }
            Err(_) => server_error_response(),
        }
    }

    async fn handle_put(
        State(cfg): State<Arc<CredentialsConfig>>,
        headers: HeaderMap,
        Json(body): Json<Credentials>,
    ) -> Response {
        let token = match extract_token(&headers) {
            Some(t) => t,
            None => return unauthorized_response(),
        };
        match cfg.update(token.as_str(), body) {
            Ok(()) => Json(OcpiResponse::success(cfg.own_credentials.clone())).into_response(),
            Err(ServerError::NotRegistered) => method_not_allowed_response("not registered"),
            Err(_) => server_error_response(),
        }
    }

    async fn handle_delete(
        State(cfg): State<Arc<CredentialsConfig>>,
        headers: HeaderMap,
    ) -> Response {
        let token = match extract_token(&headers) {
            Some(t) => t,
            None => return unauthorized_response(),
        };
        match cfg.delete(token.as_str()) {
            Ok(()) => Json(OcpiResponse::<Credentials>::success_empty()).into_response(),
            Err(ServerError::NotRegistered) => method_not_allowed_response("not registered"),
            Err(_) => server_error_response(),
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

    // ── CredentialsConfig ─────────────────────────────────────────────────────

    fn make_credentials(token: &str) -> Credentials {
        use ocpi_types::{
            common::{BusinessDetails, CiString2, CiString3},
            v2_2_1::CredentialsRole,
            Role, Url,
        };
        Credentials {
            token: token.to_owned(),
            url: Url::try_from("https://example.com/ocpi/versions").unwrap(),
            roles: vec![CredentialsRole {
                role: Role::Cpo,
                business_details: BusinessDetails {
                    name: "Test CPO".into(),
                    website: None,
                    logo: None,
                },
                party_id: CiString3::try_from("EXA").unwrap(),
                country_code: CiString2::try_from("NL").unwrap(),
            }],
        }
    }

    #[test]
    fn credentials_config_new_is_empty() {
        let cfg = CredentialsConfig::new(make_credentials("SERVER_TOKEN"));
        assert!(!cfg.is_registered("TOKEN_A"));
    }

    #[test]
    fn credentials_config_register_and_lookup() {
        let cfg = CredentialsConfig::new(make_credentials("SERVER_TOKEN"));
        let party = make_credentials("PARTY_TOKEN");
        cfg.register("TOKEN_A", party.clone()).unwrap();
        assert!(cfg.is_registered("TOKEN_A"));
        assert!(!cfg.is_registered("TOKEN_B"));
    }

    #[test]
    fn credentials_config_double_register_is_error() {
        let cfg = CredentialsConfig::new(make_credentials("SERVER_TOKEN"));
        cfg.register("TOKEN_A", make_credentials("P")).unwrap();
        let err = cfg.register("TOKEN_A", make_credentials("P2")).unwrap_err();
        assert!(matches!(err, ServerError::AlreadyRegistered));
    }

    #[test]
    fn credentials_config_update_unknown_is_error() {
        let cfg = CredentialsConfig::new(make_credentials("SERVER_TOKEN"));
        let err = cfg.update("UNKNOWN", make_credentials("P")).unwrap_err();
        assert!(matches!(err, ServerError::NotRegistered));
    }

    #[test]
    fn credentials_config_update_known_succeeds() {
        let cfg = CredentialsConfig::new(make_credentials("SERVER_TOKEN"));
        cfg.register("TOKEN_A", make_credentials("P1")).unwrap();
        cfg.update("TOKEN_A", make_credentials("P2")).unwrap();
        assert!(cfg.is_registered("TOKEN_A"));
    }

    #[test]
    fn credentials_config_delete_unknown_is_error() {
        let cfg = CredentialsConfig::new(make_credentials("SERVER_TOKEN"));
        let err = cfg.delete("UNKNOWN").unwrap_err();
        assert!(matches!(err, ServerError::NotRegistered));
    }

    #[test]
    fn credentials_config_delete_known_removes() {
        let cfg = CredentialsConfig::new(make_credentials("SERVER_TOKEN"));
        cfg.register("TOKEN_A", make_credentials("P")).unwrap();
        cfg.delete("TOKEN_A").unwrap();
        assert!(!cfg.is_registered("TOKEN_A"));
    }

    // ── axum credentials router ───────────────────────────────────────────────

    #[cfg(feature = "axum")]
    mod axum_credentials_tests {
        use super::*;
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use ocpi_types::transport::CredentialToken;
        use tower::ServiceExt as _;

        fn server_config() -> std::sync::Arc<CredentialsConfig> {
            std::sync::Arc::new(CredentialsConfig::new(make_credentials("SERVER_TOKEN")))
        }

        fn auth_header(raw_token: &str) -> String {
            CredentialToken::new(raw_token).to_header_value()
        }

        fn party_json(token: &str) -> String {
            serde_json::to_string(&make_credentials(token)).unwrap()
        }

        #[tokio::test]
        async fn get_missing_token_is_401() {
            let app = crate::http::credentials_router(server_config());
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/credentials")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn get_unregistered_token_is_401() {
            let app = crate::http::credentials_router(server_config());
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/credentials")
                        .header("Authorization", auth_header("UNKNOWN"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn post_registers_and_returns_200() {
            let cfg = server_config();
            let app = crate::http::credentials_router(cfg.clone());
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/credentials")
                        .header("Authorization", auth_header("TOKEN_A"))
                        .header("Content-Type", "application/json")
                        .body(Body::from(party_json("PARTY_TOKEN")))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            assert!(cfg.is_registered("TOKEN_A"));
        }

        #[tokio::test]
        async fn post_double_register_is_405() {
            let cfg = server_config();
            cfg.register("TOKEN_A", make_credentials("P1")).unwrap();
            let app = crate::http::credentials_router(cfg);
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/credentials")
                        .header("Authorization", auth_header("TOKEN_A"))
                        .header("Content-Type", "application/json")
                        .body(Body::from(party_json("P2")))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
        }

        #[tokio::test]
        async fn put_not_registered_is_405() {
            let app = crate::http::credentials_router(server_config());
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("PUT")
                        .uri("/credentials")
                        .header("Authorization", auth_header("TOKEN_A"))
                        .header("Content-Type", "application/json")
                        .body(Body::from(party_json("P")))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
        }

        #[tokio::test]
        async fn delete_not_registered_is_405() {
            let app = crate::http::credentials_router(server_config());
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri("/credentials")
                        .header("Authorization", auth_header("TOKEN_A"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
        }

        #[tokio::test]
        async fn delete_registered_returns_200_empty() {
            let cfg = server_config();
            cfg.register("TOKEN_A", make_credentials("P")).unwrap();
            let app = crate::http::credentials_router(cfg.clone());
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri("/credentials")
                        .header("Authorization", auth_header("TOKEN_A"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            assert!(!cfg.is_registered("TOKEN_A"));
        }

        #[tokio::test]
        async fn get_registered_returns_own_credentials() {
            let cfg = server_config();
            cfg.register("TOKEN_A", make_credentials("P")).unwrap();
            let app = crate::http::credentials_router(cfg.clone());
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/credentials")
                        .header("Authorization", auth_header("TOKEN_A"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let envelope: ocpi_types::OcpiResponse<ocpi_types::v2_2_1::Credentials> =
                serde_json::from_slice(&body).unwrap();
            assert!(envelope.is_success());
            assert_eq!(envelope.data.unwrap().token, cfg.own_credentials.token);
        }
    }
}
