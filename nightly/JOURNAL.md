# Nightly Journal

Append-only log, newest first. One short entry per run: date, issue, PR, CI
result, what worked, what to try next.

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
