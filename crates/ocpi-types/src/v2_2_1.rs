//! OCPI **2.2.1** typed models (the primary production target).
//!
//! Modules: Versions, Credentials, Locations, Sessions, CDRs, Tariffs, Tokens,
//! Commands, ChargingProfiles, HubClientInfo.
//!
//! Populated incrementally — see milestones **M2–M6** in the roadmap.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::common::{
    BusinessDetails, CiString2, CiString3, CiString36, DisplayText, EnergyMix, GeoLocation, Image,
    Role, Url,
};
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

// ── Locations module ──────────────────────────────────────────────────────────
//
// Spec: specs/ocpi/2.2.1/mod_locations.asciidoc — §Object description + §Data types

/// Token type for publish access control.
///
/// Defined here as a forward reference; will be shared with the Tokens module.
/// Spec: `specs/ocpi/2.2.1/mod_tokens.asciidoc` — TokenType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TokenType {
    /// RFID token.
    Rfid,
    /// Other token type.
    Other,
    /// App-user token (virtual).
    AppUser,
    /// Ad-hoc user token.
    AdHocUser,
}

/// EVSE status.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — Status enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    /// Ready to start a new charging session.
    Available,
    /// Not accessible because of a physical barrier.
    Blocked,
    /// In use.
    Charging,
    /// Not yet active or temporarily unavailable, but not broken.
    Inoperative,
    /// Currently out of order.
    Outoforder,
    /// Planned, not yet operating.
    Planned,
    /// Discontinued or removed.
    Removed,
    /// Reserved for a particular EV driver.
    Reserved,
    /// No status information available.
    Unknown,
}

/// EVSE capabilities.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — Capability enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Capability {
    /// Supports charging profiles.
    ChargingProfileCapable,
    /// Supports charging preferences.
    ChargingPreferencesCapable,
    /// Payment terminal supports chip cards.
    ChipCardSupport,
    /// Payment terminal supports contactless cards.
    ContactlessCardSupport,
    /// Payment terminal accepts credit cards.
    CreditCardPayable,
    /// Payment terminal accepts debit cards.
    DebitCardPayable,
    /// Payment terminal with pin-code entry device.
    PedTerminal,
    /// Can be remotely started and stopped.
    RemoteStartStopCapable,
    /// Can be reserved.
    Reservable,
    /// Charging can be authorized with an RFID token.
    RfidReader,
    /// StartSession must include a `connector_id`.
    StartSessionConnectorRequired,
    /// Supports token groups (multiple tokens act as one).
    TokenGroupCapable,
    /// Connectors have a mechanical lock that can be unlocked remotely.
    UnlockCapable,
}

/// Connector format: socket or cable.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — ConnectorFormat enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorFormat {
    /// The EV user brings a fitting plug (socket on the station).
    Socket,
    /// Attached cable; EV must have a fitting inlet.
    Cable,
}

/// Connector standard / plug type.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — ConnectorType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorType {
    /// CHAdeMO, DC.
    #[serde(rename = "CHADEMO")]
    Chademo,
    /// ChaoJi (next-gen, harmonized CHAdeMO/GB/T), DC.
    #[serde(rename = "CHAOJI")]
    Chaoji,
    /// Domestic type A — NEMA 1-15, 2 pins.
    #[serde(rename = "DOMESTIC_A")]
    DomesticA,
    /// Domestic type B — NEMA 5-15, 3 pins.
    #[serde(rename = "DOMESTIC_B")]
    DomesticB,
    /// Domestic type C — CEE 7/17, 2 pins.
    #[serde(rename = "DOMESTIC_C")]
    DomesticC,
    /// Domestic type D — 3 pin.
    #[serde(rename = "DOMESTIC_D")]
    DomesticD,
    /// Domestic type E — CEE 7/5, 3 pins.
    #[serde(rename = "DOMESTIC_E")]
    DomesticE,
    /// Domestic type F — CEE 7/4 Schuko, 3 pins.
    #[serde(rename = "DOMESTIC_F")]
    DomesticF,
    /// Domestic type G — BS 1363 Commonwealth, 3 pins.
    #[serde(rename = "DOMESTIC_G")]
    DomesticG,
    /// Domestic type H — SI-32, 3 pins.
    #[serde(rename = "DOMESTIC_H")]
    DomesticH,
    /// Domestic type I — AS 3112, 3 pins.
    #[serde(rename = "DOMESTIC_I")]
    DomesticI,
    /// Domestic type J — SEV 1011, 3 pins.
    #[serde(rename = "DOMESTIC_J")]
    DomesticJ,
    /// Domestic type K — DS 60884-2-D1, 3 pins.
    #[serde(rename = "DOMESTIC_K")]
    DomesticK,
    /// Domestic type L — CEI 23-16-VII, 3 pins.
    #[serde(rename = "DOMESTIC_L")]
    DomesticL,
    /// Domestic type M — BS 546, 3 pins.
    #[serde(rename = "DOMESTIC_M")]
    DomesticM,
    /// Domestic type N — NBR 14136, 3 pins.
    #[serde(rename = "DOMESTIC_N")]
    DomesticN,
    /// Domestic type O — TIS 166-2549, 3 pins.
    #[serde(rename = "DOMESTIC_O")]
    DomesticO,
    /// Guobiao GB/T 20234.2 AC socket/connector.
    #[serde(rename = "GBT_AC")]
    GbtAc,
    /// Guobiao GB/T 20234.3 DC connector.
    #[serde(rename = "GBT_DC")]
    GbtDc,
    /// IEC 60309-2 industrial single phase 16A (typically blue).
    #[serde(rename = "IEC_60309_2_single_16")]
    Iec603092Single16,
    /// IEC 60309-2 industrial three phase 16A (typically red).
    #[serde(rename = "IEC_60309_2_three_16")]
    Iec603092Three16,
    /// IEC 60309-2 industrial three phase 32A (typically red).
    #[serde(rename = "IEC_60309_2_three_32")]
    Iec603092Three32,
    /// IEC 60309-2 industrial three phase 64A (typically red).
    #[serde(rename = "IEC_60309_2_three_64")]
    Iec603092Three64,
    /// IEC 62196 Type 1 "SAE J1772".
    #[serde(rename = "IEC_62196_T1")]
    Iec62196T1,
    /// IEC 62196 Type 1 Combo (DC).
    #[serde(rename = "IEC_62196_T1_COMBO")]
    Iec62196T1Combo,
    /// IEC 62196 Type 2 "Mennekes".
    #[serde(rename = "IEC_62196_T2")]
    Iec62196T2,
    /// IEC 62196 Type 2 Combo (DC).
    #[serde(rename = "IEC_62196_T2_COMBO")]
    Iec62196T2Combo,
    /// IEC 62196 Type 3A.
    #[serde(rename = "IEC_62196_T3A")]
    Iec62196T3A,
    /// IEC 62196 Type 3C "Scame".
    #[serde(rename = "IEC_62196_T3C")]
    Iec62196T3C,
    /// NEMA 5-20, 3 pins.
    #[serde(rename = "NEMA_5_20")]
    Nema520,
    /// NEMA 6-30, 3 pins.
    #[serde(rename = "NEMA_6_30")]
    Nema630,
    /// NEMA 6-50, 3 pins.
    #[serde(rename = "NEMA_6_50")]
    Nema650,
    /// NEMA 10-30, 3 pins.
    #[serde(rename = "NEMA_10_30")]
    Nema1030,
    /// NEMA 10-50, 3 pins.
    #[serde(rename = "NEMA_10_50")]
    Nema1050,
    /// NEMA 14-30, 3 pins, 30A.
    #[serde(rename = "NEMA_14_30")]
    Nema1430,
    /// NEMA 14-50, 3 pins, 50A.
    #[serde(rename = "NEMA_14_50")]
    Nema1450,
    /// Bottom-up pantograph, typically for bus charging.
    #[serde(rename = "PANTOGRAPH_BOTTOM_UP")]
    PantographBottomUp,
    /// Top-down pantograph, typically for bus charging.
    #[serde(rename = "PANTOGRAPH_TOP_DOWN")]
    PantographTopDown,
    /// Tesla Roadster-type connector (round, 4 pin).
    #[serde(rename = "TESLA_R")]
    TeslaR,
    /// Tesla Model S-type connector (oval, 5 pin).
    #[serde(rename = "TESLA_S")]
    TeslaS,
}

/// Location facility type.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — Facility enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Facility {
    /// A hotel.
    Hotel,
    /// A restaurant.
    Restaurant,
    /// A cafe.
    Cafe,
    /// A mall or shopping center.
    Mall,
    /// A supermarket.
    Supermarket,
    /// Sport facilities: gym, field, etc.
    Sport,
    /// A recreation area.
    RecreationArea,
    /// Located near a park or nature reserve.
    Nature,
    /// A museum.
    Museum,
    /// A bike/e-bike/e-scooter sharing location.
    BikeSharing,
    /// A bus stop.
    BusStop,
    /// A taxi stand.
    TaxiStand,
    /// A tram stop or station.
    TramStop,
    /// A metro station.
    MetroStation,
    /// A train station.
    TrainStation,
    /// An airport.
    Airport,
    /// A parking lot.
    ParkingLot,
    /// A carpool parking.
    CarpoolParking,
    /// A fuel station.
    FuelStation,
    /// Wifi or other internet access available.
    Wifi,
}

/// Image category.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — ImageCategory enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImageCategory {
    /// Photo of the physical charging device.
    Charger,
    /// Location entrance photo.
    Entrance,
    /// Location overview photo.
    Location,
    /// Logo of an associated roaming network.
    Network,
    /// Logo of the charge point operator.
    Operator,
    /// Other image type.
    Other,
    /// Logo of the charge point owner.
    Owner,
}

/// Parking restriction on an EVSE.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — ParkingRestriction enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ParkingRestriction {
    /// Reserved for electric vehicles only.
    EvOnly,
    /// Parking allowed only while plugged in.
    Plugged,
    /// Reserved for disabled people with valid ID.
    Disabled,
    /// For customers or guests only.
    Customers,
    /// Only suitable for motorcycles or scooters.
    Motorcycles,
}

/// General type of the charge point's location.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — ParkingType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ParkingType {
    /// Rest area along a motorway, freeway, or highway.
    AlongMotorway,
    /// Multistorey car park.
    ParkingGarage,
    /// Cleared area intended for parking (supermarket, bar, etc.).
    ParkingLot,
    /// On the driveway of a house or building.
    OnDriveway,
    /// Public space along a street.
    OnStreet,
    /// Multistorey car park, mainly underground.
    UndergroundGarage,
}

/// AC/DC power type of a connector.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — PowerType enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PowerType {
    /// AC single phase.
    #[serde(rename = "AC_1_PHASE")]
    Ac1Phase,
    /// AC two phases (only two of three phases connected).
    #[serde(rename = "AC_2_PHASE")]
    Ac2Phase,
    /// AC two phases using split-phase system.
    #[serde(rename = "AC_2_PHASE_SPLIT")]
    Ac2PhaseSplit,
    /// AC three phases.
    #[serde(rename = "AC_3_PHASE")]
    Ac3Phase,
    /// Direct current.
    #[serde(rename = "DC")]
    Dc,
}

/// Additional geographic point related to a location.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — AdditionalGeoLocation class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdditionalGeoLocation {
    /// Latitude, max 10 chars. Same format as `GeoLocation.latitude`.
    pub latitude: String,
    /// Longitude, max 11 chars. Same format as `GeoLocation.longitude`.
    pub longitude: String,
    /// Human-readable name of this point.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<DisplayText>,
}

/// Regular recurring operation hours (weekday-based).
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — RegularHours class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegularHours {
    /// Day of week: 1 = Monday … 7 = Sunday.
    pub weekday: u8,
    /// Begin of the period in local time, format `HH:MM`.
    pub period_begin: String,
    /// End of the period in local time, format `HH:MM`.
    pub period_end: String,
}

/// One exceptional opening or closing period.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — ExceptionalPeriod class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionalPeriod {
    /// Start of the exception (UTC).
    pub period_begin: DateTime<Utc>,
    /// End of the exception (UTC).
    pub period_end: DateTime<Utc>,
}

/// Opening and access hours of a location.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — Hours class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hours {
    /// `true` = open 24/7 except for the given exception periods.
    pub twentyfourseven: bool,
    /// Weekday-based regular hours (only when `twentyfourseven = false`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regular_hours: Vec<RegularHours>,
    /// Exceptional opening periods (additional to regular hours).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exceptional_openings: Vec<ExceptionalPeriod>,
    /// Exceptional closing periods (override regular hours).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exceptional_closings: Vec<ExceptionalPeriod>,
}

/// Planned EVSE status schedule entry.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — StatusSchedule class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusSchedule {
    /// Start of the scheduled period (UTC).
    pub period_begin: DateTime<Utc>,
    /// End of the scheduled period; absent means open-ended.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub period_end: Option<DateTime<Utc>>,
    /// EVSE status during this period.
    pub status: Status,
}

/// Token filter for non-public location visibility.
///
/// At least one of `uid`, `visual_number`, or `group_id` MUST be set.
/// When `uid` is set, `token_type` MUST also be set.
/// When `visual_number` is set, `issuer` MUST also be set.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — PublishTokenType class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishTokenType {
    /// Unique token ID.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub uid: Option<CiString36>,
    /// Token type (required when `uid` is set).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub token_type: Option<TokenType>,
    /// Visual number printed on the token.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub visual_number: Option<String>,
    /// Issuing company name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub issuer: Option<String>,
    /// Group ID linking multiple tokens.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub group_id: Option<CiString36>,
}

/// A single connector on an EVSE.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — Connector object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    /// Connector identifier within the EVSE (unique per EVSE, not globally).
    pub id: CiString36,
    /// Plug/socket standard.
    pub standard: ConnectorType,
    /// Socket or cable format.
    pub format: ConnectorFormat,
    /// AC or DC power type.
    pub power_type: PowerType,
    /// Maximum voltage (line-to-neutral for AC_3_PHASE), in volts.
    pub max_voltage: u32,
    /// Maximum amperage, in amperes.
    pub max_amperage: u32,
    /// Maximum power in watts (when lower than voltage × amperage).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_electric_power: Option<u32>,
    /// IDs of currently valid tariffs for this connector.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tariff_ids: Vec<CiString36>,
    /// URL to the operator's terms and conditions.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub terms_and_conditions: Option<Url>,
    /// Last update timestamp (UTC).
    pub last_updated: DateTime<Utc>,
}

/// An EVSE (Electric Vehicle Supply Equipment) within a location.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — EVSE object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evse {
    /// Technical identifier, unique within the CPO's platform.
    pub uid: CiString36,
    /// eMI3 EVSE ID (optional; may be absent when status is REMOVED).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub evse_id: Option<String>,
    /// Current status.
    pub status: Status,
    /// Planned status transitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_schedule: Vec<StatusSchedule>,
    /// EVSE capabilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
    /// Connectors on this EVSE (at least one required).
    pub connectors: Vec<Connector>,
    /// Floor level in a garage (e.g. `"-1"`, `"2"`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub floor_level: Option<String>,
    /// EVSE coordinates (more precise than the location coordinates).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub coordinates: Option<GeoLocation>,
    /// Visual reference number printed on the EVSE.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub physical_reference: Option<String>,
    /// Multi-language directions to reach this EVSE from the location.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<DisplayText>,
    /// Parking restrictions at this EVSE.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parking_restrictions: Vec<ParkingRestriction>,
    /// Images (photos, logos) for this EVSE.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    /// Last update timestamp (UTC).
    pub last_updated: DateTime<Utc>,
}

/// A charging location containing one or more EVSEs.
///
/// Spec: `specs/ocpi/2.2.1/mod_locations.asciidoc` — Location object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// ISO 3166-1 alpha-2 country code of the CPO that owns this location.
    pub country_code: CiString2,
    /// eMI3 party identifier of the CPO (3 chars).
    pub party_id: CiString3,
    /// Location identifier, unique within the CPO's platform.
    pub id: CiString36,
    /// Whether this location may be published publicly.
    pub publish: bool,
    /// Token filter list (only used when `publish = false`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publish_allowed_to: Vec<PublishTokenType>,
    /// Display name of the location.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    /// Street/block name and house number.
    pub address: String,
    /// City or town.
    pub city: String,
    /// Postal code (may be absent at some highway locations).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub postal_code: Option<String>,
    /// State or province (only when relevant).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub state: Option<String>,
    /// ISO 3166-1 alpha-3 country code (e.g. `"NLD"`).
    pub country: String,
    /// Coordinates of the location.
    pub coordinates: GeoLocation,
    /// Related geographic points (e.g. parking entrance).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_locations: Vec<AdditionalGeoLocation>,
    /// General parking type.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub parking_type: Option<ParkingType>,
    /// EVSEs at this location.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evses: Vec<Evse>,
    /// Human-readable directions to reach the location.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<DisplayText>,
    /// Operator details (if absent, use credentials module data).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub operator: Option<BusinessDetails>,
    /// Sub-operator details.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub suboperator: Option<BusinessDetails>,
    /// Owner details.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub owner: Option<BusinessDetails>,
    /// Facilities this location belongs to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facilities: Vec<Facility>,
    /// IANA timezone string (e.g. `"Europe/Amsterdam"`).
    pub time_zone: String,
    /// Opening hours of the location.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub opening_times: Option<Hours>,
    /// Whether EVSEs keep charging when the location is closed. Default: `true`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub charging_when_closed: Option<bool>,
    /// Images (photos, logos) for this location.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    /// Energy mix details for this location.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub energy_mix: Option<EnergyMix>,
    /// Last update timestamp (UTC).
    pub last_updated: DateTime<Utc>,
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

    // ── Locations types ───────────────────────────────────────────────────────

    #[test]
    fn status_enum_serde_roundtrip() {
        for s in [
            Status::Available,
            Status::Blocked,
            Status::Charging,
            Status::Inoperative,
            Status::Outoforder,
            Status::Planned,
            Status::Removed,
            Status::Reserved,
            Status::Unknown,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            let back: Status = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s);
        }
    }

    #[test]
    fn status_serializes_screaming_snake() {
        assert_eq!(
            serde_json::to_string(&Status::Available).unwrap(),
            "\"AVAILABLE\""
        );
        assert_eq!(
            serde_json::to_string(&Status::Outoforder).unwrap(),
            "\"OUTOFORDER\""
        );
    }

    #[test]
    fn connector_type_mixed_case_serde() {
        assert_eq!(
            serde_json::to_string(&ConnectorType::Iec603092Single16).unwrap(),
            "\"IEC_60309_2_single_16\""
        );
        assert_eq!(
            serde_json::to_string(&ConnectorType::Iec603092Three32).unwrap(),
            "\"IEC_60309_2_three_32\""
        );
        assert_eq!(
            serde_json::to_string(&ConnectorType::Iec62196T2Combo).unwrap(),
            "\"IEC_62196_T2_COMBO\""
        );
        assert_eq!(
            serde_json::to_string(&ConnectorType::Chademo).unwrap(),
            "\"CHADEMO\""
        );
    }

    #[test]
    fn power_type_serde_roundtrip() {
        for pt in [
            PowerType::Ac1Phase,
            PowerType::Ac2Phase,
            PowerType::Ac2PhaseSplit,
            PowerType::Ac3Phase,
            PowerType::Dc,
        ] {
            let json = serde_json::to_string(&pt).unwrap();
            let back: PowerType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, pt);
        }
        assert_eq!(
            serde_json::to_string(&PowerType::Ac3Phase).unwrap(),
            "\"AC_3_PHASE\""
        );
    }

    #[test]
    fn parking_type_serde_roundtrip() {
        for pt in [
            ParkingType::AlongMotorway,
            ParkingType::ParkingGarage,
            ParkingType::OnStreet,
            ParkingType::UndergroundGarage,
        ] {
            let json = serde_json::to_string(&pt).unwrap();
            let back: ParkingType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, pt);
        }
    }

    fn now() -> DateTime<Utc> {
        "2024-01-15T10:00:00Z".parse().unwrap()
    }

    fn make_connector() -> Connector {
        Connector {
            id: CiString36::try_from("1").unwrap(),
            standard: ConnectorType::Iec62196T2,
            format: ConnectorFormat::Socket,
            power_type: PowerType::Ac3Phase,
            max_voltage: 400,
            max_amperage: 32,
            max_electric_power: None,
            tariff_ids: vec![],
            terms_and_conditions: None,
            last_updated: now(),
        }
    }

    #[test]
    fn connector_serde_roundtrip() {
        let c = make_connector();
        let json = serde_json::to_string(&c).unwrap();
        let back: Connector = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        assert!(!json.contains("tariff_ids"));
        assert!(!json.contains("terms_and_conditions"));
    }

    fn make_evse() -> Evse {
        Evse {
            uid: CiString36::try_from("EVSE-001").unwrap(),
            evse_id: Some("NL*EXA*E001".into()),
            status: Status::Available,
            status_schedule: vec![],
            capabilities: vec![Capability::RfidReader, Capability::Reservable],
            connectors: vec![make_connector()],
            floor_level: None,
            coordinates: None,
            physical_reference: Some("Terminal 1".into()),
            directions: vec![],
            parking_restrictions: vec![],
            images: vec![],
            last_updated: now(),
        }
    }

    #[test]
    fn evse_serde_roundtrip() {
        let e = make_evse();
        let json = serde_json::to_string(&e).unwrap();
        let back: Evse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
        assert!(!json.contains("status_schedule"));
        assert!(!json.contains("floor_level"));
    }

    fn make_location() -> Location {
        Location {
            country_code: CiString2::try_from("NL").unwrap(),
            party_id: CiString3::try_from("EXA").unwrap(),
            id: CiString36::try_from("LOC001").unwrap(),
            publish: true,
            publish_allowed_to: vec![],
            name: Some("Example Charging Station".into()),
            address: "Stationsplein 1".into(),
            city: "Amsterdam".into(),
            postal_code: Some("1012 AB".into()),
            state: None,
            country: "NLD".into(),
            coordinates: GeoLocation {
                latitude: "52.378773".into(),
                longitude: "4.900052".into(),
            },
            related_locations: vec![],
            parking_type: Some(ParkingType::OnStreet),
            evses: vec![make_evse()],
            directions: vec![],
            operator: None,
            suboperator: None,
            owner: None,
            facilities: vec![],
            time_zone: "Europe/Amsterdam".into(),
            opening_times: None,
            charging_when_closed: Some(true),
            images: vec![],
            energy_mix: None,
            last_updated: now(),
        }
    }

    #[test]
    fn location_serde_roundtrip() {
        let loc = make_location();
        let json = serde_json::to_string(&loc).unwrap();
        let back: Location = serde_json::from_str(&json).unwrap();
        assert_eq!(back, loc);
        assert!(!json.contains("publish_allowed_to"));
        assert!(!json.contains("state"));
    }

    #[test]
    fn location_optional_fields_absent_when_none() {
        let loc = make_location();
        let json = serde_json::to_string(&loc).unwrap();
        assert!(!json.contains("\"operator\""));
        assert!(!json.contains("\"energy_mix\""));
        assert!(!json.contains("\"opening_times\""));
        assert!(!json.contains("\"parking_restrictions\""));
    }

    #[test]
    fn hours_24_7_serde_roundtrip() {
        let h = Hours {
            twentyfourseven: true,
            regular_hours: vec![],
            exceptional_openings: vec![],
            exceptional_closings: vec![ExceptionalPeriod {
                period_begin: "2024-12-25T03:00:00Z".parse().unwrap(),
                period_end: "2024-12-25T05:00:00Z".parse().unwrap(),
            }],
        };
        let json = serde_json::to_string(&h).unwrap();
        let back: Hours = serde_json::from_str(&json).unwrap();
        assert!(back.twentyfourseven);
        assert_eq!(back.exceptional_closings.len(), 1);
        assert!(!json.contains("regular_hours"));
        assert!(!json.contains("exceptional_openings"));
    }

    #[test]
    fn publish_token_type_serde_roundtrip() {
        let pt = PublishTokenType {
            uid: Some(CiString36::try_from("abc123").unwrap()),
            token_type: Some(TokenType::Rfid),
            visual_number: None,
            issuer: None,
            group_id: None,
        };
        let json = serde_json::to_string(&pt).unwrap();
        let back: PublishTokenType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pt);
        assert!(json.contains("\"type\":\"RFID\""));
    }

    #[test]
    fn facility_enum_serde() {
        assert_eq!(
            serde_json::to_string(&Facility::RecreationArea).unwrap(),
            "\"RECREATION_AREA\""
        );
        let back: Facility = serde_json::from_str("\"BIKE_SHARING\"").unwrap();
        assert_eq!(back, Facility::BikeSharing);
    }
}
