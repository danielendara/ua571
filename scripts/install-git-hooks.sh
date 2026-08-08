#!/usr/bin/env bash
# Point this clone at repo-managed hooks (pre-commit secret guard).
# Run once after clone: ./scripts/install-git-hooks.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

chmod +x scripts/check-secrets.sh scripts/install-git-hooks.sh .githooks/pre-commit 2>/dev/null || true

git config core.hooksPath .githooks

# Local belt-and-suspenders: never assume cdk.context.json should be added
if [[ -f infra/cdk.context.json ]]; then
  # skip-worktree reduces accidental `git add -f` damage in some workflows
  git update-index --skip-worktree infra/cdk.context.json 2>/dev/null || true
fi

echo "Installed git hooks (core.hooksPath=.githooks)"
echo "Pre-commit will run: scripts/check-secrets.sh staged"
echo "Manual full scan:    ./scripts/check-secrets.sh all"
