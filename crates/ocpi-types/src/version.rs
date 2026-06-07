//! OCPI version negotiation primitives.
//!
//! Two parties discover each other's supported versions via the `/versions`
//! endpoint, then agree on a shared one. This module models the version
//! identifiers and the `/versions` list entry.

use serde::{Deserialize, Serialize};

/// An OCPI protocol version, serialized as its canonical dotted string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersionNumber {
    /// OCPI 2.0.
    #[serde(rename = "2.0")]
    V2_0,
    /// OCPI 2.1.1.
    #[serde(rename = "2.1.1")]
    V2_1_1,
    /// OCPI 2.2.
    #[serde(rename = "2.2")]
    V2_2,
    /// OCPI 2.2.1.
    #[serde(rename = "2.2.1")]
    V2_2_1,
    /// OCPI 2.3.0.
    #[serde(rename = "2.3.0")]
    V2_3_0,
}

impl VersionNumber {
    /// The canonical dotted version string (e.g. `"2.2.1"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V2_0 => "2.0",
            Self::V2_1_1 => "2.1.1",
            Self::V2_2 => "2.2",
            Self::V2_2_1 => "2.2.1",
            Self::V2_3_0 => "2.3.0",
        }
    }
}

/// An entry in the `/versions` list: a supported version and where to fetch
/// its details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    /// The supported OCPI version.
    pub version: VersionNumber,
    /// URL of the endpoint listing this version's module details.
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::{Version, VersionNumber};

    #[test]
    fn version_number_serializes_as_dotted_string() {
        let json = serde_json::to_string(&VersionNumber::V2_2_1).expect("serialize");
        assert_eq!(json, "\"2.2.1\"");
    }

    #[test]
    fn version_entry_round_trips() {
        let v = Version {
            version: VersionNumber::V2_3_0,
            url: "https://example.com/2.3.0".into(),
        };
        let json = serde_json::to_string(&v).expect("serialize");
        let back: Version = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, v);
        assert_eq!(back.version.as_str(), "2.3.0");
    }
}
