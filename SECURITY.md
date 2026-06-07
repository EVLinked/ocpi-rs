# Security Policy

## Supported versions

`ocpi-rs` is pre-1.0 and under active development. Security fixes target the
latest `main` and the most recent tagged release.

| Version | Supported |
|---|---|
| latest `main` / newest release | ✅ |
| older tags | ❌ |

## Reporting a vulnerability

**Do not open a public issue for security reports.**

Use GitHub's private vulnerability reporting:
**Security → Report a vulnerability** on
<https://github.com/EVLinked/ocpi-rs/security/advisories/new>.

Please include: affected crate/version, a description, reproduction steps or a
proof of concept, and the impact. We aim to acknowledge within 72 hours and to
ship a fix or mitigation as fast as severity warrants.

## What we do to stay secure

- `#![forbid(unsafe_code)]` in the type layer.
- `cargo deny` (advisories, licenses, bans, sources) and `cargo audit` (RustSec)
  run on every PR and on a weekly schedule.
- Dependabot keeps Cargo and GitHub Actions dependencies current.
- OpenSSF Scorecard tracks repository security posture.
- GitHub Actions are pinned and run with least-privilege `permissions:`.
- Secret scanning with push protection is enabled.
- Branch protection requires green security checks before any merge.

## Threat model notes for integrators

OCPI parties exchange tokens and trust declared roles. When you build on
`ocpi-rs`, treat all inbound payloads as untrusted: validate at the boundary,
authenticate every request (`Authorization: Token …`), and never infer a peer's
role — read it from the negotiated credentials.
