# Web: console loads

## Sub-features
Theme/scale selects, Demo/Sound/Skip boot, Touch pad, Fullscreen, Restart.

## How to get to it (user POV)
Open the served `web/` directory in a browser; POST/boot screen shows "PRESS ANY KEY".

## Driving it with headless Chrome
Build + serve per SKILL.md and screenshot. Proof: header shows `v0.2.0`, canvas renders the `UA 571-C` boot screen, `pkg/ua571_web_bg.wasm` returns 200.

## Gotchas
Needs `scripts/build-web.sh` first (web/pkg is gitignored). Headless CLI cannot send keys; use CDP/Playwright for interaction. `?skipBoot=1` did not skip boot in the run observed; use the Skip boot checkbox.
