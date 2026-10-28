#!/usr/bin/env bash
# Guard: deploy-web.yml must use long-lived immutable cache only for versioned pkg/ uploads.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WF="$ROOT/.github/workflows/deploy-web.yml"

if ! grep -q 'PKG_CACHE="public,max-age=31536000,immutable"' "$WF"; then
  echo "check-deploy-web-cache: missing PKG_CACHE=public,max-age=31536000,immutable" >&2
  exit 1
fi

bad=0
while IFS= read -r cmd; do
  [[ -z "$cmd" ]] && continue
  cc="$(grep -oE '\-\-cache-control "[^"]+"' <<<"$cmd" || true)"
  if [[ -z "$cc" ]]; then
    echo "check-deploy-web-cache: aws s3 command missing --cache-control: $cmd" >&2
    bad=1
    continue
  fi
  is_pkg=0
  if [[ "$cmd" == *'web/pkg/'* ]] || [[ "$cmd" == *'/pkg/'* ]]; then
    is_pkg=1
  fi
  if [[ "$is_pkg" -eq 1 ]]; then
    if [[ "$cc" != *'${PKG_CACHE}'* ]]; then
      echo "check-deploy-web-cache: pkg/ upload must use \${PKG_CACHE}: $cc" >&2
      bad=1
    fi
  else
    if [[ "$cc" == *'PKG_CACHE'* ]] || [[ "$cc" == *'immutable'* ]]; then
      echo "check-deploy-web-cache: non-pkg upload must not use PKG_CACHE/immutable: $cmd" >&2
      bad=1
    fi
    if [[ "$cc" != *'${CACHE}'* ]]; then
      echo "check-deploy-web-cache: non-pkg upload must use \${CACHE}: $cc" >&2
      bad=1
    fi
  fi
done < <(
  awk '
    /^      - name: Sync to S3/ { in_sync=1; next }
    in_sync && /^      - name:/ { in_sync=0 }
    in_sync && /aws s3 (sync|cp)/ { buf=$0; next }
    in_sync && buf != "" {
      gsub(/^[[:space:]]+/, "", $0)
      buf=buf " " $0
      if ($0 !~ /\\[[:space:]]*$/) { print buf; buf="" }
    }
  ' "$WF"
)

if [[ "$bad" -ne 0 ]]; then
  exit 1
fi
echo "check-deploy-web-cache: OK"
