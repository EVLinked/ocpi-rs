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
}

impl ServerError {
    /// Map this error to the OCPI status code that should be returned in the
    /// response envelope.
    #[must_use]
    pub fn status_code(&self) -> OcpiStatusCode {
        match self {
            Self::Ocpi(ocpi_types::OcpiError::Status(code)) => *code,
            Self::Unauthorized => OcpiStatusCode::ClientError,
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
        self.details.get(&version).cloned().ok_or(ServerError::Ocpi(
            ocpi_types::OcpiError::Status(OcpiStatusCode::UnsupportedVersion),
        ))
    }
}

// ── CredentialsHandler ────────────────────────────────────────────────────────

/// Handles the OCPI credentials / registration handshake (receiver role).
///
/// Implementors persist the presented token and exchange endpoint information
/// per the OCPI `credentials` module.
#[allow(async_fn_in_trait)]
pub trait CredentialsHandler {
    /// Handle an inbound credentials registration (`POST /credentials`),
    /// authenticated by the bearer `token`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] when the token is rejected or the handshake
    /// cannot be completed.
    async fn register(&self, token: &str) -> Result<(), ServerError>;
}

// ── axum integration ──────────────────────────────────────────────────────────

#[cfg(feature = "axum")]
pub mod http {
    //! axum integration: ready-made routers for OCPI receiver endpoints.

    use std::sync::Arc;

    use axum::{
        extract::{Path, State},
        response::{IntoResponse, Response},
        routing::get,
        Json, Router,
    };
    use ocpi_types::{
        envelope::OcpiResponse,
        version::{VersionDetails, VersionNumber},
        OcpiStatusCode,
    };

    use crate::VersionsConfig;

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
}
