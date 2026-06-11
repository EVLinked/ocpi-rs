# Nightly Journal

Append-only log, newest first. One short entry per run: date, issue, PR, CI
result, what worked, what to try next.

---

## 2026-06-11 — M2 credentials axum router (issue #22)

- **Issue:** #22 — M2: credentials axum router — `credentials_router()` with concrete `CredentialsConfig`
- **Branch:** `nightly/2026-06-11-issue-22`
- **PR:** (opened this run)
- **CI:** `fmt` ✅ `clippy -D warnings` ✅ `test` ✅ (93 tests) `deny check` ✅
- **What shipped:**
  - `ocpi-types::envelope` — new `OcpiResponse::success_empty()` constructor (returns `status_code=1000`, no data, no timestamp dep in ocpi-server)
  - `ocpi-server` — `CredentialsConfig` struct: `own_credentials` (public), `registered: RwLock<HashMap<String, Credentials>>` (private), with `new()`, `is_registered()`, `register()`, `update()`, `delete()` methods; `Debug` impl
  - `ocpi-server::http` — `credentials_router(Arc<CredentialsConfig>) -> Router` (GET/POST/PUT/DELETE /credentials); token extraction via `CredentialToken::from_header_value()`; 401 for bad/missing/unregistered token; 405 for AlreadyRegistered/NotRegistered; 200 + OCPI envelope otherwise
  - 16 new unit + axum integration tests (8 unit, 8 http)
- **Cargo.toml change:** added `tower`, `tokio`, `serde_json` as dev-deps to `ocpi-server` (all already in Cargo.lock via axum; no new packages). PR labeled `needs-human`; no auto-merge.
- **Design choices:**
  - `CredentialsConfig` does NOT implement `CredentialsHandler` (avoids async_fn_in_trait + axum Send issue; same pattern as VersionsConfig)
  - GET /credentials requires the token to be in the registered map (Unauthorized if not; callers must POST first)
  - All success paths return `own_credentials`; DELETE returns `success_empty()` (no data)
  - The "fetch-back" step (server calls client's /versions during POST) is deferred to a follow-up issue per spec
- **Deferred:** the initial pre-shared token validation concept (who is "allowed" to POST before registration) is open; current impl accepts any unrecognized token as a fresh registration request
- **Next:** #19 (M2: version negotiation helper, P1) completes M2. After that, groom M3 issues.

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
