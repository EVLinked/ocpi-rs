//! OCPI **2.2.1** typed models (the primary production target).
//!
//! Modules: Versions, Credentials, Locations, Sessions, CDRs, Tariffs, Tokens,
//! Commands, ChargingProfiles, HubClientInfo.
//!
//! Populated incrementally — see milestones **M2–M6** in the roadmap.

use serde::{Deserialize, Serialize};

use crate::common::{BusinessDetails, CiString2, CiString3, Role, Url};
use crate::OcpiError;

// ── CredentialsRole ───────────────────────────────────────────────────────────

/// A single role entry in a [`Credentials`] object.
///
/// Every role needs a unique combination of `role`, `party_id`, and
/// `country_code`. A platform that provides white-label CPO services may
/// carry multiple `CredentialsRole` entries — the schema is forward-compatible
/// with that use-case even though the current server implementation only
/// handles single-role registrations.
///
/// Spec: `specs/ocpi/2.2.1/credentials.asciidoc` — CredentialsRole class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialsRole {
    /// The role this party fulfils in the OCPI ecosystem.
    pub role: Role,
    /// Business details about this party.
    pub business_details: BusinessDetails,
    /// eMI3 party identifier (3-char, e.g. `"EXA"`).
    pub party_id: CiString3,
    /// ISO 3166-1 alpha-2 country code (e.g. `"NL"`).
    pub country_code: CiString2,
}

// ── Credentials ───────────────────────────────────────────────────────────────

/// The credentials object exchanged during OCPI registration (POST) and
/// updates (PUT), and returned on GET.
///
/// `roles` must be non-empty; multi-role is schema-legal per the spec.
/// Server implementations that have not yet added multi-role support
/// should call [`Credentials::check_single_role`] and return
/// [`OcpiStatusCode::ServerError`](crate::OcpiStatusCode::ServerError) on
/// failure.
///
/// Spec: `specs/ocpi/2.2.1/credentials.asciidoc` — Credentials object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    /// Bearer token the remote party must use in subsequent requests.
    ///
    /// OCPI 2.2.1 spec: printable non-whitespace ASCII (U+0021–U+007E),
    /// max 64 characters. Not validated here — callers are responsible.
    pub token: String,
    /// URL of this party's `/versions` endpoint.
    pub url: Url,
    /// Roles this party provides. Non-empty; one entry is the common case.
    pub roles: Vec<CredentialsRole>,
}

impl Credentials {
    /// Returns `Err` when `roles` is empty (spec requires at least one).
    ///
    /// # Errors
    ///
    /// Returns [`OcpiError::Invalid`] if `roles` is empty.
    pub fn validate(&self) -> Result<(), OcpiError> {
        if self.roles.is_empty() {
            return Err(OcpiError::Invalid(
                "credentials.roles must contain at least one entry".into(),
            ));
        }
        Ok(())
    }

    /// Returns `Err` when `roles` has more than one entry.
    ///
    /// Call this in server implementations that have not yet added multi-role
    /// support; return [`OcpiStatusCode::ServerError`](crate::OcpiStatusCode::ServerError)
    /// to the remote party so it knows the limitation is server-side.
    ///
    /// # Errors
    ///
    /// Returns [`OcpiError::Invalid`] if `roles.len() > 1`.
    pub fn check_single_role(&self) -> Result<(), OcpiError> {
        if self.roles.len() > 1 {
            return Err(OcpiError::Invalid(
                "multi-role credentials are not yet supported by this server".into(),
            ));
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::BusinessDetails;

    fn make_role(role: Role, party: &str, country: &str) -> CredentialsRole {
        CredentialsRole {
            role,
            business_details: BusinessDetails {
                name: "Test Party".into(),
                website: None,
                logo: None,
            },
            party_id: CiString3::try_from(party).unwrap(),
            country_code: CiString2::try_from(country).unwrap(),
        }
    }

    fn cpo_credentials() -> Credentials {
        Credentials {
            token: "TOKEN_B".into(),
            url: Url::try_from("https://example.com/ocpi/versions").unwrap(),
            roles: vec![make_role(Role::Cpo, "EXA", "NL")],
        }
    }

    // ── Serde round-trips ─────────────────────────────────────────────────────

    #[test]
    fn credentials_serde_roundtrip() {
        let c = cpo_credentials();
        let json = serde_json::to_string(&c).unwrap();
        let back: Credentials = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn credentials_role_serde_roundtrip() {
        let r = make_role(Role::Emsp, "MSP", "DE");
        let json = serde_json::to_string(&r).unwrap();
        let back: CredentialsRole = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    /// Spec example: minimal CPO credentials object.
    #[test]
    fn credentials_spec_example_minimal_cpo() {
        let json = r#"{
            "token": "ZXhhbXBsZS10b2tlbgo=",
            "url": "https://example.com/ocpi/versions",
            "roles": [
                {
                    "role": "CPO",
                    "business_details": {"name": "Example CPO"},
                    "party_id": "EXA",
                    "country_code": "NL"
                }
            ]
        }"#;
        let c: Credentials = serde_json::from_str(json).unwrap();
        assert_eq!(c.token, "ZXhhbXBsZS10b2tlbgo=");
        assert_eq!(c.roles.len(), 1);
        assert_eq!(c.roles[0].role, Role::Cpo);
        assert_eq!(c.roles[0].party_id.as_str(), "EXA");
        assert_eq!(c.roles[0].country_code.as_str(), "NL");
        assert_eq!(c.roles[0].business_details.name, "Example CPO");
    }

    /// Spec example: combined CPO + eMSP credentials (multi-role, schema-legal).
    #[test]
    fn credentials_spec_example_multi_role() {
        let json = r#"{
            "token": "TOKEN_C",
            "url": "https://example.com/ocpi/versions",
            "roles": [
                {
                    "role": "CPO",
                    "business_details": {"name": "Example Operator"},
                    "party_id": "EXA",
                    "country_code": "NL"
                },
                {
                    "role": "EMSP",
                    "business_details": {"name": "Example Provider"},
                    "party_id": "EXB",
                    "country_code": "NL"
                }
            ]
        }"#;
        let c: Credentials = serde_json::from_str(json).unwrap();
        assert_eq!(c.roles.len(), 2);
        assert_eq!(c.roles[0].role, Role::Cpo);
        assert_eq!(c.roles[1].role, Role::Emsp);
    }

    // ── Validation ────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_empty_roles() {
        let c = Credentials {
            token: "T".into(),
            url: Url::try_from("https://example.com/ocpi/versions").unwrap(),
            roles: vec![],
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_accepts_single_role() {
        assert!(cpo_credentials().validate().is_ok());
    }

    #[test]
    fn check_single_role_rejects_multi_role() {
        let json = r#"{
            "token": "T",
            "url": "https://example.com/ocpi/versions",
            "roles": [
                {"role":"CPO","business_details":{"name":"A"},"party_id":"AAA","country_code":"NL"},
                {"role":"EMSP","business_details":{"name":"B"},"party_id":"BBB","country_code":"NL"}
            ]
        }"#;
        let c: Credentials = serde_json::from_str(json).unwrap();
        assert!(c.check_single_role().is_err());
    }

    #[test]
    fn check_single_role_accepts_single() {
        assert!(cpo_credentials().check_single_role().is_ok());
    }
}
