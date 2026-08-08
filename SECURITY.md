# Security Policy

## Accidental secret commits

This repo enforces guards so AWS account bindings and credentials are not committed:

- `scripts/check-secrets.sh` + pre-commit hooks (`./scripts/install-git-hooks.sh`)
- CI job `secret-guard`
- Denylists under `security/`
- Rules for AI agents in `AGENTS.md`

Details: [docs/SECURITY_GUARDS.md](docs/SECURITY_GUARDS.md).

If you believe a secret was committed, contact the maintainer immediately and do not open a public issue with the secret value.

## Scope

**ua571** is a fan recreation of a movie prop operator console. It is **not** real weapon control software and has no network weapon interfaces.

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a vulnerability

If you find a security issue in this repository (e.g. unsafe handling in the web build, dependency problems, or anything that could affect users who build or host the project), please report it privately rather than opening a public issue.

- Prefer email to the maintainer listed on [danielendara.com](https://danielendara.com) or the GitHub account that owns this repository.
- Include steps to reproduce and affected platforms if possible.

We will acknowledge reports as soon as practical and work on a fix for supported versions.
