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

# Each case copies the workflow and applies one perl edit. `perl -i` behaves the same on
# Linux and macOS; `sed -i` does not (BSD sed needs a suffix argument). The edit must change
# the file: a mutation that silently does not apply would test the unmutated workflow.
mutate() {
  local name=$1 flags=$2 script=$3
  cp "$WF" "$tmp/$name.yml"
  if ! perl "$flags" -e "$script" "$tmp/$name.yml"; then
    echo "FAIL: mutation $name errored" >&2
    return 1
  fi
  if cmp -s "$WF" "$tmp/$name.yml"; then
    echo "FAIL: mutation $name did not change the workflow" >&2
    return 1
  fi
}

# Wrong CACHE value (chrome would inherit immutable).
mutate wf-cache-immutable -pi 's/CACHE="public,max-age=60,must-revalidate"/CACHE="public,max-age=31536000,immutable"/' &&
  expect_fail "CACHE set to immutable max-age" "$tmp/wf-cache-immutable.yml" || bad=1

# PKG_CACHE redefined later in the step (value must not contain CACHE= substring).
mutate wf-pkg-redef -pi '$_ .= qq{          PKG_CACHE="public,max-age=600"\n} if /PKG_CACHE="public,max-age=31536000,immutable"/' &&
  expect_fail "PKG_CACHE defined twice" "$tmp/wf-pkg-redef.yml" || bad=1

# Renamed step — parser finds no commands.
mutate wf-renamed-step -pi 's/- name: Sync to S3/- name: Sync to S3 bucket/' &&
  expect_fail "Sync step renamed" "$tmp/wf-renamed-step.yml" || bad=1

# Single-line aws s3 cp with immutable directly before aws s3 sync web/ (awk overwrite case).
mutate wf-oneline-cp -pi '$_ = qq{          aws s3 cp web/index.html "s3://\${UA571_S3_BUCKET}/index.html" --cache-control "public,max-age=31536000,immutable"\n} . $_ if m{^          aws s3 sync web/ "s3://\$\{UA571_S3_BUCKET\}/" \\$}' &&
  expect_fail "single-line non-pkg cp with literal immutable" "$tmp/wf-oneline-cp.yml" || bad=1

# Chrome sync uses PKG_CACHE.
mutate wf-chrome-pkg -pi 's/\$\{CACHE\}/\${PKG_CACHE}/ if m{aws s3 sync web/ "s3://\$\{UA571_S3_BUCKET\}/" \\$} .. /--cache-control/' &&
  expect_fail "chrome sync uses PKG_CACHE" "$tmp/wf-chrome-pkg.yml" || bad=1

# Fewer than four aws s3 commands (manifest cp removed).
mutate wf-no-manifest -ni 'print unless m{aws s3 cp web/manifest\.webmanifest} .. /--cache-control "\$\{CACHE\}"/' &&
  expect_fail "manifest cp deleted (only 3 s3 commands)" "$tmp/wf-no-manifest.yml" || bad=1

if [[ "$bad" -eq 0 ]]; then
  echo "check-deploy-cache.test: OK (real workflow + 6 negative cases)"
fi
exit "$bad"
