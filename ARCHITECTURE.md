# Architecture

`ocpi-rs` is a Cargo workspace split by responsibility, not by version. Versions
are modelled *inside* `ocpi-types` as namespaced modules so a single client or
server can speak multiple OCPI versions after negotiation.

## Crates

```
ocpi-types   →  wire types, no I/O           (foundation; depended on by all)
ocpi-client  →  async HTTP sender (reqwest)  →  depends on ocpi-types
ocpi-server  →  receiver traits + axum       →  depends on ocpi-types
ocpi-cli     →  `ocpi` binary                →  depends on ocpi-client + ocpi-types
```

- **`ocpi-types`** is transport-agnostic. It owns the response [envelope](crates/ocpi-types/src/envelope.rs)
  (`status_code` / `status_message` / `timestamp` / `data`), the canonical
  [status codes](crates/ocpi-types/src/status.rs), [common data types](crates/ocpi-types/src/common.rs),
  and [version negotiation](crates/ocpi-types/src/version.rs). Module models live
  under `v2_1_1` / `v2_2_1` / `v2_3_0`.
- **`ocpi-client`** is the sender role. It carries the base URL + token and
  performs requests, parsing the envelope and returning typed `data`.
- **`ocpi-server`** is the receiver role. It defines handler traits (e.g.
  `CredentialsHandler`) and maps errors to OCPI status codes. The optional
  `axum` feature provides ready-made routers.
- **`ocpi-cli`** is a thin operator tool over the client + types.

## Version strategy

OCPI versions are largely additive. Rather than one crate per version, shared
primitives (envelope, status codes, common types, version negotiation) live at
the crate root, and version-specific module shapes live in `v2_x_y` modules.
Negotiation (`/versions`) selects which module set a connection uses at runtime.

## Design philosophy

These principles are non-negotiable and reviewers enforce them:

1. **Defer logic, not schema.** Ship the forward-compatible type now even if the
   behaviour is deferred. A field that will be needed soon (or an array that is
   always length 1 today but may grow) goes into the type immediately, so later
   work is a data migration, not a breaking contract change.
2. **Explicit rejection over silent drop.** When a case is unsupported, return a
   distinct OCPI `status_code` (e.g. `2002` for "not enough information",
   `3002` for an unsupported version) so the caller can self-diagnose. Never
   silently discard data and mis-route it days later.
3. **Align semantics with the spec.** Role is data a party *declares* in the
   handshake, never inferred by the receiver. A field absent from the spec
   stays unwired rather than invented.

## Error mapping

`ocpi-server::ServerError::status_code()` is the single place that maps internal
errors to wire status codes. Handlers return `Result<_, ServerError>`; the
router layer renders the envelope. Keep the mapping exhaustive.

## Testing

- Unit tests live beside the code (`#[cfg(test)]`).
- Serde round-trip tests guard wire compatibility (see `envelope.rs`, `version.rs`).
- As modules land, conformance tests validate against the official spec example
  payloads vendored under `specs/`.
