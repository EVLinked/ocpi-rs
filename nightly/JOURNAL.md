# Nightly Journal

Append-only log, newest first. One short entry per run: date, issue, PR, CI
result, what worked, what to try next.

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
