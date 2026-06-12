# Nightly Learnings

Durable, project-specific lessons. Read at the start of every run. Add an entry
when you discover something that would save the next run time or a failed CI
cycle. Keep entries short and specific. Prune contradictions.

## Toolchain & CI

- **rustfmt.toml is stable-safe on purpose.** Do not add nightly-only options
  (`imports_granularity`, `group_imports`, `wrap_comments`, `fn_args_layout`,
  etc.). Stable `cargo fmt --check` ignores them and the formatting drifts
  between contributors. Keep it to stable keys.
- **clippy.toml accepts only valid keys.** An unknown key makes clippy error out
  for the whole workspace. `missing-safety-doc` is **not** a clippy.toml key
  (it's a lint, set via attributes). Verify keys against the installed clippy.
- **`axum::Router` is already `#[must_use]`.** Do not put `#[must_use]` on a
  function returning it — clippy's `double_must_use` fails under `-D warnings`.
- **Public async traits need `#[allow(async_fn_in_trait)]`.** The
  `async_fn_in_trait` lint is warn-by-default and becomes an error under
  `-D warnings`. Add the allow (we don't need `Send` bounds for these handlers).
- **Private struct fields must be read** or rustc's dead-code lint fails the
  build under `-D warnings`. Either use the field in a real method or rethink it.
- **Use `reqwest` with `rustls-tls` + `default-features = false`** to avoid a
  system OpenSSL dependency in CI and in static builds.
- **No DB in the core.** This is a library; CI needs no Postgres service. Keep
  the server's default store in-memory and feature-gate anything heavier.

## Rust type system

- **`#[derive(Hash)]` with manual `PartialEq` is a clippy error** (`derived_hash_with_manual_eq`,
  deny-by-default under `-D warnings`). When `PartialEq` is case-insensitive, implement
  `Hash` manually too — hash `.to_ascii_lowercase()` so `a == b → hash(a) == hash(b)`.
- **Const-generic newtypes for bounded strings** — `struct Foo<const N: usize>(String)` is
  ergonomic in Rust 1.65+. Serde requires a manual impl (no derive support for const
  generics); the pattern is `Serialize → serialize_str` and `Deserialize → String::deserialize +
  TryFrom`.

## Serde patterns

- **Serialize an enum as its integer value** (not variant name): use `#[serde(from = "u16", into = "u16")]` on the enum + `impl From<u16>` (infallible) + `impl From<MyEnum> for u16`. No manual `Serialize`/`Deserialize` impl needed. Works for any `Copy`/`Clone` enum.
- **`Display` needed when changing a field type**: if downstream code formats a field with `{}`, changing that field's type (e.g., `u16` → `MyEnum`) requires `impl Display` on the new type, or the format call won't compile.
- **`Option<OcpiStatusCode>` vs plain `OcpiStatusCode`**: once you have an `Unknown(u16)` catch-all variant, `from_code` can stay `Option<Self>` for the _known_ lookup while `From<u16>` gives the infallible path. This keeps existing call sites that `expect("known code")` working unchanged.

## OCPI domain

- **`status_code` is an integer in the body**, independent of the HTTP status.
  `1000` = success. Keep `OcpiStatusCode` ↔ `u16` mapping exhaustive.
- **Version strings are dotted** (`"2.2.1"`); `VersionNumber` serializes via
  serde `rename`. `/versions` negotiation selects the module set at runtime.
- **3.0 is upstream-restricted.** Do not expect to fully implement it from public
  sources; scaffold types and mark work `blocked-upstream`.
- **`Authorization: Token <base64>`** — OCPI 2.2.1 requires Base64 (RFC 4648
  standard alphabet) encoding of the raw credentials token. 2.1.1/2.2
  implementations often skip the encoding; interop requires a config flag at the
  HTTP client layer, not in the type model.
- **Lenient header parsing: try-Base64-first is ambiguous but unavoidable.** For
  server-side compat with mixed 2.1.1/2.2/2.2.1 peers, try Base64-decode first;
  if that yields valid UTF-8, use the decoded result. Otherwise treat the raw
  tail as a plaintext token. Document the caveat: raw tokens that are
  coincidentally valid Base64 will be decoded incorrectly. Expose as two separate
  functions (`from_header_value` strict vs `from_header_value_lenient`) so call
  sites are self-documenting.
- **`base64 0.22` is already a transitive dep** (comes in via reqwest). Promoting
  it to a direct workspace dep does NOT add a new package to Cargo.lock and does
  not require a `needs-human` for the dep itself, but touching workspace
  Cargo.toml still triggers `needs-human` per the workflow rules.
- **Routing headers vs configuration modules** — `OCPI-to/from-party-id/country-code`
  headers are REQUIRED for Functional Modules (Tokens, Locations, CDRs) but MUST
  NOT be used on Configuration Modules (Credentials, Versions, Hub Client Info).
- **`Link` header format** — `<URL>; rel="next"`, comma-separated for multiple
  relations. Absent on the last page. `X-Limit` reflects the server's upper bound,
  not the count actually returned.
- **`async_fn_in_trait` + axum incompatibility** — Axum requires handler futures
  to be `Send`. `async_fn_in_trait` does not guarantee `Send` on the returned
  future. Do NOT wire an `async_fn_in_trait` trait directly to an axum generic
  handler. Use a concrete struct (e.g. `VersionsConfig`) as axum `State` instead,
  and keep the trait as a standalone interface for non-axum uses.
- **`#[serde(untagged)]` on a single variant is invalid** — Apply `untagged` to
  the whole enum or not at all. Per-variant `untagged` on a tuple variant inside
  an otherwise-tagged enum does not work as a catch-all; it compiles but
  deserialization will be wrong. Use a custom `Deserialize` impl for catch-all
  unknown string variants.
- **Clippy `unnecessary_lazy_evaluations`** — `Option::ok_or_else(|| T)` is
  flagged when `T` is cheap to construct (not a method call with side effects).
  Use `ok_or(T)` for simple enum variants.
- **Clippy `unnecessary_get_then_check`** — `map.get(k).is_none()` should be
  `!map.contains_key(k)`; `map.get(k).is_some()` should be `map.contains_key(k)`.
- **`#[derive(PartialOrd, Ord)]` on enums works correctly when variants are declared in the intended ordering.** Rust's auto-derived `Ord` assigns discriminants 0, 1, 2, … in declaration order. For `VersionNumber`, declaring variants `V2_0, V2_1_1, V2_2, V2_2_1, V2_3_0` means older < newer automatically. Verify declaration order before adding `Ord` to any domain enum.
- **Extract pure helpers for async methods that have non-trivial selection logic.** `select_version(remote, supported) -> Option<&Version>` is a synchronous pure function despite `negotiate_version` being async. Testing the selection logic directly (no HTTP mocking) makes the test suite faster and more targeted.
- **`f64` fields prevent `Eq` derivation.** Any struct containing an `f64` field (Price, EnergySource, EnvironmentalImpact, EnergyMix) can only derive `PartialEq`, not `Eq`. If downstream code or tests use `.eq()` inside a `HashMap` key or `BTreeSet`, switch those fields to a decimal representation. For now `PartialEq` is sufficient.
- **`Vec<T>` optional arrays (cardinality `*` in OCPI): use `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.** This gives clean JSON (arrays omitted when empty) while allowing missing fields to deserialize as `vec![]` without wrapping in `Option<Vec<T>>`.
- **Coordinate validation without a regex crate:** The OCPI `GeoLocation` regex (`-?[0-9]{1,2}\.[0-9]{5,7}`) can be validated with a small private helper that strips the optional sign, splits on `.`, and checks digit counts. Avoids the `regex` crate (which would be a new dep and trigger `needs-human`).
- **EnergyMix and friends live in `common.rs`, not a version-specific module.** They are shared across Locations, Tariffs, and Sessions. Place them in `ocpi-types::common` and re-export from the crate root.
- **`cargo-deny` is not pre-installed in the remote execution environment.** Skip local `deny check` and trust CI; no new dependencies in this run means deny will pass.
- **OCPI `type` field collides with the Rust keyword.** Structs like `CdrDimension` and `CdrToken` have a spec field named `type`. Rename it in Rust (e.g. `dimension_type`, `token_type`) and annotate with `#[serde(rename = "type")]` to preserve the wire name. Every CDR/Session type that embeds a `type` field needs this treatment — check before implementing.
- **Shared CDR primitives (CdrToken, AuthMethod, ChargingPeriod, CdrDimension/Type) live in `v2_2_1.rs`** — defined when Sessions types were implemented (#34). CDR types issue (#35) must re-export them from there, not redefine. Use `pub use v2_2_1::{…}` in the CDR module if it becomes a separate file.
- **`serde_json` was already in Cargo.lock as a dev-dep of `ocpi-types`.** Moving it to `[dependencies]` (regular dep) does NOT change `Cargo.lock` — the package was already locked. Re-exporting from `ocpi-types` avoids any new direct deps in `ocpi-server`/`ocpi-client`, keeping Cargo.lock unchanged.
- **Cargo.lock metadata changes when you add a new direct dep, even if the package was already transitive.** The `[[package]]` entry for the workspace member gains an extra line in the `dependencies = [...]` list. Any change to `Cargo.lock` triggers RISK=high in guardrails. Avoid new direct deps by routing through `ocpi-types` re-exports.
- **`pub use chrono::{self, DateTime, Utc};` re-exports both the module AND specific types.** Downstream crates can `use ocpi_types::chrono::TimeZone as _` for trait methods and `use ocpi_types::{DateTime, Utc}` for the common types. No direct chrono dep needed.
- **`cargo generate-lockfile` regenerates the ENTIRE lock file with latest compatible versions.** Never run it to check if Cargo.lock would change. Instead, run `cargo check --locked` — it fails if the lock file needs updating and succeeds otherwise.
- **`use super::*` in tests does NOT re-export private `use` items.** Private `use` statements are not part of the module's public API and are not imported by glob. If tests need a trait (e.g., `chrono::TimeZone`) for method calls, import it explicitly inside the test module.
- **`ocpi_types::serde_json::json!({…})` works in tests** once `serde_json` is re-exported from `ocpi-types`. No direct dep on `serde_json` needed in `ocpi-server`.
- **`OcpiResponse::success_empty()` for mutation responses** — PUT/PATCH that return `status_code=1000` with no `data` body. Since `data` has `#[serde(skip_serializing_if = "Option::is_none")]`, the JSON omits the `data` key entirely. Use this instead of `success(())` which would serialize `data: null`.
- **CDR POST `Location` header via reqwest:** `response.headers().get(reqwest::header::LOCATION)` extracts the `Location` header from a 201 response. Calling `.and_then(|v| v.to_str().ok()).map(|s| s.to_owned())` copies to an owned String before the response body is consumed by `.json()`. NLL lets you borrow headers, copy to Strings, then move the response — no lifetime clash.
- **`PaginationMeta::from_headers` takes three `Option<&str>`, not a `HeaderMap`.** Extract each header separately as an owned String first, then pass `.as_deref()`. Constructing a fallback `PaginationMeta { next_url: None, total_count: 0, limit: 50 }` works because all three fields are `pub`.
- **CDR store key is flat `id`, not composite.** Unlike Sessions (keyed by `{country_code}/{party_id}/{session_id}`), CDRs have a globally unique `id` within the CPO's system. The POST endpoint doesn't include path segments — the server constructs a URL like `{base_url}/cdrs/{id}` and returns it in the `Location` header. Pass `base_url` to `CdrsConfig::new()` at construction time.
- **`axum::routing::post` is not needed when chaining `.post()` on a `MethodRouter`.** `get(handler).post(other_handler)` uses the `MethodRouter::post` method. Importing `axum::routing::post` alongside `get` is only needed when both are called as free functions in separate `route()` calls. Clippy's `unused_import` catches this.
