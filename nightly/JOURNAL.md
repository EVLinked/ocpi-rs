# Nightly Journal

Append-only log, newest first. One short entry per run: date, issue, PR, CI
result, what worked, what to try next.

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

---

## 2026-06-07 — M0 bootstrap (setup, human)

- **Done:** Repo scaffolded — workspace (`ocpi-types`, `ocpi-client`,
  `ocpi-server`, `ocpi-cli`), strict CI + security, owner-trust governance,
  vendored specs, this nightly substrate. All local gates green.
- **State:** M0 complete. M1 issues seeded for the routine to pick up.
- **Next:** Start M1 — flesh out the OCPI response envelope edge cases, the full
  common-types set, and transport headers/pagination. Then M2 (Versions +
  Credentials handshake) toward the first `v0.1.0` release.
