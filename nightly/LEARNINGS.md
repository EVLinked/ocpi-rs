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
- **JOURNAL.md / LEARNINGS.md merge conflicts are always doc-only.** When merging main into a long-running branch, these append-only files conflict because both sides added new sections. Resolution: keep ALL entries from both sides in newest-first order. Never discard either side.
- **`mergeable_state: "dirty"` blocks owner merge.** A PR with dirty state cannot be merged even manually. Fix it immediately by merging main into the branch (prefer merge commit over rebase to avoid force-push). The `msrv (continue-on-error)` failure is informational; dirty state is the real blocker.
- **`VersionFetcher` trait avoids a cyclic dep.** The credentials fetch-back (server calls /versions) would create ocpi-server → ocpi-client → ocpi-server cycle. Avoid by defining a `VersionFetcher` trait in ocpi-server and implementing it in ocpi-client. The concrete impl lives in the client; the trait lives in the server.
