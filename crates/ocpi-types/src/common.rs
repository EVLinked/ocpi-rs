//! Common data types shared across OCPI modules.
//!
//! This is a foundational subset (milestone M1). Additional shared types are
//! added as their owning modules are implemented.

use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::OcpiError;

// ── Role ──────────────────────────────────────────────────────────────────────

/// Party / platform role in the OCPI ecosystem.
///
/// Spec: `specs/ocpi/2.2.1/types.asciidoc` — Role enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    /// Charge Point Operator.
    #[serde(rename = "CPO")]
    Cpo,
    /// eMobility Service Provider.
    #[serde(rename = "EMSP")]
    Emsp,
    /// Hub.
    Hub,
    /// National Access Point.
    #[serde(rename = "NAP")]
    Nap,
    /// Navigation Service Provider.
    #[serde(rename = "NSP")]
    Nsp,
    /// Other role.
    Other,
    /// Smart Charging Service Provider.
    #[serde(rename = "SCSP")]
    Scsp,
}

// ── CiString<N> ───────────────────────────────────────────────────────────────

/// A validated case-insensitive ASCII string of at most `MAX` bytes.
///
/// OCPI spec: "Only printable ASCII allowed" (U+0020–U+007E). The `MAX`
/// const parameter enforces the per-field length constraint at construction
/// time. Case-insensitivity is a *comparison* property — the stored value
/// preserves the original casing; callers normalize when comparing.
///
/// Spec: `specs/ocpi/2.2.1/types.asciidoc` — CiString type.
#[derive(Debug, Clone, Eq)]
pub struct CiString<const MAX: usize>(String);

impl<const MAX: usize> CiString<MAX> {
    /// The stored string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<const MAX: usize> PartialEq for CiString<MAX> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl<const MAX: usize> Hash for CiString<MAX> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_ascii_lowercase().hash(state);
    }
}

impl<const MAX: usize> TryFrom<&str> for CiString<MAX> {
    type Error = OcpiError;

    fn try_from(s: &str) -> Result<Self, OcpiError> {
        if s.len() > MAX {
            return Err(OcpiError::Invalid(format!(
                "CiString<{MAX}>: value too long ({} bytes)",
                s.len()
            )));
        }
        if !s.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
            return Err(OcpiError::Invalid(
                "CiString: only printable ASCII (U+0020–U+007E) allowed".into(),
            ));
        }
        Ok(Self(s.to_owned()))
    }
}

impl<const MAX: usize> TryFrom<String> for CiString<MAX> {
    type Error = OcpiError;

    fn try_from(s: String) -> Result<Self, OcpiError> {
        if s.len() > MAX {
            return Err(OcpiError::Invalid(format!(
                "CiString<{MAX}>: value too long ({} bytes)",
                s.len()
            )));
        }
        if !s.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
            return Err(OcpiError::Invalid(
                "CiString: only printable ASCII (U+0020–U+007E) allowed".into(),
            ));
        }
        Ok(Self(s))
    }
}

impl<const MAX: usize> fmt::Display for CiString<MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<const MAX: usize> Serialize for CiString<MAX> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de, const MAX: usize> Deserialize<'de> for CiString<MAX> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

/// `CiString(2)` — ISO 3166-1 alpha-2 country code and similar 2-char fields.
pub type CiString2 = CiString<2>;
/// `CiString(3)` — eMI3 party identifier and similar 3-char fields.
pub type CiString3 = CiString<3>;
/// `CiString(36)` — UUIDs and similar medium-length identifiers.
pub type CiString36 = CiString<36>;
/// `CiString(255)` — general-purpose, matches the URL field limit.
pub type CiString255 = CiString<255>;

// ── Url ───────────────────────────────────────────────────────────────────────

/// An OCPI URL: a `string(255)` following the W3C URI spec.
///
/// Validates only the length constraint (≤ 255 bytes). Full URI syntax
/// validation is out of scope at this stage.
///
/// Spec: `specs/ocpi/2.2.1/types.asciidoc` — URL type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Url(String);

impl Url {
    /// The raw URL string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for Url {
    type Error = OcpiError;

    fn try_from(s: &str) -> Result<Self, OcpiError> {
        if s.len() > 255 {
            return Err(OcpiError::Invalid(format!(
                "URL exceeds 255 bytes ({} bytes)",
                s.len()
            )));
        }
        Ok(Self(s.to_owned()))
    }
}

impl TryFrom<String> for Url {
    type Error = OcpiError;

    fn try_from(s: String) -> Result<Self, OcpiError> {
        if s.len() > 255 {
            return Err(OcpiError::Invalid(format!(
                "URL exceeds 255 bytes ({} bytes)",
                s.len()
            )));
        }
        Ok(Self(s))
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Url {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Url {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

// ── Pre-existing types ────────────────────────────────────────────────────────

/// Human-readable text in a specific language (OCPI `DisplayText`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayText {
    /// ISO 639-1 language code (e.g. `en`, `vi`).
    pub language: String,
    /// The text to be displayed (max 512 characters per the spec).
    pub text: String,
}

/// A geographic location in decimal degrees (OCPI `GeoLocation`).
///
/// Latitude and longitude are strings per the specification, formatted with a
/// fixed decimal notation (e.g. `"51.047599"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoLocation {
    /// Latitude in decimal degrees, as a string.
    pub latitude: String,
    /// Longitude in decimal degrees, as a string.
    pub longitude: String,
}

/// Logo / image reference (OCPI `Image`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// URL from which the image can be fetched.
    pub url: String,
    /// Optional URL of a thumbnail-sized version.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thumbnail: Option<String>,
    /// Image category (e.g. `OPERATOR`, `LOCATION`).
    pub category: String,
    /// Image file type (e.g. `png`, `jpeg`).
    #[serde(rename = "type")]
    pub image_type: String,
    /// Optional width in pixels.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub width: Option<u32>,
    /// Optional height in pixels.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub height: Option<u32>,
}

/// Details of a business / party (OCPI `BusinessDetails`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusinessDetails {
    /// Name of the operator.
    pub name: String,
    /// Optional link to the operator's website.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub website: Option<String>,
    /// Optional logo of the operator.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub logo: Option<Image>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Role ──

    #[test]
    fn role_serializes_to_uppercase() {
        assert_eq!(serde_json::to_string(&Role::Cpo).unwrap(), "\"CPO\"");
        assert_eq!(serde_json::to_string(&Role::Emsp).unwrap(), "\"EMSP\"");
        assert_eq!(serde_json::to_string(&Role::Hub).unwrap(), "\"HUB\"");
        assert_eq!(serde_json::to_string(&Role::Nap).unwrap(), "\"NAP\"");
        assert_eq!(serde_json::to_string(&Role::Nsp).unwrap(), "\"NSP\"");
        assert_eq!(serde_json::to_string(&Role::Other).unwrap(), "\"OTHER\"");
        assert_eq!(serde_json::to_string(&Role::Scsp).unwrap(), "\"SCSP\"");
    }

    #[test]
    fn role_roundtrip() {
        for role in [
            Role::Cpo,
            Role::Emsp,
            Role::Hub,
            Role::Nap,
            Role::Nsp,
            Role::Other,
            Role::Scsp,
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let back: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(back, role);
        }
    }

    // ── CiString ──

    #[test]
    fn ci_string_accepts_valid_ascii() {
        let s = CiString3::try_from("NL").unwrap();
        assert_eq!(s.as_str(), "NL");
    }

    #[test]
    fn ci_string_accepts_max_length() {
        let s = CiString3::try_from("TNM").unwrap();
        assert_eq!(s.as_str(), "TNM");
    }

    #[test]
    fn ci_string_rejects_too_long() {
        let err = CiString3::try_from("TOOLONG").unwrap_err();
        assert!(matches!(err, OcpiError::Invalid(_)));
    }

    #[test]
    fn ci_string_rejects_non_printable_ascii() {
        let err = CiString3::try_from("A\nB").unwrap_err();
        assert!(matches!(err, OcpiError::Invalid(_)));
    }

    #[test]
    fn ci_string_rejects_non_ascii() {
        let err = CiString3::try_from("Ñ").unwrap_err();
        assert!(matches!(err, OcpiError::Invalid(_)));
    }

    #[test]
    fn ci_string_equality_is_case_insensitive() {
        let a = CiString3::try_from("TNM").unwrap();
        let b = CiString3::try_from("tnm").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn ci_string_serde_roundtrip() {
        let s = CiString3::try_from("CPO").unwrap();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"CPO\"");
        let back: CiString3 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn ci_string_serde_rejects_too_long() {
        let err = serde_json::from_str::<CiString2>("\"TOOLONG\"");
        assert!(err.is_err());
    }

    // ── Url ──

    #[test]
    fn url_accepts_valid() {
        let u = Url::try_from("https://example.com/ocpi/cpo/2.2.1/").unwrap();
        assert_eq!(u.as_str(), "https://example.com/ocpi/cpo/2.2.1/");
    }

    #[test]
    fn url_accepts_255_chars() {
        let s = "https://".to_owned() + &"a".repeat(247);
        assert_eq!(s.len(), 255);
        assert!(Url::try_from(s).is_ok());
    }

    #[test]
    fn url_rejects_256_chars() {
        let s = "https://".to_owned() + &"a".repeat(248);
        assert_eq!(s.len(), 256);
        let err = Url::try_from(s).unwrap_err();
        assert!(matches!(err, OcpiError::Invalid(_)));
    }

    #[test]
    fn url_serde_roundtrip() {
        let u = Url::try_from("https://example.com/").unwrap();
        let json = serde_json::to_string(&u).unwrap();
        assert_eq!(json, "\"https://example.com/\"");
        let back: Url = serde_json::from_str(&json).unwrap();
        assert_eq!(back, u);
    }

    #[test]
    fn url_serde_rejects_too_long() {
        let long = format!("\"https://{}\"", "a".repeat(250));
        assert!(serde_json::from_str::<Url>(&long).is_err());
    }
}
