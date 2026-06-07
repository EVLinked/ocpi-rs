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

## OCPI domain

- **`status_code` is an integer in the body**, independent of the HTTP status.
  `1000` = success. Keep `OcpiStatusCode` ↔ `u16` mapping exhaustive.
- **Version strings are dotted** (`"2.2.1"`); `VersionNumber` serializes via
  serde `rename`. `/versions` negotiation selects the module set at runtime.
- **3.0 is upstream-restricted.** Do not expect to fully implement it from public
  sources; scaffold types and mark work `blocked-upstream`.
