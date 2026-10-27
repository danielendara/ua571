---
name: verify-ua571
description: Launch and drive the UA 571-C sentry console (ratatui TUI via tmux; WASM web console via static server + headless Chrome) and capture evidence. Use to prove a behavior change in the real app, or before opening a PR.
---

# Verify ua571

Surfaces: **TUI** (primary, `ua571` binary), **web** (WASM canvas), **pixel** (minifb desktop window; not drivable headless, skip). No server/DB/auth/seed data: state is in-memory, fresh each launch.

Full gate first: `./scripts/verify.sh` (see AGENTS.md). This skill is for driving the real app.

Evidence dir (outside the repo; never commit evidence). Default: **`$EVIDENCE=/tmp/ua571-evidence`** — override as needed.

## Launch

TUI (build once, each drive in its own tmux session; isolated config/data via temp HOME):

```bash
EVIDENCE="${EVIDENCE:-/tmp/ua571-evidence}"; mkdir -p "$EVIDENCE"
cargo build -p ua571-tui --release --locked        # binary: target/release/ua571
CFG=$(mktemp -d); S=ua571-verify-$$
tmux new-session -d -s $S -x 100 -y 30 \
  "HOME=$CFG XDG_CONFIG_HOME=$CFG XDG_DATA_HOME=$CFG target/release/ua571 --no-boot --mute --no-save-session"
```
Ready: `tmux capture-pane -t $S -p` shows `SYSTEM BOOT — UA 571-C CONSOLE ONLINE` in EVENT LOG. `--no-save-session` + temp HOME keep the user's saved session/config untouched.

Web:

```bash
./scripts/build-web.sh        # needs rustup, wasm32 target, wasm-bindgen-cli 0.2.100; writes gitignored web/pkg, web/build-id.js
setsid python3 -m http.server 18571 --bind 127.0.0.1 --directory web >/dev/null 2>&1 & SP=$!; sleep 1; PGID=$(ps -o pgid= -p $SP | tr -d ' ')
```
Skip the boot animation in the web console with `?boot=0` (e.g. `http://127.0.0.1:18571/?boot=0`; parsed in `web/prefs.js`). Ready: `curl -sI http://127.0.0.1:18571/pkg/ua571_web_bg.wasm` returns 200. Pick another port if 18571 is busy.

## Doctor

- TUI: `test -x target/release/ua571 && target/release/ua571 --version`; `tmux ls` shows only sessions you started.
- Web: `./scripts/verify-web-artifacts.sh` (exit 0) and the curl above; `ss -ltnp | grep 18571` must be your server (pid = `$SP`).

## Drive

TUI keys: `a` arm/safe, `f` toggle Fire/Options panel, `Enter`/`Space` fire (needs ARMED, firing panel), `r` reload, `1`-`4` sentry, `←→` section, `↑↓` select, `d` demo, `t` theme, `q` quit.

```bash
tmux send-keys -t $S a f; sleep 0.5; tmux send-keys -t $S Enter; sleep 1.5
tmux capture-pane -t $S -p > "$EVIDENCE/tui-2-armed-fired.txt"
```
Web (no Playwright in repo; Chrome is at `google-chrome`):
```bash
google-chrome --headless=new --no-sandbox --disable-gpu --window-size=1280,900 \
  --virtual-time-budget=8000 --screenshot="$EVIDENCE/web-console.png" http://127.0.0.1:18571/
google-chrome --headless=new --no-sandbox --disable-gpu --window-size=1280,900 \
  --virtual-time-budget=8000 --screenshot="$EVIDENCE/web-console-boot0.png" "http://127.0.0.1:18571/?boot=0"
```
The canvas is `#ua571` (aria-label "UA 571-C display"); controls (the `#skipBoot` checkbox is the "Skip boot" toggle; the URL param is `?boot=0`) `#theme #scale #demo #sound #skipBoot #restart #touchPadToggle #fullscreenToggle`; touch pad buttons `[data-pad-action=fire|arm|reload|panel|up|down|left|right]`. For key interaction beyond a static screenshot use CDP/Playwright against the same URL (not installed in-repo; do not add it as a dependency).

## Evidence

Capture the action and resulting state, not just the end screen: pane text before/after (TUI), screenshot + curl headers (web). Prove side effects visible in-app: e.g. arming logs `SENTRY-1 WEAPON ARMED`, one Enter fire drops `500` -> `499 rds` and logs `SENTRY-1 FIRE`. Use real keys, not internal setters.

## Isolation

TUI instances are isolated by tmux session + temp HOME and can run side by side. Web: one static server per port. Never drive the user's own running instance or default config dir.

## Cleanup

```bash
tmux kill-session -t $S 2>/dev/null; rm -rf "$CFG"
kill -- -$PGID 2>/dev/null; sleep 1
! ss -ltn | grep -q ':18571 ' && ! curl -sf -m 2 http://127.0.0.1:18571/ >/dev/null && echo "port 18571 free" || { echo "CLEANUP FAILED: port 18571 still in use" >&2; false; }
```
Kill only what you started (tmux session name / saved process group), never by process name. The last line must print `port 18571 free` (nothing listening, health check fails); otherwise cleanup failed. Evidence stays.

## Helpers

None; the commands above are the recipe.
