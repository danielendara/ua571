#!/usr/bin/env bash
# Negative tests for check-deploy-web-cache.sh (mutated deploy-web.yml copies must fail).
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WF="$ROOT/.github/workflows/deploy-web.yml"
GUARD="$ROOT/scripts/check-deploy-web-cache.sh"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

run_guard() {
  CHECK_DEPLOY_WEB_CACHE_FILE="$1" "$GUARD"
}

expect_fail() {
  local label=$1 file=$2
  if run_guard "$file" >/dev/null 2>&1; then
    echo "FAIL: guard should reject: $label" >&2
    return 1
  fi
  return 0
}

expect_pass() {
  if ! run_guard "$WF" >/dev/null 2>&1; then
    echo "FAIL: real deploy-web.yml should pass" >&2
    return 1
  fi
  return 0
}

bad=0
expect_pass || bad=1

# Wrong CACHE value (chrome would inherit immutable).
cp "$WF" "$tmp/wf-cache-immutable.yml"
sed -i 's/CACHE="public,max-age=60,must-revalidate"/CACHE="public,max-age=31536000,immutable"/' "$tmp/wf-cache-immutable.yml"
expect_fail "CACHE set to immutable max-age" "$tmp/wf-cache-immutable.yml" || bad=1

# PKG_CACHE redefined later in the step (value must not contain CACHE= substring).
cp "$WF" "$tmp/wf-pkg-redef.yml"
sed -i '/PKG_CACHE="public,max-age=31536000,immutable"/a\          PKG_CACHE="public,max-age=600"' "$tmp/wf-pkg-redef.yml"
expect_fail "PKG_CACHE defined twice" "$tmp/wf-pkg-redef.yml" || bad=1

# Renamed step — parser finds no commands.
cp "$WF" "$tmp/wf-renamed-step.yml"
sed -i 's/- name: Sync to S3/- name: Sync to S3 bucket/' "$tmp/wf-renamed-step.yml"
expect_fail "Sync step renamed" "$tmp/wf-renamed-step.yml" || bad=1

# Single-line aws s3 cp with immutable directly before aws s3 sync web/ (awk overwrite case).
cp "$WF" "$tmp/wf-oneline-cp.yml"
sed -i '/          aws s3 sync web\/ "s3:\/\/\${UA571_S3_BUCKET}\/" \\/i\          aws s3 cp web/index.html "s3://${UA571_S3_BUCKET}/index.html" --cache-control "public,max-age=31536000,immutable"' "$tmp/wf-oneline-cp.yml"
expect_fail "single-line non-pkg cp with literal immutable" "$tmp/wf-oneline-cp.yml" || bad=1

# Chrome sync uses PKG_CACHE.
cp "$WF" "$tmp/wf-chrome-pkg.yml"
sed -i '/aws s3 sync web\/ "s3:\/\/\${UA571_S3_BUCKET}\/" \\/,/--cache-control/ s/\${CACHE}/\${PKG_CACHE}/' "$tmp/wf-chrome-pkg.yml"
expect_fail "chrome sync uses PKG_CACHE" "$tmp/wf-chrome-pkg.yml" || bad=1

# Fewer than four aws s3 commands (manifest cp removed).
cp "$WF" "$tmp/wf-no-manifest.yml"
sed -i '/aws s3 cp web\/manifest.webmanifest/,/--cache-control "\${CACHE}"/d' "$tmp/wf-no-manifest.yml"
expect_fail "manifest cp deleted (only 3 s3 commands)" "$tmp/wf-no-manifest.yml" || bad=1

if [[ "$bad" -eq 0 ]]; then
  echo "check-deploy-cache.test: OK (real workflow + 6 negative cases)"
fi
exit "$bad"
