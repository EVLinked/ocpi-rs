//! Errors produced by the OCPI client.

/// Something went wrong while talking to a remote OCPI party.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The underlying HTTP request failed.
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    /// A URL could not be constructed.
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    /// The response envelope reported success but carried no `data`.
    #[error("response envelope contained no data")]
    EmptyData,

    /// The response carried a non-success OCPI status code.
    #[error(transparent)]
    Ocpi(#[from] ocpi_types::OcpiError),

    /// The requested operation is not yet implemented.
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),
}
