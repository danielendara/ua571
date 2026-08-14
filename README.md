# ua571

**UA 571-C Remote Sentry Weapon System** — a modern Rust recreation of the Colonial Marines operator console from *Aliens* (1986).

Unofficial fan project. **Not affiliated with** 20th Century Studios, Disney, Fox, James Cameron, or any *Alien* franchise rights holders.

[![CI](https://github.com/danielendara/ua571/actions/workflows/ci.yml/badge.svg)](https://github.com/danielendara/ua571/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

**Live demo:** [https://ua571.danielendara.com](https://ua571.danielendara.com)  
*(static WASM build on AWS; see [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md))*

Three frontends share one simulation core:

| Frontend | How to run | Feel |
|----------|------------|------|
| **TUI** ([ratatui](https://ratatui.rs)) | `cargo run -p ua571-tui --release` | Terminal operator console |
| **Pixel** (GRiD-faithful) | `cargo run -p ua571-pixel --release` | Desktop 640×240 monochrome canvas |
| **Web** (WASM) | `./scripts/build-web.sh` then serve `web/` | Same canvas in the browser |

```
┌ UA 571-C  REMOTE SENTRY WEAPON SYSTEM ─────────────────────────┐
│ S1 S2 S3 S4 │ OPTIONS │ phosphor │ DEMO off                      │
│ SYSTEM MODE │ WEAPON │ IFF STATUS │ TEST │ TARGET │ SPECTRAL │ … │
│ AUTO-REMOTE │ SAFE   │ SEARCH     │ AUTO │ SOFT   │ BIO      │ … │
│ …                                                                │
│ log: SENTRY-1 WEAPON ARMED                                       │
│ ←→ section  ↑↓ select  f fire  1-4 sentry  d demo  q quit        │
└──────────────────────────────────────────────────────────────────┘
```

## Features

- **Options panel** — system mode, weapon status, IFF, test routine, target / spectral / select profiles
- **Firing panel** — rounds remaining, time-at-100%, temperature & R(M) gauges (bottom-up), CRITICAL alert
- **Four sentries** — independent ammo and configuration (film setup)
- **Event log** — arming, fire, critical, demo messages
- **Demo mode** — scripted perimeter defense auto-play
- **Themes** — **yellow** (default, film/GRiD prop), phosphor green, amber, mono
- **Fire SFX** — MG42-inspired high-rate mechanical pulse when a round fires (mute with `m` or `--mute`)
- **Cross-platform** — macOS, Linux, Windows; browser via WebAssembly

## Quick start

**Requirements:** Rust stable (1.80+ recommended), a terminal (TUI) or windowed desktop (pixel).

```bash
git clone https://github.com/danielendara/ua571.git
cd ua571

# Terminal UI
cargo run -p ua571-tui --release

# Desktop pixel UI (closest to the GRiD display layout)
cargo run -p ua571-pixel --release -- --no-boot
cargo run -p ua571-pixel --release -- -s 3 --demo

# WebAssembly (local static server)
./scripts/build-web.sh
python3 -m http.server 8080 --directory web
# open http://localhost:8080
```

Requires `wasm32-unknown-unknown` (script installs the target) and `wasm-bindgen-cli`.

### Production web host

**https://ua571.danielendara.com** — S3 + CloudFront + Route53.

| Who | How |
|-----|-----|
| Anyone | Self-host `web/` (+ `pkg/`) on any static HTTPS host |
| Maintainers | GitHub Actions OIDC deploy on `main` — [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) |
| Infra as code | [`infra/`](infra/) (AWS CDK) |

Deploy is written for a **public repo**: forks/PRs cannot assume the AWS role. The GitHub repo may stay private until you’re ready.

### CLI flags

```
ua571 / ua571-pixel [OPTIONS]

  -t, --theme <THEME>       yellow | phosphor | amber | mono  [default: yellow]
  -r, --rounds <N>          starting rounds per sentry  [default: 500]
      --tick-ms <MS>        UI tick interval  [default: 80]
      --no-boot             skip POST splash
      --demo                start demo after boot
      --mute                disable fire SFX
  -c, --config <PATH>       load TOML config
  -s, --scale <N>           (pixel only) integer scale 1–6  [default: 2]
```

### Config file (optional)

`~/.config/ua571/config.toml` (or platform equivalent via [`dirs`](https://crates.io/crates/dirs)):

```toml
theme = "yellow"
tick_ms = 80
starting_rounds = 500
show_boot = true
demo_on_start = false
log_capacity = 64
sound = true
```

## Keys

| Key | Action |
|-----|--------|
| `←` `→` / `h` `l` | Previous / next options section |
| `↑` `↓` / `k` `j` | Change selection in section |
| `f` | Firing panel |
| `o` / `Esc` | Options panel |
| `Enter` / `Space` | Fire (firing panel; requires ARMED) or open fire from options |
| `1`–`4` | Select sentry |
| `a` | Toggle SAFE / ARMED on active sentry |
| `r` | Reload active sentry drum |
| `d` | Toggle demo auto-play |
| `m` | Toggle fire SFX mute |
| `t` | Cycle theme (TUI) |
| `q` | Quit |
| any (on boot) | Skip POST |

## Architecture

```
crates/
  ua571-core/     pure domain + simulation (no UI)
  ua571-render/   shared GRiD monochrome framebuffer + scene draw
  ua571-tui/      ratatui → binary `ua571`
  ua571-pixel/    minifb window → binary `ua571-pixel`
  ua571-web/      wasm-bindgen + canvas → web/pkg (build artifact)
web/              static HTML/CSS/JS host page
infra/            AWS CDK (S3, CloudFront, ACM, Route53, OIDC role)
scripts/          build-web.sh
docs/             DEPLOYMENT.md
```

Pixel and Web share `ua571-render`. The TUI is a separate character-cell view of the same `ua571-core` state.

Deploy `web/` **including** `web/pkg/` (produced by `build-web.sh`) to any static host (GitHub Pages, Cloudflare Pages, Netlify, S3, …).

## Inspiration & attribution

This project was informed by [tschak909/UA571C](https://github.com/tschak909/UA571C) (GPL-3.0) by **Thom Cherryhomes** — a GRiD-OS Pascal recreation of the film displays, written while restoring GRiD development tools.

**ua571** is a **clean-room modernization** in Rust: new source and architecture. It does **not** ship GRiD-OS binaries, compilers, or disk images.

See [NOTICE](NOTICE) for the full film disclaimer and upstream credit.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[GPL-3.0](LICENSE) — same family as Thom’s UA571C, at his preference.

Copyright (c) 2026 Daniel Endara. See [NOTICE](NOTICE) for film disclaimer and upstream credit.
