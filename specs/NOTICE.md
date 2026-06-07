# OCPI specification — attribution & license

The files under `specs/ocpi/<version>/` are copies of the **Open Charge Point
Interface (OCPI)** specification, sourced from the official repository:

> https://github.com/ocpi/ocpi

OCPI is published by the **EV Roaming Foundation**. The specification text,
diagrams, and PDFs remain **© EV Roaming Foundation** and are included here
**only as an implementation reference**.

**They are NOT covered by this project's MIT license.** Nothing in this
repository relicenses the OCPI specification. If you redistribute these files,
follow the upstream project's terms.

Each version directory contains a `SOURCE.txt` recording the exact upstream
branch and commit the copy was taken from. Refresh with:

```bash
scripts/fetch-specs.sh
```

| Version | Status | Source branch |
|---|---|---|
| 2.0 | legacy (markdown era) | see `2.0/README.md` |
| 2.1.1 | supported | `release-2.1.1-bugfixes` |
| 2.2 | supported | `release-2.2-bugfixes` |
| 2.2.1 | primary target | `release-2.2.1-bugfixes` |
| 2.3.0 | latest released | `release-2.3.0-bugfixes` |
| 3.0 | upstream-restricted | see `3.0/README.md` |
