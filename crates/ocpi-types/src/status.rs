//! Canonical OCPI status codes.
//!
//! Every OCPI HTTP response carries an integer `status_code` in its body
//! (independent of the HTTP status). `1000` means success; `2xxx` are client
//! errors, `3xxx` server errors, and `4xxx` hub errors. See the OCPI
//! `status_codes` specification chapter.

/// A status code returned in the body of every OCPI response.
///
/// Use [`OcpiStatusCode::code`] for the wire integer and
/// [`OcpiStatusCode::from_code`] to parse one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OcpiStatusCode {
    /// `1000` — generic success.
    Success,
    /// `2000` — generic client error.
    ClientError,
    /// `2001` — invalid or missing parameters.
    InvalidParameters,
    /// `2002` — not enough information provided.
    NotEnoughInformation,
    /// `2003` — unknown location / EVSE / connector.
    UnknownLocation,
    /// `2004` — unknown token.
    UnknownToken,
    /// `3000` — generic server error.
    ServerError,
    /// `3001` — unable to use the client's API.
    UnableToUseClientApi,
    /// `3002` — unsupported version.
    UnsupportedVersion,
    /// `3003` — no matching endpoints or expected endpoints missing.
    MissingEndpoints,
    /// `4000` — generic hub error.
    HubError,
    /// `4001` — unknown receiver (the `to` address is unknown).
    UnknownReceiver,
    /// `4002` — timeout on a forwarded request.
    HubTimeout,
    /// `4003` — connection problem inside the hub.
    HubConnectionProblem,
}

impl OcpiStatusCode {
    /// The integer code as sent on the wire.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Success => 1000,
            Self::ClientError => 2000,
            Self::InvalidParameters => 2001,
            Self::NotEnoughInformation => 2002,
            Self::UnknownLocation => 2003,
            Self::UnknownToken => 2004,
            Self::ServerError => 3000,
            Self::UnableToUseClientApi => 3001,
            Self::UnsupportedVersion => 3002,
            Self::MissingEndpoints => 3003,
            Self::HubError => 4000,
            Self::UnknownReceiver => 4001,
            Self::HubTimeout => 4002,
            Self::HubConnectionProblem => 4003,
        }
    }

    /// Parse a wire integer into a known status code, if recognised.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        let value = match code {
            1000 => Self::Success,
            2000 => Self::ClientError,
            2001 => Self::InvalidParameters,
            2002 => Self::NotEnoughInformation,
            2003 => Self::UnknownLocation,
            2004 => Self::UnknownToken,
            3000 => Self::ServerError,
            3001 => Self::UnableToUseClientApi,
            3002 => Self::UnsupportedVersion,
            3003 => Self::MissingEndpoints,
            4000 => Self::HubError,
            4001 => Self::UnknownReceiver,
            4002 => Self::HubTimeout,
            4003 => Self::HubConnectionProblem,
            _ => return None,
        };
        Some(value)
    }

    /// `true` if this is the success code (`1000`).
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }

    /// A short human-readable description of the code.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::ClientError => "Client error",
            Self::InvalidParameters => "Invalid or missing parameters",
            Self::NotEnoughInformation => "Not enough information",
            Self::UnknownLocation => "Unknown location",
            Self::UnknownToken => "Unknown token",
            Self::ServerError => "Server error",
            Self::UnableToUseClientApi => "Unable to use the client's API",
            Self::UnsupportedVersion => "Unsupported version",
            Self::MissingEndpoints => "Missing expected endpoints",
            Self::HubError => "Hub error",
            Self::UnknownReceiver => "Unknown receiver",
            Self::HubTimeout => "Timeout on forwarded request",
            Self::HubConnectionProblem => "Hub connection problem",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OcpiStatusCode;

    #[test]
    fn codes_round_trip() {
        for code in [
            1000u16, 2000, 2001, 2002, 2003, 2004, 3000, 3001, 3002, 3003, 4000, 4001, 4002, 4003,
        ] {
            let parsed = OcpiStatusCode::from_code(code).expect("known code");
            assert_eq!(parsed.code(), code);
        }
    }

    #[test]
    fn unknown_code_is_none() {
        assert_eq!(OcpiStatusCode::from_code(9999), None);
    }

    #[test]
    fn only_1000_is_success() {
        assert!(OcpiStatusCode::Success.is_success());
        assert!(!OcpiStatusCode::ServerError.is_success());
    }
}
