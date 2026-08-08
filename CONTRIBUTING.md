# Contributing to ua571

Thanks for your interest. This is an unofficial fan recreation of a movie prop UI.

## Development

```bash
# Once per clone — enables pre-commit secret guard
./scripts/install-git-hooks.sh

cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p ua571-tui
cargo run -p ua571-pixel -- --no-boot

# Before committing (also runs automatically via pre-commit)
./scripts/check-secrets.sh staged
```

**Never commit** `infra/cdk.context.json`, AWS credentials, or `.env` files.  
See [docs/SECURITY_GUARDS.md](docs/SECURITY_GUARDS.md) and [AGENTS.md](AGENTS.md).

Web frontend:

```bash
./scripts/build-web.sh
python3 -m http.server 8080 --directory web
```

## Project layout

| Crate | Role |
|-------|------|
| `ua571-core` | Domain / simulation only (no UI) |
| `ua571-render` | Shared monochrome GRiD-style drawing |
| `ua571-tui` | Terminal UI |
| `ua571-pixel` | Desktop window UI |
| `ua571-web` | WASM bindings for the browser |

## Guidelines

- Keep simulation and UI separated: domain logic belongs in `ua571-core`.
- Prefer pure functions and unit tests for fire rules and navigation.
- Do **not** add real weapon-control interfaces, targeting of real people, or non-simulated hardware arming.
- Respect film IP: no official logos/assets; keep the fan-work disclaimer in the README and NOTICE.
- Credit upstream inspiration ([tschak909/UA571C](https://github.com/tschak909/UA571C)) when relevant.
- Do not commit `web/pkg/` or `target/` (build artifacts).

## Pull requests

1. Fork and branch from `main`.
2. Keep commits focused.
3. Ensure CI passes (fmt, clippy, tests on Linux/macOS/Windows).
