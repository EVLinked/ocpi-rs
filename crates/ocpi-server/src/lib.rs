//! # ocpi-server
//!
//! Server-side building blocks for the **receiver** role — the side that
//! exposes OCPI endpoints and is called by remote parties.
//!
//! The core is framework-agnostic: you implement handler traits such as
//! [`CredentialsHandler`]. Enable the `axum` feature for ready-made routers
//! (see the `http` module).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use ocpi_types::OcpiStatusCode;

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

#[cfg(feature = "axum")]
pub mod http {
    //! axum integration: ready-made routers for OCPI receiver endpoints.
    //!
    //! Routes are added incrementally per the roadmap; today this exposes a
    //! stub `/versions` route to anchor the wiring.

    use axum::{routing::get, Router};

    /// Build an axum router exposing the OCPI receiver endpoints.
    pub fn router() -> Router {
        Router::new().route("/versions", get(versions_stub))
    }

    async fn versions_stub() -> &'static str {
        "{\"data\":[],\"status_code\":1000}"
    }
}

#[cfg(test)]
mod tests {
    use super::ServerError;
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
}
