# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Web Audio: create/resume `AudioContext` on Sound checkbox or a key (not page load); close it on Restart
- Pixel/web status strip is two lines so INTERROGATE + LINK DOWN + help keys fit in 640px
- Demo drives sentry `link_ok` (datalink fault on SENTRY-3) so TUI LINK DOWN and the status strip are reachable
- TUI boot splash shows crate version (`v0.2.0`) instead of leftover `v0.1`
- Pixel/web POST line uses ASCII hyphen (bitmap font has no em dash)
- Web chrome: Demo checkbox stays in sync with `d`; Skip boot applies on change
- Web Demo checkbox stays checked through POST when demo is requested (`?demo=1` / Restart); unchecking it during boot cancels the pending auto-play
- Web keys no longer steal Space/arrows from checkboxes and selects; key-repeat does not dump rounds
- README: `--mute` starts muted (same as default); it does not lock SFX off
- TUI restores the terminal on draw/poll errors and panic, not only on a clean quit
- Secret-guard now applies `security/commit-denylist.txt` to tracked/staged paths

### Changed

- Web footer lists reload / Space; status shows `QUIT (Restart)` after `q`
- CI: cancel superseded PRs, timeouts, Linux-only release build, locked WASM build, CDK `tsc` (`types: ["node"]`), shared wasm cache
- Deploy: upload `pkg/` first (no parent `--delete` of WASM); do not cancel in-flight production syncs
- Dependabot ignores rodio 0.x minors and wasm-bindgen (must match CLI 0.2.100)

## [0.2.0] — 2026-08-21

### Added

- Web chrome shows the crate version (`v0.2.0`) and links to the GitHub release tag
- `ua571 --version` and `ua571-pixel --version`
- Open Graph / X `summary_large_image` card for https://ua571.danielendara.com (`web/og.png`)
- AWS hosting path for **https://ua571.danielendara.com**: CDK (`infra/`), OIDC deploy role, GitHub Actions `deploy-web.yml`
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) — open-source-safe maintainer deploy + self-host notes
- **Yellow** theme matching the original GRiD / film prop monochrome yellow
- **Natural cool-down**: barrel temperature falls when idle; R(M) spins down after firing stops
- **Fire SFX** — MG42-style burst on each expended round (native via `ua571-audio`/rodio; browser via Web Audio). Toggle with `m` / web Sound checkbox / `--mute`
- **Rounds-remaining ▶ marker** — film-accurate right triangle left of the ammo box (original `FIRE.PAS` `CHR(81H)`)

### Changed

- Toolchain pin: Rust **1.98.0** (`rust-toolchain.toml` + CI `toolchain: 1.98.0`)
- Fire SFX uses a trimmed Freesound MG42 burst (~90 ms), retriggered with each expended round
- Native audio: rodio **0.22** (`DeviceSinkBuilder` / `Player`; `SamplesBuffer` NonZero rates)
- Fire SFX is **muted by default** (`m` or the web **Sound** checkbox enables it; `sound = true` in config)
- Web chrome has a **Sound** checkbox (default off) that stays in sync with `m`; `?sound=1` deep-link
- Broader unit coverage (boot/demo/fire/config/render/web keys/CLI) and CI `--locked`; fmt/clippy only on Ubuntu
- Web footer credits the maintainer ([danielendara.com](https://danielendara.com), [github.com/danielendara/ua571](https://github.com/danielendara/ua571)); Thom’s UA571C remains named, full link stays in the README
- CLI `--theme` no longer overwrites a config-file theme unless the flag is passed (#17)
- Shared `load_native_config`, `AppState::toggle_arm`, and `Theme::{on_rgba,on_rgb_u32}` in core (#19)
- GitHub Actions pinned to commit SHAs; checkout does not persist credentials (#18)
- CloudFront: extension-less SPA rewrite via Function; missing `pkg/*` stays 404 (#20)

### Fixed

- Ignore and deny-list local AI-agent files (`.sessions/`, `.albatross/`, `agent.config.json`) (#16)
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

[Unreleased]: https://github.com/danielendara/ua571/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/danielendara/ua571/compare/f44c3bb...v0.2.0
[0.1.0]: https://github.com/danielendara/ua571/tree/f44c3bb
