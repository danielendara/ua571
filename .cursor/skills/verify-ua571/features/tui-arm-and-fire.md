# TUI: arm and fire

## Sub-features
Arm/safe (`a`), Fire panel (`f` opens it, `Esc` toggles), fire (`Enter`/`Space`), reload (`r`), sentry select (`1`-`4`), demo (`d`).

## How to get to it (user POV)
Run `ua571 --no-boot`; options panel shows first. Press `a`, then `f` for the firing panel, then `Enter`.

## Driving it with tmux
Launch per SKILL.md, `tmux send-keys -t $S a f`, then `Enter`, then `capture-pane -p`.
Proof: header `S1:ARMED 499rds`, panel `▶ 499`, log `SENTRY-1 FIRE — SOFT/BIO — 499 RDS`.

## Gotchas
Fire while SAFE is denied (no round used). `f` opens the Fire panel (it does not fire and does not toggle; `Esc` toggles); fire with `Enter`. Fire needs several ticks before the pane updates: sleep >=1s.
