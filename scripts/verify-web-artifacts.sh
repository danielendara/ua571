#!/usr/bin/env bash
# Verify web build outputs exist before deploy or after CI wasm build.
# Site-root ship list is parsed from .github/workflows/deploy-web.yml (Sync to S3)
# so emergency docs, CI, and deploy cannot drift on hand-maintained includes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
WF="${VERIFY_WEB_ARTIFACTS_WORKFLOW:-$ROOT/.github/workflows/deploy-web.yml}"

if [[ ! -f "$WF" ]]; then
  echo "verify-web-artifacts: missing workflow: $WF" >&2
  exit 1
fi

sync_block="$(
  awk '
    /^      - name: Sync to S3$/ { in_step=1; in_run=0; next }
    in_step && /^      - name:/ { in_step=0; in_run=0 }
    in_step && /^        run: \|/ { in_run=1; next }
    in_step && in_run { print }
  ' "$WF"
)"

if [[ -z "$sync_block" ]]; then
  echo "verify-web-artifacts: could not find Sync to S3 run block in $WF" >&2
  exit 1
fi

require_web_root_include() {
  local pattern="$1"
  if [[ "$pattern" == *'*'* ]]; then
    shopt -s nullglob
    local matches=(web/"$pattern")
    shopt -u nullglob
    if [[ ${#matches[@]} -eq 0 ]]; then
      echo "verify-web-artifacts: no files match web/$pattern (deploy-web.yml --include)" >&2
      exit 1
    fi
  elif [[ ! -f "web/$pattern" ]]; then
    echo "verify-web-artifacts: missing web/$pattern (deploy-web.yml --include)" >&2
    exit 1
  fi
}

# Versioned WASM bundle (pkg/ sync in deploy-web.yml).
test -f web/pkg/ua571_web.js
test -f web/pkg/ua571_web_bg.wasm

# Site chrome: each --include on the root `aws s3 sync web/` command.
in_site=0
while IFS= read -r line; do
  if [[ "$line" == *'aws s3 sync web/'* ]] && [[ "$line" != *'web/pkg/'* ]]; then
    in_site=1
    continue
  fi
  if [[ "$in_site" -eq 1 ]] && [[ "$line" == *'aws s3 '* ]] && [[ "$line" != *'aws s3 sync web/'* ]]; then
    break
  fi
  if [[ "$in_site" -eq 1 ]] && [[ "$line" == *'--include'* ]]; then
    inc="$(sed -n 's/.*--include "\([^"]*\)".*/\1/p' <<<"$line")"
    if [[ -z "$inc" ]]; then
      echo "verify-web-artifacts: could not parse --include in: $line" >&2
      exit 1
    fi
    require_web_root_include "$inc"
  fi
done <<<"$sync_block"

if ! grep -q 'manifest\.webmanifest' <<<"$sync_block"; then
  echo "verify-web-artifacts: deploy workflow must upload manifest.webmanifest" >&2
  exit 1
fi
test -f web/manifest.webmanifest

echo "verify-web-artifacts: OK"