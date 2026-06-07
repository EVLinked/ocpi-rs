#!/usr/bin/env bash
# guardrails.sh — repository safety checks and auto-merge risk classification.
#
#   scripts/guardrails.sh check                 # fail (exit 1) on forbidden content
#   scripts/guardrails.sh classify <base_ref>   # print RISK=low|high (always exit 0)
#
# `check` is a required CI job (it must stay green for normal diffs so the owner
# can always merge manually). `classify` is consumed by the auto-merge gate to
# decide whether a PR needs human review before it may auto-merge.
set -euo pipefail

cmd="${1:-check}"

# Paths whose modification requires human review before auto-merge.
GUARDED_REGEX='^(\.github/|deny\.toml$|LICENSE$|Cargo\.lock$|.*\.ya?ml$|scripts/)'
SIZE_CAP="${GUARDRAILS_SIZE_CAP:-800}"   # max added lines before a diff needs human review

case "$cmd" in
  check)
    rc=0
    if git grep -nE '^(<<<<<<<|>>>>>>>|=======)$' -- ':!*.md' ':!scripts/guardrails.sh' >/dev/null 2>&1; then
      echo "guardrails: merge-conflict markers present"; rc=1
    fi
    if git grep -nE '(BEGIN (RSA|OPENSSH|EC|PGP) PRIVATE KEY|AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{30,})' \
         -- ':!*.md' ':!scripts/guardrails.sh' >/dev/null 2>&1; then
      echo "guardrails: possible committed secret"; rc=1
    fi
    big="$(find . -type f -not -path './.git/*' -not -path './target/*' -size +10M 2>/dev/null || true)"
    if [ -n "$big" ]; then echo "guardrails: file larger than 10MB:"; echo "$big"; rc=1; fi
    [ "$rc" -eq 0 ] && echo "guardrails: check OK"
    exit "$rc"
    ;;
  classify)
    base="${2:-origin/main}"
    files="$(git diff --name-only "${base}...HEAD" 2>/dev/null || true)"
    added="$(git diff --numstat "${base}...HEAD" 2>/dev/null | awk '{s+=$1} END{print s+0}')"
    risk=low
    while IFS= read -r f; do
      [ -z "$f" ] && continue
      if printf '%s\n' "$f" | grep -qE "$GUARDED_REGEX"; then risk=high; fi
    done <<< "$files"
    if [ "${added:-0}" -gt "$SIZE_CAP" ]; then risk=high; fi
    echo "RISK=$risk"
    echo "ADDED_LINES=${added:-0}"
    exit 0
    ;;
  *)
    echo "usage: guardrails.sh [check|classify <base_ref>]" >&2
    exit 2
    ;;
esac
