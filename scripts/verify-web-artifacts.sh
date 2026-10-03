#!/usr/bin/env bash
# Verify web build outputs exist before deploy or after CI wasm build.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

test -f web/index.html
test -f web/pkg/ua571_web.js
test -f web/pkg/ua571_web_bg.wasm
test -f web/build-id.js
test -f web/og.png
