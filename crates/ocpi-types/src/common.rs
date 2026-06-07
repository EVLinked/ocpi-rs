//! Common data types shared across OCPI modules.
//!
//! This is a foundational subset (milestone M1). Additional shared types are
//! added as their owning modules are implemented.

use serde::{Deserialize, Serialize};

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
