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
- **axum method routing on a single path** — use `.route("/path", get(a).post(b).put(c).delete(d))` to register multiple HTTP methods on the same URL. Do NOT import `routing::{post, put, delete}` — those are for separate `.route()` calls; importing them when unused triggers `unused-imports` under `-D warnings`.
- **`tower` as dev-dep for axum tests** — `tower::ServiceExt::oneshot` requires `tower = { version = "0.5", features = ["util"] }` as an explicit dev-dep. tower 0.5.x is already in Cargo.lock as a transitive dep of axum, so no new packages are added; but the Cargo.toml change still triggers `needs-human`.
- **`OcpiResponse::success_empty()`** — add this constructor to `ocpi-types::envelope` rather than reaching for chrono in ocpi-server. Since ocpi-server has no direct chrono dep, constructing `OcpiResponse` directly would require either adding chrono or working around it; the clean solution is a `success_empty()` on the type itself.
