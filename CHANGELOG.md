# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- AWS hosting path for **https://ua571.danielendara.com**: CDK (`infra/`), OIDC deploy role, GitHub Actions `deploy-web.yml`
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) — open-source-safe maintainer deploy + self-host notes
- **Yellow** theme matching the original GRiD / film prop monochrome yellow

### Changed

- Default theme is now **yellow** (was phosphor green) across TUI, pixel, and web
- Web chrome CSS follows the selected theme (not only the canvas)
- Relicensed from MIT to **GPL-3.0-only** at the preference of Thom Cherryhomes (UA571C author)

## [0.1.0] — 2026-07-29

### Added

- `ua571-core` — simulation state, four sentries, fire telemetry, demo mode, event log, config
- `ua571-tui` — ratatui terminal UI (`ua571` binary)
- `ua571-render` — shared GRiD-style monochrome framebuffer and scene drawing
- `ua571-pixel` — desktop window UI via minifb
- `ua571-web` — WebAssembly + HTML canvas frontend
- Static web host page under `web/`
- `scripts/build-web.sh` for wasm-bindgen packaging
- Themes: phosphor, amber, mono
- CI on Linux, macOS, and Windows (fmt, clippy, test, release build)
- License, NOTICE (film disclaimer + UA571C credit), CONTRIBUTING

### Notes

- Temperature and R(M) start at 0 and climb under fire; gauges fill bottom-up
- Clean-room Rust modernization inspired by tschak909/UA571C and the film prop UI
