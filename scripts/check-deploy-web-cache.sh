#!/usr/bin/env bash
# Guard: deploy-web.yml must use long-lived immutable cache only for versioned pkg/ uploads.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WF="${CHECK_DEPLOY_WEB_CACHE_FILE:-$ROOT/.github/workflows/deploy-web.yml}"

EXPECTED_CACHE='CACHE="public,max-age=60,must-revalidate"'
EXPECTED_PKG='PKG_CACHE="public,max-age=31536000,immutable"'
MIN_S3_COMMANDS=4

if [[ ! -f "$WF" ]]; then
  echo "check-deploy-web-cache: missing workflow: $WF" >&2
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
  echo "check-deploy-web-cache: could not find Sync to S3 run block" >&2
  exit 1
fi

cache_defs="$(grep -E '[[:space:]]+CACHE=' <<<"$sync_block" || true)"
pkg_defs="$(grep -E '[[:space:]]+PKG_CACHE=' <<<"$sync_block" || true)"
cache_count="$(grep -cF "$EXPECTED_CACHE" <<<"$sync_block" || true)"
pkg_count="$(grep -cF "$EXPECTED_PKG" <<<"$sync_block" || true)"
cache_line_count="$(grep -cE '[[:space:]]+CACHE=' <<<"$sync_block" || true)"
pkg_line_count="$(grep -cE '[[:space:]]+PKG_CACHE=' <<<"$sync_block" || true)"

if [[ "$cache_line_count" -ne 1 ]] || [[ "$cache_count" -ne 1 ]]; then
  echo "check-deploy-web-cache: need exactly one ${EXPECTED_CACHE} in Sync to S3 step" >&2
  exit 1
fi
if [[ "$pkg_line_count" -ne 1 ]] || [[ "$pkg_count" -ne 1 ]]; then
  echo "check-deploy-web-cache: need exactly one ${EXPECTED_PKG} in Sync to S3 step" >&2
  exit 1
fi

mapfile -t s3_cmds < <(
  awk '
    /^      - name: Sync to S3$/ { in_step=1; in_run=0; next }
    in_step && /^      - name:/ { in_step=0; in_run=0 }
    in_step && /^        run: \|/ { in_run=1; next }
    in_step && in_run && /aws s3 (sync|cp)/ {
      if ($0 ~ /\\[[:space:]]*$/) { buf=$0; next }
      print $0
      next
    }
    in_step && in_run && buf != "" {
      gsub(/^[[:space:]]+/, "", $0)
      buf=buf " " $0
      if ($0 !~ /\\[[:space:]]*$/) { print buf; buf="" }
    }
  ' "$WF"
)

if [[ "${#s3_cmds[@]}" -lt "$MIN_S3_COMMANDS" ]]; then
  echo "check-deploy-web-cache: expected at least ${MIN_S3_COMMANDS} aws s3 commands in Sync to S3, found ${#s3_cmds[@]}" >&2
  exit 1
fi

bad=0
for cmd in "${s3_cmds[@]}"; do
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
done

if [[ "$bad" -ne 0 ]]; then
  exit 1
fi
echo "check-deploy-web-cache: OK"
