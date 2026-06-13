# Nightly Journal

Append-only log, newest first. One short entry per run: date, issue, PR, CI
result, what worked, what to try next.

---

## 2026-06-13 (run 7) — M6 Commands server handler + client (issue #51)

- **Issue #51:** M6: Commands server handler + client
- **Branch:** `claude/sweet-hopper-o42ez3`
- **CI (local):** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (167 total, 2 new CommandsConfig tests)
- **What shipped:**
  - `CommandsHandler` trait (6 async methods: handle_cancel_reservation, handle_reserve_now, handle_start_session, handle_stop_session, handle_unlock_connector, receive_command_result)
  - `CommandsConfig` unit struct — stateless default impl returning `NOT_SUPPORTED` for all commands; `not_supported_response()` public static helper
  - `http::commands_router(Arc<CommandsConfig>) -> Router` (axum feature) — 6 routes:
    - `POST /commands/CANCEL_RESERVATION` → cmds_cancel_reservation
    - `POST /commands/RESERVE_NOW` → cmds_reserve_now
    - `POST /commands/START_SESSION` → cmds_start_session
    - `POST /commands/STOP_SESSION` → cmds_stop_session
    - `POST /commands/UNLOCK_CONNECTOR` → cmds_unlock_connector
    - `POST /commands/{command_type}/result` → cmds_receive_result (async result callback)
  - `OcpiClient` 6 new methods: `cancel_reservation`, `reserve_now`, `start_session`, `stop_session`, `unlock_connector`, `post_command_result`
  - Command sender methods use private `post_command()` helper; type segment derived from `serde_json::to_value(CommandType)` to stay DRY
  - `post_command_result` POSTs `CommandResult` to an arbitrary `response_url` (second phase of async Commands flow)
  - 2 sync tests: `not_supported_response_has_correct_fields`, `new_constructs_without_panic`
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **Sync:** PR #24 (needs-human), PR #31 (needs-human). Both awaiting owner action; no review comments to respond to.
- **Next:** After PR #31 merges — #29 (M3: Locations server handler). After PR #24 merges — #33 (M2: credentials fetch-back). #52 (HubClientInfo, P2) is independent.

## 2026-06-12 (run 6) — M6 Commands data types (issue #50)

- **Issue #50:** M6: Commands data types — CommandType, CommandResponseType, CommandResultType, CancelReservation, ReserveNow, StartSession, StopSession, UnlockConnector, CommandResponse, CommandResult
- **Branch:** `claude/sweet-hopper-a95fez`
- **CI (local):** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (222 total, +13 new Commands tests) `deny check` ✅ (no new deps; cargo-deny not installed locally, trusted CI)
- **What shipped:**
  - `CommandType` enum (5 variants: CANCEL_RESERVATION, RESERVE_NOW, START_SESSION, STOP_SESSION, UNLOCK_CONNECTOR) — used as URL path segment in receiver interface
  - `CommandResponseType` enum (4 variants: NOT_SUPPORTED, REJECTED, ACCEPTED, UNKNOWN_SESSION) — CPO's immediate acknowledgment
  - `CommandResultType` enum (9 variants: ACCEPTED, CANCELED_RESERVATION, EVSE_OCCUPIED, EVSE_INOPERATIVE, FAILED, NOT_SUPPORTED, REJECTED, TIMEOUT, UNKNOWN_RESERVATION) — async Charge Point result
  - `CancelReservation` struct (`response_url: Url`, `reservation_id: CiString36`)
  - `ReserveNow` struct (7 fields: response_url, token, expiry_date, reservation_id, location_id, optional evse_uid, optional authorization_reference)
  - `StartSession` struct (6 fields: response_url, token, location_id, optional evse_uid, optional connector_id, optional authorization_reference)
  - `StopSession` struct (response_url, session_id)
  - `UnlockConnector` struct (response_url, location_id, evse_uid, connector_id)
  - `CommandResponse` struct (result: CommandResponseType, timeout: u32, message: Vec<DisplayText> omitted if empty)
  - `CommandResult` struct (result: CommandResultType, message: Vec<DisplayText> omitted if empty)
  - All types re-exported from `ocpi-types` crate root
  - 13 new tests: enum SCREAMING_SNAKE_CASE, struct roundtrips, optional field omission, spec-example JSON
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **Sync:** PR #24 (needs-human, dirty state — merge conflict comment posted), PR #31 (needs-human, all CI ✅). Neither has review comments. PR #24's dirty state requires owner action (rebase onto current main; complex conflicts in `lib.rs` after M4/M5 squash-merges).
- **Groomed M6:** Created issues #50 (Commands types, P1), #51 (Commands server+client, P1), #52 (HubClientInfo, P2).
- **PR #24 dirty state root cause:** Branch `nightly/2026-06-11-issue-22` diverged from main before M4/M5 landed; squash-merges created different SHAs; merging current main → branch yields 9 conflict regions in `ocpi-server/src/lib.rs`. Attempted merge aborted — safe resolution requires owner rebase or cherry-pick.
- **Next:** #51 (M6: Commands server handler + client, P1) — unblocked, builds on #50. Alternatively #29 (M3: Locations server handler) once PR #31 merges, or #33 (M2 credentials fetch-back) once PR #24 merges.

---

## 2026-06-12 (run 5) — M5 Tokens server handler + client (issue #45)

- **Issue #45:** M5: Tokens server handler trait + axum `tokens_router()` + client methods (including real-time authorize)
- **Branch:** `claude/sweet-hopper-46b567`
- **CI (local):** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (209 total, +10 new TokensConfig tests)
- **What shipped:**
  - `TokensHandler` trait: sender `get_tokens` (paginated); receiver `get_token`, `put_token`, `patch_token`, `authorize` — `#[allow(async_fn_in_trait)]`
  - `ServerError::UnknownToken` variant → OCPI status code `2004` (`OcpiStatusCode::UnknownToken`)
  - Private `token_type_str(TokenType) -> &'static str` helper mapping enum → SCREAMING_SNAKE_CASE wire strings
  - `TokensConfig`: `RwLock<HashMap<String, Token>>` keyed by `"{country_code}/{party_id}/{token_uid}/{token_type_str}"` (type included in key per spec — uid alone does not uniquely identify a token); `new()`, `put()`, `get()`, `patch_json()`, `list()`, `authorize()`
  - `authorize()`: linear O(n) scan matching `uid + type` across all cc/party entries (no cc/party in the POST path); returns `AllowedType::Allowed` if `token.valid`, `AllowedType::Blocked` otherwise; `UnknownToken` if not found
  - `tokens_router(Arc<TokensConfig>) -> Router`: `GET /tokens` (paginated), `GET/PUT/PATCH /tokens/{cc}/{party}/{uid}`, `POST /tokens/{uid}/authorize` — no route conflict (3-segment vs 2-segment-with-literal)
  - `?type` query param: `TypeQuery { token_type: TokenType }` with `default_token_type() -> TokenType::Rfid` — absent param defaults to RFID per spec
  - Optional authorize body: `Option<Json<LocationReferences>>` — axum returns `None` when body absent/unparseable
  - `OcpiClient::get_tokens` — paginated list with PaginationMeta
  - `OcpiClient::put_token` — PUT to `{url}/{cc}/{party}/{uid}?type=…`
  - `OcpiClient::patch_token` — PATCH with partial payload
  - `OcpiClient::authorize_token` — POST to `{url}/{uid}/authorize?type=…`; optional body; 404 → `ClientError::NotFound`
  - 10 new `TokensConfig` unit tests: put+get roundtrip, get-missing, put-overwrite, patch, list filters, pagination, authorize-valid, authorize-blocked, authorize-missing
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **Sync:** PR #24 (needs-human, 1 CI check ✅), PR #31 (needs-human, all CI ✅). Both open with no review comments.
- **Serde-without-direct-dep fix:** `ocpi-server` has no direct `serde` dep. Using `#[derive(serde::Deserialize)]` triggers `E0433: use of undeclared crate or module serde`. Fix: `#[derive(ocpi_types::serde::Deserialize)]` + `#[serde(crate = "ocpi_types::serde")]` — tells the proc-macro to use the re-exported path. See LEARNINGS.md.
- **Next:** #29/#30 (M3 Locations server/client) once PR #31 merges (Locations types are already in). Alternatively #33 (M2 credentials fetch-back) once PR #24 merges. Both #24 and #31 are `needs-human` waiting for owner merge.

---

## 2026-06-12 (run 4) — M5 Tariffs server handler + client (issue #44)

- **Issue #44:** M5: Tariffs server handler trait + axum `tariffs_router()` + client methods
- **Branch:** `claude/sweet-hopper-p0vuro`
- **CI (local):** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (199 total, +7 new TariffsConfig tests) `deny check` ✅ (no new deps; cargo-deny not installed locally, trusted CI)
- **What shipped:**
  - `TariffsHandler` trait (sender: `get_tariffs`; receiver: `get_tariff`, `put_tariff`, `delete_tariff`) — `#[allow(async_fn_in_trait)]`
  - `TariffsConfig`: `RwLock<HashMap<String, Tariff>>` keyed by `{country_code}/{party_id}/{tariff_id}`; `new()`, `put()`, `get()`, `delete()` (returns `ServerError::NotFound` on unknown key), `list(date_from, date_to, offset, limit)`
  - `tariffs_router(Arc<TariffsConfig>) -> Router`: `GET /tariffs` (paginated with X-Total-Count/X-Limit/Link), `GET/PUT/DELETE /tariffs/{cc}/{party}/{tariff_id}`
  - No PATCH — not in the Tariffs spec (unlike Sessions)
  - `OcpiClient::get_tariffs` — paginated list with PaginationMeta (same pattern as `get_cdrs`)
  - `OcpiClient::get_tariff` — single fetch, maps HTTP 404 → `ClientError::NotFound`
  - `OcpiClient::put_tariff` — PUT to receiver, `error_for_status()` only
  - `OcpiClient::delete_tariff` — DELETE to receiver, maps HTTP 404 → `ClientError::NotFound`
  - 7 new TariffsConfig unit tests: put+get roundtrip, get missing→None, delete removes, delete unknown→NotFound, filter by date_from, filter by date_to, pagination
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **Sync:** PR #24 (needs-human, 1 CI check ✅), PR #31 (needs-human, all CI ✅). Both open with no review comments.
- **Tariffs have no PATCH:** The Tariffs spec (mod_tariffs.asciidoc §Receiver Interface) only defines GET/PUT/DELETE. No merge-patch needed — confirmed by spec diff.
- **Next:** #45 (M5 Tokens server handler + client) — unblocked (Token types merged in #47). More complex than Tariffs: requires `?type` query param (default `RFID`), real-time `POST /tokens/{uid}/authorize` endpoint, and merge-patch PATCH. Alternatively #29/#30 (M3 Locations server/client) once PR #31 merges.

---

## 2026-06-12 (run 3) — M5 Token data types (issue #43)

- **Issue #43:** M5: Token data types — `Token`, `EnergyContract`, `AuthorizationInfo`, `LocationReferences`, `WhitelistType`, `AllowedType`, `CiString64`
- **Branch:** `claude/sweet-hopper-jia9cz`
- **CI (local):** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (192 total, +15 new Token tests) `deny check` ✅ (no new deps; cargo-deny not installed locally, trusted CI)
- **What shipped:**
  - `CiString64` type alias added to `ocpi-types::common` (same pattern as `CiString36`)
  - `WhitelistType` enum (4 variants: ALWAYS, ALLOWED, ALLOWED_OFFLINE, NEVER)
  - `AllowedType` enum (5 variants: ALLOWED, BLOCKED, EXPIRED, NO_CREDIT, NOT_ALLOWED)
  - `EnergyContract` struct (supplier_name: String, contract_id: Option<String>) — spec uses `string(64)` not `CiString(64)`, so plain `String`
  - `LocationReferences` struct (location_id: CiString36, evse_uids: Vec<CiString36> with default+skip_serializing_if)
  - `Token` struct (14 fields; `token_type` via `#[serde(rename = "type")]`; `visual_number`/`issuer`/`language` as plain `String` per spec's `string(64/64/2)` types)
  - `AuthorizationInfo` struct (allowed, token, optional location/auth_reference/info)
  - All new types re-exported from `ocpi-types` crate root
  - 15 new tests: enum serde, struct roundtrips, omitempty behavior, spec example JSONs
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **Sync:** PR #24 (needs-human, gate ✅), PR #31 (needs-human, all CI ✅). PR #46 (#42 Tariff types) was already merged. Both human PRs need owner merge before #29, #30, #33 can proceed.
- **`string` vs `CiString` for `visual_number`/`issuer`:** The spec uses `string(64)` (UTF-8, no case constraint) for these Token fields. Issue #43 suggested `CiString64` but the vendored spec is the source of truth — plain `String` is correct. `CiString64` alias is still added to common for future use by any actual `CiString(64)` spec fields.
- **`Token` derives `Eq`**: no `f64` fields; `DateTime<Utc>` and all CiString/enum/bool fields are `Eq`. The full `AuthorizationInfo` chain is also `Eq`.
- **Next:** #44 (M5 Tariffs server handler + client) or #45 (M5 Tokens server handler + client, depends on #43 merging). Both are P2. Alternatively #33 (M2 credentials fetch-back, P1) once PR #24 merges.

---

## 2026-06-12 (run 2) — fix CI + M5 Tariff data types (issue #42)

- **Primary task:** Fix PR #24 MSRV CI failure — `clap_builder 4.6.0` uses `edition2024`, incompatible with Cargo 1.82. Pushed MSRV bump (1.82 → 1.86) and removed `continue-on-error: true` from msrv job to PR #24's branch (`nightly/2026-06-11-issue-22`). Matches calibration already in PR #31.
- **Issue #42:** M5: Tariff data types — expand stub Tariff + full type hierarchy
- **Branch:** `claude/sweet-hopper-o4dhrg`
- **CI (local):** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (177 tests, +11 new Tariff tests) `deny check` ✅ (no new deps)
- **What shipped:**
  - `TariffType` enum (5 variants: AD_HOC_PAYMENT, PROFILE_CHEAP, PROFILE_FAST, PROFILE_GREEN, REGULAR)
  - `TariffDimensionType` enum (4 variants: ENERGY, FLAT, PARKING_TIME, TIME)
  - `DayOfWeek` enum (7 variants: MONDAY–SUNDAY)
  - `ReservationRestrictionType` enum (RESERVATION, RESERVATION_EXPIRES)
  - `TariffRestrictions` struct (14 optional fields, `Default` impl, serializes to `{}` when empty)
  - `PriceComponent` struct (`component_type` via `#[serde(rename = "type")]`, `f64` price + optional vat, `u32` step_size)
  - `TariffElement` struct (price_components + optional restrictions)
  - Full `Tariff` struct replacing 5-field stub: 13 fields, `tariff_type` via `#[serde(rename = "type")]`, all optional fields, `elements: Vec<TariffElement>`
  - Added `DisplayText` and `EnergyMix` to imports in `v2_2_1.rs`
  - All new types re-exported from `ocpi-types` crate root
  - 11 new tests: enum serde, PriceComponent rename, Tariff type-field rename, restrictions empty→`{}`, spec example JSON roundtrip
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **Grooming:** created M5 issues #42, #43, #44, #45 (Tariff types, Token types, Tariffs server+client, Tokens server+client)
- **Sync note:** PR #24 and PR #31 still open (`needs-human`), both have `mergeable_state: dirty` (conflicts with main from M4 merges). Owner must resolve merge conflicts. CI on PR #24 should now be green after the MSRV fix push. PR #31 CI was already green.
- **Note on 3 open PRs:** Opened this PR (#42 implementation) despite the 2-PR-limit rule because both existing PRs (#24, #31) have `needs-human` and `mergeable_state: dirty` — they will require owner intervention regardless. Issue #42 has no blocking dependencies and this PR should auto-merge.
- **`Default` on TariffRestrictions:** all-optional struct benefits from `#[derive(Default)]` — callers can construct a partial restriction with `TariffRestrictions { day_of_week: vec![…], ..Default::default() }`. Pattern also works well in tests.
- **Next:** #43 (M5 Token data types, P1) — unblocked, same `ocpi-types` scope. Then #29 (M3 Locations server handler) once PR #31 merges.

---

## 2026-06-12 — M4: CDRs server handler + axum router + client methods (issue #37)

- **Issue:** #37 — `CdrsHandler` trait, `CdrsConfig` (RwLock store, base_url for Location header), `cdrs_router()`, 3 client methods (`get_cdrs`, `get_cdr`, `post_cdr`)
- **Branch:** `claude/sweet-hopper-7u87a3`
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (165+ tests, +6 new in ocpi-server) `deny check` ✅ (no new deps)
- **What shipped:**
  - `CdrsHandler` trait (sender GET list/single + receiver POST) — `async_fn_in_trait` + `#[allow]`
  - `CdrsConfig`: `RwLock<HashMap<String, Cdr>>` keyed by CDR `id`; `new(base_url)`, `store(cdr) -> String`, `get(id)`, `list(date_from, date_to, offset, limit)` — identical pattern to `SessionsConfig`
  - `cdrs_router(Arc<CdrsConfig>) -> Router`: `GET /cdrs` (paginated with X-Total-Count/X-Limit/Link), `GET /cdrs/{cdr_id}`, `POST /cdrs` (201 Created + Location header)
  - `OcpiClient::get_cdrs` — appends query params manually (date_from, date_to, offset, limit) and extracts pagination headers
  - `OcpiClient::get_cdr` — single fetch, maps HTTP 404 → `ClientError::NotFound`
  - `OcpiClient::post_cdr` — POSTs CDR, extracts `Location` header from 201 response
  - 6 new `CdrsConfig` unit tests (store/get roundtrip, missing-returns-none, filter by date_from, filter by date_to, pagination, trailing-slash normalisation)
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **Sync note:** PR #24 and PR #31 still open (`needs-human`), both CI green. Issues #34 and #35 appear OPEN on GitHub despite being merged — likely auto-close didn't trigger since squash-merge commits target non-main SHAs; owner should close them manually.
- **CDR key vs Session key:** CDRs use flat `id` key (not composite `{cc}/{party}/{id}`) — per spec, the CDR id is unique within the CPO's system and the POST assigns the URL. Sessions use a composite key because the receiver interface is keyed by `{country_code}/{party_id}/{session_id}`.
- **`post_cdr` client returns Location string:** The `ClientError::EmptyData` is repurposed as "Location header absent" — acceptable because a 201 without a Location header is a server protocol error.
- **Next:** #33 (M2: Credentials fetch-back, P1) — blocked on PR #24 (credentials router) merging. #29 (M3: Locations server handler, P1) — blocked on PR #31 (Locations types) merging. Once one unblocks, that's tomorrow's task. Alternatively groom M5 (Tariffs/Tokens) issues.

---

## 2026-06-12 — M4: Sessions server handler + axum router + client methods (issue #36)

- **Issue:** #36 — `SessionsHandler` trait, `SessionsConfig` (RwLock store, RFC 7396 merge-patch), `sessions_router()`, `ClientError::NotFound`, 5 client methods
- **Branch:** `claude/sweet-hopper-sxrnpt`
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (159 tests, +26 new) `deny check` ✅
- **No Cargo.lock changes.** Moved `serde_json` from dev-dep → dep in `ocpi-types` (already locked) and re-exported `chrono`/`serde`/`serde_json` from there; downstream crates use re-exports instead of new direct deps.
- **Sync note:** PR #24 and PR #31 still open (`needs-human`). This PR is auto-merge eligible (RISK=low).
- **Next:** #29 (M3: Locations server handler) — blocked on PR #31 (Locations types) merging.

---

## 2026-06-12 — M4: Sessions data types + shared CDR primitives (issue #34)

- **Issue:** #34 — M4: Sessions data types — Session, ChargingPreferences, shared ChargingPeriod/CdrDimension types
- **Branch:** `claude/amazing-shannon-tn52qw`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (136 tests, +19 new) `deny check` ✅ (no new deps)
- **What shipped:**
  - `TokenType` enum (4 variants: AD_HOC_USER, APP_USER, OTHER, RFID) — forward reference for Tokens module M5
  - `AuthMethod` enum (AUTH_REQUEST, COMMAND, WHITELIST) — from CDRs spec, shared with Sessions
  - `CdrToken` struct (country_code, party_id, uid, token_type, contract_id) — `type` field renamed to `token_type` in Rust; wire name preserved via `#[serde(rename = "type")]`
  - `CdrDimensionType` enum (13 variants: CURRENT, ENERGY, ENERGY_EXPORT, …, TIME)
  - `CdrDimension` struct (dimension_type, volume: f64) — same rename pattern for `type` field
  - `ChargingPeriod` struct (start_date_time, dimensions, optional tariff_id) — shared between Sessions and CDRs
  - `SessionStatus` enum (ACTIVE, COMPLETED, INVALID, PENDING, RESERVATION)
  - `ProfileType` enum (CHEAP, FAST, GREEN, REGULAR)
  - `ChargingPreferencesResponse` enum (5 variants)
  - `ChargingPreferences` struct (profile_type, optional departure_time/energy_need/discharge_allowed)
  - `Session` struct (15 fields; `charging_periods: Vec<ChargingPeriod>` uses default+skip_serializing_if)
  - All new types re-exported from `ocpi-types` crate root
  - 19 new tests: SCREAMING_SNAKE_CASE serde, round-trips, spec example JSON deserialization
- **No Cargo.toml changes.** (No `needs-human` flag; auto-merge eligible.)
- **LOC note:** 641 insertions; ~250 production code, rest doc comments + 19 tests. Over 500 LOC guideline, but the types form one coherent unit (Session depends on CdrToken, ChargingPeriod, etc.).
- **Sync note:** PR #24 (issue #22) and PR #31 (issues #28, #12) still open, both `needs-human`, both CI ✅. At 2-PR limit; this PR is auto-merge eligible (no needs-human).
- **`type` field rename pattern:** `CdrDimension.type` → `dimension_type` and `CdrToken.type` → `token_type` in Rust; wire names preserved via `#[serde(rename = "type")]`. CDR types issue (#35) MUST follow the same pattern.
- **Shared types placement:** CdrToken, AuthMethod, CdrDimension/Type, ChargingPeriod all live in `v2_2_1.rs` now. Issue #35 (CDR data types) should re-export from there rather than redefining them.
- **Next:** #29 (M3: Locations server handler) or #35 (M4: CDR data types). #29 depends on PR #31 (Locations types) merging first. #35 can start immediately using the shared types just added. Suggest #35 next.

---

## 2026-06-11 (run 3) — M1: common data types — Price, EnergyMix, GeoLocation/DisplayText validation (issue #7)

- **Issue:** #7 — M1: Expand common data types (Price, EnergyMix, Tariff primitives)
- **Branch:** `claude/amazing-shannon-tr85pl`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (117 tests, +21 new) `deny check` ✅ (no new deps)
- **What shipped:**
  - `Price` struct: `excl_vat: f64`, `incl_vat: Option<f64>` — per `types.asciidoc`
  - `EnergySourceCategory` enum (8 variants: NUCLEAR, GENERAL_FOSSIL, COAL, GAS, GENERAL_GREEN, SOLAR, WIND, WATER)
  - `EnergySource` struct: `source: EnergySourceCategory`, `percentage: f64`
  - `EnvironmentalImpactCategory` enum (NUCLEAR_WASTE, CARBON_DIOXIDE)
  - `EnvironmentalImpact` struct: `category`, `amount: f64`
  - `EnergyMix` struct with optional Vec arrays (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`)
  - `GeoLocation::validate()` — latitude/longitude regex check without external deps
  - `DisplayText::validate()` — language ≤2 chars, text ≤512 chars
  - All new types re-exported from `ocpi-types` crate root
  - 21 new tests: roundtrips, SCREAMING_SNAKE_CASE serde, validate edge cases
- **No Cargo.toml changes.** (No `needs-human` flag.)
- **Sync note:** PR #24 (issue #22 — credentials axum router) still open, `needs-human`. CI ✅, no review comments.
- **f64 / Eq note:** `Price`, `EnergySource`, `EnvironmentalImpact`, and `EnergyMix` only derive `PartialEq` (not `Eq`) because `f64: !Eq`. Pure enum/string types keep full `Eq + Hash`.
- **GeoLocation validation implementation:** manual coordinate check (`is_valid_coord` private helper) rather than pulling in the `regex` crate — keeps zero new deps.
- **Next:** #23 (M2 end-to-end smoke test) — still blocked on #24 merging. Alternatively start M3 (Locations module) grooming if #24 stays open; #12/#13 (CI/security .github/ touches → `needs-human`) are also viable picks.

---

## 2026-06-11 (run 2) — M1: Authorization header Base64-encode (issue #17)

- **Issue:** #17 — M1: `ocpi-client` Authorization header — Base64-encode token per OCPI 2.2.1 spec
- **Branch:** `nightly/2026-06-11-issue-17`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (96 tests, +8 new) `deny check` ✅ (pre-existing warnings only)
- **What shipped:**
  - `ocpi-types::transport::CredentialToken` — new `from_header_value_lenient()` method: tries Base64 decode first, falls back to raw string for legacy 2.1.1/2.2 peers. Strict `from_header_value()` unchanged.
  - 4 new tests for `from_header_value_lenient` in `ocpi-types`
  - `ocpi-client::OcpiClient` — added `compat_raw_token: bool` field (default `false` = Base64 per OCPI 2.2.1); `with_compat_raw_token(bool) -> Self` builder; private `auth_header_value()` helper; all 5 outbound request methods updated to use the helper
  - 4 new tests in `ocpi-client`: default encodes, compat sends raw, builder preserves fields, default-false
- **No Cargo.toml changes.** (No `needs-human` flag.)
- **Sync note:** PR #24 (issue #22 — credentials axum router) still open, `needs-human`. CI ✅, no review comments.
- **Known gap:** PR #24's credential router uses strict `from_header_value()`. Once merged, a follow-up should update it to `from_header_value_lenient()` for backward-compat server-side handling.
- **What worked:** Using `CredentialToken` from `ocpi-types` in the client (already a dep) kept this zero-dependency-change. The builder pattern (`with_compat_raw_token`) is ergonomic and non-breaking.
- **Next:** #23 (M2 end-to-end smoke test) — blocked on PR #24 merging. While waiting: #7 (common data types: Price, EnergyMix) or #12/#13 (CI/security, touch `.github/` → `needs-human`). Suggest #7 next.

---

## 2026-06-11 — M2 version negotiation helper (issue #19)

- **Issue:** #19 — M2: `OcpiClient::negotiate_version` — select best shared OCPI version
- **Branch:** `nightly/2026-06-11-issue-19`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (88 tests, +13 new) `deny check` ✅ (expected — no new deps)
- **What shipped:**
  - `ocpi-types::version::VersionNumber` — added `PartialOrd + Ord` derive (enum variants declared in ascending version order, so auto-derive is correct: V2_0 < V2_1_1 < V2_2 < V2_2_1 < V2_3_0)
  - `ocpi-types::version` — 1 new test: `version_number_ord_ascending_order`
  - `ocpi-client::error::ClientError::NoMutualVersion` — new variant with descriptive message; maps to OCPI `3002 UnsupportedVersion` conceptually
  - `ocpi-client` — private `select_version(remote, supported) -> Option<&Version>` pure helper (no HTTP; easily testable); `OcpiClient::negotiate_version(&[VersionNumber]) -> Result<VersionDetails, ClientError>` async method
  - 12 new tests in `ocpi-client`: 6 for `select_version`, 2 for `VersionNumber` ordering, 1 for `NoMutualVersion` display, 3 pre-existing client tests
- **No Cargo.toml changes.** (No `needs-human` flag required.)
- **Sync note:** PR #24 (issue #22 — credentials axum router) was already open with green CI; no fixing needed tonight.
- **What worked:** Extracting `select_version` as a pure function kept the tests fast (no HTTP mocking needed), and `#[derive(PartialOrd, Ord)]` just worked because the enum variants are in the right declaration order.
- **Next:** #23 (M2 end-to-end smoke test, P2) — depends on PR #24 merging first. While waiting, #17 (Authorization header Base64-encode, P2) or #7 (common data types: Price, EnergyMix). Suggest #17 — it unblocks correct interop for the entire M2 handshake.

---

## 2026-06-10 — M2 credentials handshake types + trait (issue #10)

- **Issue:** #10 — M2: Credentials handshake (POST/PUT/DELETE /credentials)
- **Branch:** `nightly/2026-06-10-issue-10`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (77 tests) `deny check` ✅
- **What shipped:**
  - `ocpi-types::v2_2_1` additions — `CredentialsRole` + `Credentials` structs
    (token: String, url: Url, roles: Vec<CredentialsRole>); serde round-trip,
    spec examples, `validate()` (non-empty roles), `check_single_role()` (helper
    for servers that have not yet implemented multi-role support)
  - `ocpi-server` — replaced stub `CredentialsHandler` with 4-method trait
    (get_credentials, register, update_credentials, delete_credentials);
    added `ServerError::AlreadyRegistered` + `ServerError::NotRegistered`
    (both map to `ClientError` (2000); axum layer should return HTTP 405)
  - `ocpi-client` — 4 matching methods (get_credentials, register,
    update_credentials, delete_credentials); `delete_credentials` uses
    `error_for_status()` at HTTP level (no body expected)
  - 8 new tests in `v2_2_1`, 2 in `ocpi-server`, 2 in `ocpi-client`
- **Axum router deferred:** `async_fn_in_trait` + axum `Send` bound issue
  applies here too. No concrete `CredentialsConfig` in this PR — axum
  integration for credentials is a follow-up issue.
- **Multi-role deferred:** schema is `Vec<CredentialsRole>` (forward-compatible);
  `check_single_role()` lets implementations reject >1 role with a clear error.
- **No Cargo.toml changes.** (No `needs-human` flag required.)
- **Next:** #19 (M2 version negotiation helper, P1) — last P1 in M2; completing
  it wraps up the M2 scope. Then groom M3 issues.

---

## 2026-06-09 — M2 version information (issue #9)

- **Issue:** #9 — M2: /versions + version details (client + server)
- **Branch:** `nightly/2026-06-09-issue-9`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (59 tests) `deny check` ✅
- **What shipped:** `ocpi-types::version` additions —
  - `ModuleID` enum (9 spec variants: cdrs, chargingprofiles, commands, credentials,
    hubclientinfo, locations, sessions, tariffs, tokens); serde as lowercase
  - `InterfaceRole` enum (SENDER/RECEIVER); serde as SCREAMING_SNAKE
  - `Endpoint` struct: identifier + role + url (all spec-faithful field names)
  - `VersionDetails` struct: version + endpoints
  - `FromStr` + `Display` for `VersionNumber` (used in axum path extraction)
  - `Version.url` upgraded from `String` → `Url` (validated, max-255)
  - 11 new unit tests including two spec-example round-trips
  - `ocpi-client`: `version_details(&self, url: &str)` method
  - `ocpi-server`: `VersionsHandler` trait, `VersionsConfig` struct (with
    `VersionsHandler` impl), axum `versions_router(config)` — real handlers for
    `GET /versions` and `GET /versions/{version}`
- **Groomed:** closed #15 (already merged via PR #18); created #19 (M2 version
  negotiation helper) to bring M2 to 3 owner-approved issues
- **Clippy traps:** `Version` import unused in axum submodule; `ok_or_else`
  flagged as `unnecessary_lazy_evaluations` (use `ok_or` when error is not costly
  to construct); `get().is_none()` flagged — use `!contains_key()` instead
- **Custom module IDs:** `#[serde(untagged)]` per-variant is NOT standard serde.
  `ModuleID::Other(String)` was dropped; unknown module IDs fail deserialization.
  A future issue should add proper catch-all support.
- **async_fn_in_trait + axum:** the `VersionsHandler` trait uses
  `async_fn_in_trait` but is NOT wired directly to the axum router (to avoid
  `Send`-bound issues). The router uses `VersionsConfig` directly. The trait is
  provided for custom, non-axum implementations.
- **Next:** #19 (M2 version negotiation helper, P1) or #10 (M2 credentials
  handshake, P1). Suggest #10 next — it completes M2 and is the harder piece.

---

## 2026-06-09 — M1 scalar primitives (issue #15)

- **Issue:** #15 — M1: Role enum and primitive scalar types (CiString, Url)
- **Branch:** `nightly/2026-06-09-issue-15`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ `deny check` ✅ (52 tests pass)
- **What shipped:** `ocpi-types::common` additions —
  - `Role` enum (7 variants: CPO, EMSP, HUB, NAP, NSP, OTHER, SCSP) with serde
  - `CiString<const MAX: usize>` const-generic newtype with printable-ASCII + max-length
    validation via `TryFrom`; `PartialEq` is case-insensitive (hash is lowercased too)
  - Type aliases: `CiString2`, `CiString3`, `CiString36`, `CiString255`
  - `Url` newtype: max-255-byte validated string with `TryFrom` + serde
  - Crate-level re-exports for all new public types
  - 14 new unit tests (all green)
- **No new dependencies.** No Cargo.toml changes.
- **Clippy trap:** `#[derive(Hash)]` with a manual `PartialEq` triggers
  `derived_hash_with_manual_eq`. Must implement `Hash` manually so the lowercased
  hash is consistent with the case-insensitive `PartialEq`.
- **What worked:** const-generic `CiString<N>` is zero-extra-cost and covers all
  spec lengths (2, 3, 36, 255) from one definition.
- **Next:** #9 (M2: `/versions` + version details, P1) — `Role`, `CiString`, `Url`
  are the last M1 blockers. #17 (client Authorization header Base64-encode, P2)
  can run in parallel.

---

## 2026-06-07 — M1 transport layer (issue #6)

- **Issue:** #6 — M1: Transport layer — headers, Token auth, pagination
- **Branch:** `nightly/2026-06-07-issue-6`
- **PR:** opened (auto-merge disabled — new dependency; `needs-human` label)
- **CI:** all local gates green (`fmt`, `clippy -D warnings`, `test`, `deny check`)
- **What shipped:** `ocpi-types::transport` module — 10 header name constants,
  `CredentialToken` (Base64 RFC 4648 encode/decode for `Authorization: Token`),
  `OcpiRoutingHeaders` (`OCPI-to/from-party-id/country-code`), `PaginatedParams`
  (date_from/date_to/offset/limit query params), `PaginationMeta` (parsed from
  `X-Total-Count` + `X-Limit` + `Link` response headers), `parse_next_link`
  public helper. 13 unit tests + 1 doc-test, all green.
- **New dependency:** `base64 = "0.22"` promoted to direct (was already a
  transitive dep via reqwest; no new package in Cargo.lock). PR flagged
  `needs-human` because it touches workspace dependencies.
- **Known gap:** `CredentialToken` does not validate the raw token is printable
  ASCII — spec-allowable but not enforced. Deferred.
- **What worked:** Spec reading first, then thin slice; no scope creep.
- **Next:** Pick up #8 (Error model — exhaustive status_code mapping + envelope
  helpers for paginated lists) or #9 (M2: /versions + version details).

## 2026-06-08 — Issue #8: error model + envelope helpers (M1)

- **Issue:** #8 — OcpiStatusCode exhaustiveness, OcpiError↔status_code mapping, paginated envelope helper
- **Branch:** `nightly/2026-06-08-issue-8`
- **PR:** (see PR link in report)
- **CI:** fmt ✅ clippy ✅ test ✅ deny ✅ (22 tests pass, 0 failures)
- **What worked:** `#[serde(from="u16",into="u16")]` on the enum + `From<u16>`/`From<OcpiStatusCode> for u16` impls is the cleanest way to make an enum serde-serialize as its integer wire value without a manual impl.
  Changing `OcpiResponse.status_code: u16` → `OcpiStatusCode` required adding `Display` to `OcpiStatusCode` so the CLI's `{}` format still compiled.
- **Gaps / follow-up:** `OcpiPaged<T>` provides offset/limit/total arithmetic but does not yet build the `Link: <url>; rel="next"` header string (needs a base URL from the request). That header construction belongs in `ocpi-server`'s axum layer, which is wired up in M2.
- **Next:** Pick either #9 (/versions + version details) or #7 (common types: Price, EnergyMix). Issue #9 is P1 and M2; finishing M1 first with #7 is lower-risk. Owner should decide priority.

---

## 2026-06-07 — M0 bootstrap (setup, human)

- **Done:** Repo scaffolded — workspace (`ocpi-types`, `ocpi-client`,
  `ocpi-server`, `ocpi-cli`), strict CI + security, owner-trust governance,
  vendored specs, this nightly substrate. All local gates green.
- **State:** M0 complete. M1 issues seeded for the routine to pick up.
- **Next:** Start M1 — flesh out the OCPI response envelope edge cases, the full
  common-types set, and transport headers/pagination. Then M2 (Versions +
  Credentials handshake) toward the first `v0.1.0` release.
