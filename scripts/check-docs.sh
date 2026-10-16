#!/usr/bin/env bash
# Docs self-check: agent-facing docs must not claim CI is unavailable, advertise a missing
# `cargo verify` alias, or use the wrong boot-skip param. Scans AGENTS.md, CLAUDE.md, docs/,
# scripts/verify.sh and the verify skill; excludes only this script (and its test).
# Usage: ./scripts/check-docs.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

targets=(AGENTS.md CLAUDE.md docs scripts/verify.sh .cursor/skills/verify-ua571)
fail=0

scan() { # scan <message> <ERE>
  local hits
  hits="$(grep -rniE "$2" "${targets[@]}" 2>/dev/null || true)"
  if [ -n "$hits" ]; then
    printf '%s\n' "$hits" >&2
    echo "check-docs: $1" >&2
    fail=1
  fi
}

if [ -f .github/workflows/ci.yml ]; then
  scan "docs claim CI is unavailable but .github/workflows/ci.yml exists" \
    'ci (is|are) (down|unavailable|disabled|not running|local-only)|local-only[ ,;(-]+(ci|github)|(github )?actions (is|are) (down|unavailable|disabled|not running)|github actions (unavailable|down|disabled|billing)|actions billing'
fi
if [ ! -f .cargo/config.toml ]; then
  scan "docs advertise 'cargo verify' but no .cargo/config.toml alias exists" 'cargo[ ]+verify'
fi
scan "use ?boot=0 (web/prefs.js), not skipBoot" '[?&]skipBoot|skipBoot='

[ "$fail" -eq 0 ] && echo "check-docs: OK"
exit "$fail"
