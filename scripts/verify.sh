#!/usr/bin/env bash
# Single verify entrypoint; mirrors .github/workflows/ci.yml. CI is live and must also be green on the exact SHA.
# Fails fast. Usage: ./scripts/verify.sh
# Env: COVERAGE_FAIL_UNDER_LINES (default 65, same as CI).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

step() { printf '\n==> [%s] %s\n' "$1" "$2"; }

step 1/8 "secret guard"
./scripts/check-secrets.sh tracked
# docs self-check: docs must not claim CI is unavailable or advertise a missing cargo alias
if [ -f .github/workflows/ci.yml ]; then
  if grep -rniE 'actions is unavailable|ci is local-only' AGENTS.md CLAUDE.md scripts .cursor 2>/dev/null | grep -v 'scripts/verify.sh'; then
    echo "docs claim CI is unavailable but .github/workflows/ci.yml exists" >&2; exit 1
  fi
fi
if [ ! -f .cargo/config.toml ] && grep -rnE 'cargo verify' AGENTS.md CLAUDE.md scripts .cursor 2>/dev/null | grep -v 'scripts/verify.sh'; then
  echo "docs advertise 'cargo verify' but no .cargo/config.toml alias exists" >&2; exit 1
fi

step 2/8 "lint: cargo fmt --check"
cargo fmt --all -- --check

step 3/8 "lint: cargo clippy -D warnings"
cargo clippy --workspace --all-targets --locked -- -D warnings

step 4/8 "unit tests: cargo test"
cargo test --workspace --locked

step 5/8 "coverage gate (>= ${COVERAGE_FAIL_UNDER_LINES:-65}% lines)"
command -v cargo-llvm-cov >/dev/null || {
  echo "cargo-llvm-cov missing: cargo install cargo-llvm-cov --locked && rustup component add llvm-tools-preview" >&2
  exit 1
}
cargo llvm-cov --workspace --locked --no-report
cargo llvm-cov report --fail-under-lines "${COVERAGE_FAIL_UNDER_LINES:-65}"

step 6/8 "wasm build + artifact check"
./scripts/build-web.sh
./scripts/verify-web-artifacts.sh

step 7/8 "web chrome tests: npm --prefix web test"
npm --prefix web test

step 8/8 "infra typecheck (CDK)"
(cd infra && npm ci --no-audit --no-fund && npx tsc --noEmit)

printf '\nVERIFY PASSED\n'
