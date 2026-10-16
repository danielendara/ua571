#!/usr/bin/env bash
# Negative test for check-docs.sh: each originally flagged phrase, inserted into each scanned
# file of a temp copy of the tree, must make check-docs fail; the clean tree must pass.
# Usage: ./scripts/check-docs.test.sh
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

phrases=(
  "GitHub Actions is unavailable"
  "Verification (local-only; GitHub Actions is unavailable)"
  "CI is local-only"
  "(alias: cargo verify)"
  "cargo verify"
  "GitHub Actions is down"
  "github actions are down"
  "CI is down"
  "CI is unavailable"
  "CI is disabled"
  "CI is not running"
  "GitHub Actions billing"
  "local-only CI"
  "?skipBoot=1 did not skip boot"
)
files=(AGENTS.md docs/DEPLOYMENT.md scripts/verify.sh .cursor/skills/verify-ua571/SKILL.md)

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
copy() { rm -rf "$tmp/t"; mkdir "$tmp/t"; (cd "$ROOT" && cp -r AGENTS.md CLAUDE.md docs scripts .cursor .github "$tmp/t/"); }

bad=0
copy
if ! "$tmp/t/scripts/check-docs.sh" >/dev/null 2>&1; then
  echo "FAIL: clean tree should pass" >&2; bad=1
fi
for f in "${files[@]}"; do
  for p in "${phrases[@]}"; do
    copy
    printf '\n%s\n' "$p" >> "$tmp/t/$f"
    if "$tmp/t/scripts/check-docs.sh" >/dev/null 2>&1; then
      echo "FAIL: not caught: '$p' in $f" >&2; bad=1
    fi
  done
done
[ "$bad" -eq 0 ] && echo "check-docs.test: OK (${#files[@]} files x ${#phrases[@]} phrases + clean tree)"
exit "$bad"
