# Security guards (anti-leak)

This repository is designed so **opening it publicly does not expose AWS credentials**.  
These guards exist so **you, CI, harnesses, and AI coding sessions** cannot accidentally commit account-binding files or secrets.

## What must never be committed

| Path / class | Why |
|--------------|-----|
| `infra/cdk.context.json` | Contains **AWS account ID** + Route53 hosted zone ID after `cdk deploy` |
| `infra/cdk.out/` | Synth output; may embed account-specific ARNs |
| `.env`, credentials, `*.pem` | Secrets |
| `web/pkg/`, `target/` | Build artifacts (not secrets, but must not be force-added carelessly) |

GitHub Environment variables (`AWS_ROLE_ARN`, bucket, distribution ID) stay in **GitHub Settings**, not in git.

## Layers of protection

```
1. .gitignore              — default ignore of context + credentials patterns
2. security/*-denylist*    — explicit path + content deny lists
3. scripts/check-secrets.sh— local / CI scanner
4. .githooks/pre-commit    — blocks bad commits in this clone
5. CI job "secret-guard"   — fails the PR if denylisted files appear in the tree
6. AGENTS.md               — hard rules for AI coding agents
```

## Setup (every clone / machine)

```bash
./scripts/install-git-hooks.sh
./scripts/check-secrets.sh all
```

This sets `core.hooksPath=.githooks` for **this repository only**.

## Manual checks

```bash
# Staged files only (same as pre-commit)
./scripts/check-secrets.sh staged

# Tracked + untracked (respects gitignore)
./scripts/check-secrets.sh all

# What git is tracking
git ls-files | grep -E 'context|credential|\.env|cdk\.out' || echo "clean"
```

## If the guard fails

1. **Unstage** the file: `git restore --staged <path>`
2. Confirm it is ignored: `git check-ignore -v <path>`
3. **Never** use `git add -f` on denylisted paths
4. If already committed: remove from history before public release (`git filter-repo` / BFG) — contact yourself before force-pushing `main`

## AI / harness rules (non-negotiable)

Agents must **not**:

- `git add -f infra/cdk.context.json` or any credentials file  
- Commit AWS account IDs, access keys, or private keys  
- “Helpfully” track `cdk.out` for debugging  
- Put ARNs/keys into source files “for convenience”

Agents **may**:

- Edit CDK **templates** under `infra/lib/` (no account IDs)  
- Document deploy steps with placeholders (`ACCOUNT`, not real IDs)  
- Run `./scripts/check-secrets.sh` after staging

## Extending the denylist

- Paths → `security/commit-denylist.txt`  
- Content regexes → `security/content-deny-patterns.txt`  

CI and pre-commit pick them up automatically.
