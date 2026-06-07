//! Error type shared across the OCPI crates.

use crate::status::OcpiStatusCode;

/// An error constructing or interpreting OCPI messages.
#[derive(Debug, thiserror::Error)]
pub enum OcpiError {
    /// A response carried a non-success [`OcpiStatusCode`].
    #[error("OCPI status code {} ({})", .0.code(), .0.message())]
    Status(OcpiStatusCode),

    /// A value did not conform to the OCPI specification.
    #[error("invalid OCPI value: {0}")]
    Invalid(String),

    /// A feature is part of the protocol but not yet implemented in this crate.
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),
}

impl From<OcpiStatusCode> for OcpiError {
    fn from(code: OcpiStatusCode) -> Self {
        Self::Status(code)
    }
}
