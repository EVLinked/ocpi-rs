# OCPI 2.0 (legacy)

OCPI 2.0 predates the project's conversion to AsciiDoc and is not carried on a
dedicated `release-2.0-bugfixes` branch upstream, so `scripts/fetch-specs.sh`
does not vendor it automatically.

It is treated as **back-coverage** in this project (milestone **M7**): types are
added where cheap and version negotiation recognises `2.0`, but it is not a
primary target.

Upstream history: https://github.com/ocpi/ocpi
