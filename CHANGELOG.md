# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **M0 bootstrap.** Cargo workspace with four crates: `ocpi-types`,
  `ocpi-client`, `ocpi-server`, `ocpi-cli`.
- OCPI response envelope, canonical status codes, foundational common data
  types, and version-negotiation primitives in `ocpi-types`.
- Async `versions()` client call and the `ocpi` CLI (`versions`, `validate`).
- Receiver-side handler traits with an optional `axum` integration.
- Strict CI (fmt, clippy `-D warnings`, tests, doctests, MSRV, coverage),
  security workflows (`cargo-deny`, `cargo-audit`, OpenSSF Scorecard,
  Dependabot), owner-trust governance, and the nightly development routine.
- Vendored OCPI specifications under `specs/`.

[Unreleased]: https://github.com/EVLinked/ocpi-rs/commits/main
