/**
 * Boots the UA 571-C WASM module against the page canvas.
 * Expects `./pkg/ua571_web.js` from `scripts/build-web.sh`.
 * Chrome helpers are exported for Node tests (no WASM).
 */

let raf = 0;
let app = null;
let onKey = null;
let onKeyUp = null;
let onVisibility = null;
/** Bumped on every boot. A completion whose id no longer matches must not publish. */
let bootGeneration = 0;
/**
 * Node tests substitute the WASM import. Production leaves this null.
 * The loader resolves to the `Ua571Web` instance that boot created.
 * @type {null | ((opts: object) => Promise<object>)}
 */
let bootInstanceLoader = null;
/** Last PREFS snapshot syncPrefs saw; reseeded (no write) on every (re)boot. */
let lastPrefs = {};
/** The user's Touch pad show/hide choice; undefined until they toggle it. */
let touchPadChoice;
/** One-shot status after T cycles theme (cleared on the next game key). */
let pendingThemeHint = null;
/** On-screen pad hold-to-fire controller; stopped on teardown and when hidden. */
const fireHold = createFireHold(() => app);

function readOptions() {
  // System mode / weapon status / IFF have no HTML chrome element (they're
  // in-game, changed via key presses on the Options screen) — prefer the
  // *live* running app's current values when rebooting (e.g. a theme
  // change) so an in-progress selection survives, falling back to stored
  // prefs only on a genuinely fresh boot (no `app` yet).
  const stored = readStoredPrefs(
    pageStorage()
  );
  return {
    theme: document.getElementById("theme").value,
    scale: Number(document.getElementById("scale").value) || 3,
    demo: document.getElementById("demo").checked,
    skipBoot: document.getElementById("skipBoot").checked,
    sound: document.getElementById("sound").checked,
    systemMode: (app && app.system_mode) || stored.systemMode || "",
    weaponStatus: (app && app.weapon_status) || stored.weaponStatus || "",
    iffStatus: (app && app.iff_status) || stored.iffStatus || "",
  };
}

function teardown() {
  fireHold.stop();
  if (raf) cancelAnimationFrame(raf);
  raf = 0;
  if (onKey) {
    window.removeEventListener("keydown", onKey);
    onKey = null;
  }
  if (onKeyUp) {
    window.removeEventListener("keyup", onKeyUp);
    onKeyUp = null;
  }
  if (onVisibility) {
    document.removeEventListener("visibilitychange", onVisibility);
    onVisibility = null;
  }
  // Drop WASM app so a new theme/scale re-instantiates cleanly.
  if (app && typeof app.free === "function") {
    try {
      app.free();
    } catch (_) {
      /* ignore */
    }
  }
  app = null;
}

/** Browser-chrome colour per theme; matches each theme's `--fg` in style.css. */
export const THEME_COLORS = Object.freeze({
  yellow: "#ffee00",
  phosphor: "#50fa7b",
  amber: "#ffb000",
  mono: "#e0e0e0",
});

export function themeColorFor(theme) {
  return THEME_COLORS[String(theme || "").toLowerCase()] || THEME_COLORS.yellow;
}

function applyPageTheme(theme) {
  if (typeof document === "undefined" || !document.body) return;
  document.body.dataset.theme = theme || "yellow";
  const meta = document.querySelector('meta[name="theme-color"]');
  const color = themeColorFor(theme);
  if (meta && meta.getAttribute("content") !== color) meta.setAttribute("content", color);
}

/** Switch theme on the live WASM app; false means the caller must reboot. */
export function applyThemeLive(app, theme) {
  if (!app || typeof app.set_theme !== "function") return false;
  app.set_theme(theme);
  return true;
}

function showVersion(v) {
  const wrap = document.getElementById("app-version-wrap");
  const el = document.getElementById("app-version");
  if (!wrap || !el || !v) return;
  el.textContent = `v${v}`;
  el.href = `https://github.com/danielendara/ua571/releases/tag/v${v}`;
  wrap.hidden = false;
}

import {
  PREFS_STORAGE_KEY,
  pageStorage,
  readStoredPrefs,
  writeStoredPrefs,
  prefsFromSearch,
  searchFromPrefs,
  syncShareUrl,
  selectHasValue,
  applyPrefsToElements,
  hydrateChromePrefs,
  snapshotPrefs,
  syncPrefs,
} from "./prefs.js";

// Re-exported so existing importers of main.js keep working.
export {
  PREFS_STORAGE_KEY,
  pageStorage,
  readStoredPrefs,
  writeStoredPrefs,
  prefsFromSearch,
  searchFromPrefs,
  syncShareUrl,
  selectHasValue,
  applyPrefsToElements,
  hydrateChromePrefs,
  snapshotPrefs,
  syncPrefs,
};
export function isChromeTarget(el) {
  return Boolean(
    el &&
      el.closest &&
      el.closest("input, select, button, a, label, textarea")
  );
}

/** Browser/OS shortcuts (Ctrl/Cmd/Alt + key) belong to the browser, not the console. */
export function hasCommandModifier(e) {
  return Boolean(e && (e.ctrlKey || e.metaKey || e.altKey));
}

/**
 * After `boot()` finishes, move focus to the canvas unless a chrome control
 * (e.g. a Theme/Scale select stepped with the keyboard) already has it.
 */
export function shouldRefocusAfterBoot(activeElement) {
  return !isChromeTarget(activeElement);
}

/** Theme order matches `Theme::ALL` / the #theme `<option>` list. */
export function themeValuesFromSelect(select) {
  if (!select || !select.options) return [];
  return Array.from(select.options, (o) => o.value);
}

export function nextThemeValue(current, values) {
  if (!values || !values.length) return current;
  const i = values.indexOf(current);
  const next = i >= 0 ? (i + 1) % values.length : 0;
  return values[next];
}

export function themeStatusHint(theme) {
  switch (String(theme || "").toLowerCase()) {
    case "yellow":
      return "THEME YELLOW";
    case "phosphor":
      return "THEME PHOSPHOR";
    case "amber":
      return "THEME AMBER";
    case "mono":
      return "THEME MONO";
    default:
      return `THEME ${String(theme || "").toUpperCase()}`;
  }
}

/** Advance `#theme` to the next option; returns the new value. */
export function cycleThemeSelect(select) {
  const values = themeValuesFromSelect(select);
  const next = nextThemeValue(select.value, values);
  select.value = next;
  return next;
}

/**
 * T key: cycle theme, `sync` prefs, repaint page chrome, show hint.
 * Returns `{ theme, hint }` when handled, else `false`.
 */
export function handleThemeKeyDown(
  e,
  { select, status, sync }
) {
  if (!e || e.code !== "KeyT" || e.repeat) return false;
  if (hasCommandModifier(e)) return false;
  if (isChromeTarget(e.target)) return false;
  if (!select) return false;

  const theme = cycleThemeSelect(select);
  if (sync) sync();
  applyPageTheme(theme);
  const hint = themeStatusHint(theme);
  pendingThemeHint = hint;
  writeLiveRegion(status, hint);
  return { theme, hint };
}

/** Return keyboard focus to the play canvas. */
export function refocusPlaySurface(canvas) {
  if (!canvas || typeof canvas.focus !== "function") return false;
  canvas.focus();
  return true;
}

/** Skip-link click: prevent hash-only jump and focus the canvas. */
export function handleSkipToPlaySurface(event, canvas) {
  if (event && typeof event.preventDefault === "function") {
    event.preventDefault();
  }
  return refocusPlaySurface(canvas);
}

/** Keys that change a closed `<select>` value (Windows/Linux fire `change` per key). */
const SELECT_VALUE_KEYS = new Set([
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

function isSelectValueKey(e) {
  if (SELECT_VALUE_KEYS.has(e.key)) return true;
  // Type-ahead: a single printable character.
  return (
    typeof e.key === "string" &&
    e.key.length === 1 &&
    !e.ctrlKey &&
    !e.metaKey &&
    !e.altKey
  );
}

/**
 * After chrome `change` (or a button click), put focus back on the canvas
 * so keyboard play is not trapped in the header controls. A `<select>` being
 * stepped with the keyboard keeps focus (WCAG 3.2.2) until Enter or Escape;
 * Tab leaves natively.
 */
export function bindPlaySurfaceRefocus(chromeRoot, canvas) {
  if (!chromeRoot || !canvas) return () => {};
  let keyboardSelect = null;
  const onKeydown = (e) => {
    const t = e && e.target;
    if (!t || t.tagName !== "SELECT") return;
    if (e.key === "Enter" || e.key === "Escape") {
      keyboardSelect = null;
      refocusPlaySurface(canvas);
    } else if (isSelectValueKey(e)) {
      keyboardSelect = t;
    }
  };
  const onChange = (e) => {
    const t = e && e.target;
    if (keyboardSelect && t === keyboardSelect) {
      keyboardSelect = null;
      return;
    }
    refocusPlaySurface(canvas);
  };
  const onClick = (e) => {
    // A pointer interaction ends any keyboard-stepping state.
    keyboardSelect = null;
    const t = e && e.target;
    if (t && t.closest && t.closest("button")) refocusPlaySurface(canvas);
  };
  chromeRoot.addEventListener("keydown", onKeydown);
  chromeRoot.addEventListener("change", onChange);
  chromeRoot.addEventListener("click", onClick);
  return () => {
    chromeRoot.removeEventListener("keydown", onKeydown);
    chromeRoot.removeEventListener("change", onChange);
    chromeRoot.removeEventListener("click", onClick);
  };
}

/** Write `next` only when the visible text changes (avoids live-region chatter). */
export function writeLiveRegion(el, next) {
  if (!el || el.textContent === next) return false;
  el.textContent = next;
  return true;
}

/** Status text for screen readers: drops the per-round count so firing is not announced. */
export function announcementFromStatus(text) {
  return String(text).replace(/ · \d+ rds/, "");
}

export function formatChromeStatus(app) {
  const screen = app.screen_name().toUpperCase();
  const quit = app.should_quit ? " · QUIT (Restart)" : "";
  return `${screen} · ${app.status_line()}${quit}`;
}

/** One animation-frame of chrome: checkboxes + #status live region. */
export function syncChromeFromApp(app, els) {
  if (!app) return false;
  if (typeof app.frame === "function") app.frame();
  if (els.sound) els.sound.checked = app.sound_enabled;
  if (els.demo) els.demo.checked = app.demo_active;
  const next = pendingThemeHint || formatChromeStatus(app);
  writeLiveRegion(els.announce, announcementFromStatus(next));
  return writeLiveRegion(els.status, next);
}

/**
 * Pause the rAF loop and mute WASM audio while the document is hidden.
 * Does not reset sim state (rounds, screen, sound preference).
 */
export function applyDocumentVisibility(hidden, app, loopCtl) {
  if (hidden) {
    if (loopCtl && typeof loopCtl.pause === "function") loopCtl.pause();
    if (app && typeof app.set_hidden === "function") app.set_hidden(true);
    return { paused: true };
  }
  if (app && typeof app.set_hidden === "function") app.set_hidden(false);
  if (loopCtl && typeof loopCtl.resume === "function") loopCtl.resume();
  return { paused: false };
}

/** Background tabs must not advance the sim. */
export function shouldAdvanceFrame(hidden) {
  return !hidden;
}

/** Player-facing WASM load failure. Local dev also gets the build-script hint. */
export function wasmLoadFailureStatus(hostname) {
  const player = "Console did not load. Use Restart to try again.";
  if (hostname === "localhost" || hostname === "127.0.0.1") {
    return `${player} Run: ./scripts/build-web.sh  then serve the web/ folder.`;
  }
  return player;
}

/** @internal Tests inject a stand-in for `import("./pkg/ua571_web.js")`. Pass null to restore. */
export function setBootInstanceLoaderForTests(loader) {
  bootInstanceLoader = loader;
}

function freeWasmInstance(instance) {
  if (instance && typeof instance.free === "function") {
    try {
      instance.free();
    } catch (_) {
      /* ignore */
    }
  }
}

/**
 * Sound checkbox. A failed unlock (no context, or resume() rejected) turns
 * sound back off and says it did not start. Hidden-tab mute is unchanged.
 */
export async function applySoundChoice(app, on, els) {
  if (!app || typeof app.set_sound !== "function") return { on: false, started: false };
  app.set_sound(Boolean(on));
  let started = false;
  if (on) {
    try {
      started = (await app.unlock_audio()) === true;
    } catch (_) {
      started = false;
    }
    if (!started) {
      app.set_sound(false);
      if (typeof app.sound_did_not_start === "function") app.sound_did_not_start();
    }
  }
  if (els && els.sound) els.sound.checked = Boolean(app.sound_enabled);
  if (els && els.status && typeof app.status_line === "function") {
    const text = pendingThemeHint || formatChromeStatus(app);
    writeLiveRegion(els.announce, announcementFromStatus(text));
    writeLiveRegion(els.status, text);
  }
  return { on: Boolean(app.sound_enabled), started };
}

export function handleGameKeyDown(app, e) {
  // Let the HTML chrome (checkboxes, selects, links) keep native keys.
  if (isChromeTarget(e.target)) return false;
  if (hasCommandModifier(e)) return false;
  pendingThemeHint = null;
  switch (e.code) {
    case "ArrowUp":
    case "ArrowDown":
    case "ArrowLeft":
    case "ArrowRight":
    case "Space":
      if (typeof e.preventDefault === "function") e.preventDefault();
      break;
    default:
      break;
  }
  // Repeat is gated in WASM: Space/Enter hold-to-fire only if the
  // originating (non-repeat) keydown was already on Fire.
  if (app) app.key_down(e.code, e.repeat);
  return true;
}

/**
 * On-screen touch pad. Every button sends the same `KeyboardEvent.code`
 * through `key_down` / `key_up` as the matching key, so all game rules (demo
 * stop, fire denial, CRITICAL/EMPTY/LINK DOWN) stay in WASM.
 */
export const TOUCH_PAD_ACTIONS = Object.freeze({
  left: "ArrowLeft",
  right: "ArrowRight",
  up: "ArrowUp",
  down: "ArrowDown",
  fire: "Space",
  arm: "KeyA",
  reload: "KeyR",
  panel: "Escape",
});

/** Hold-to-fire mimics OS key repeat: first repeat after a delay, then steady. */
export const FIRE_REPEAT_DELAY_MS = 500;
export const FIRE_REPEAT_INTERVAL_MS = 33;

/** Media query that shows the pad by default (phones / tablets). */
export const TOUCH_PAD_DEFAULT_QUERY = "(pointer: coarse)";

/** Stored Touch pad choice wins; otherwise the pointer media query decides. */
export function initialTouchPadVisible(stored, coarsePointer) {
  return stored && typeof stored.touchPad === "boolean"
    ? stored.touchPad
    : Boolean(coarsePointer);
}

/** The pad does nothing during POST, after Quit, or before WASM loads. */
export function padIsInert(app) {
  if (!app) return true;
  if (app.should_quit) return true;
  return typeof app.screen_name === "function" && app.screen_name() === "boot";
}

/** A tap: one non-repeat keydown + keyup, exactly like a key press. */
export function pressPadAction(app, action) {
  const code = TOUCH_PAD_ACTIONS[action];
  if (!code || padIsInert(app)) return false;
  pendingThemeHint = null;
  app.key_down(code, false);
  app.key_up(code);
  return true;
}

/**
 * Press-and-hold Fire. `start` sends the originating Space keydown, then
 * repeat keydowns (WASM only honours them if the origin was on Fire). `stop`
 * is idempotent and always sends the keyup, so fire cannot stick on.
 */
export function createFireHold(getApp, timers = globalThis) {
  let delayId = null;
  let intervalId = null;
  let holding = null;
  const clear = () => {
    if (delayId !== null) timers.clearTimeout(delayId);
    if (intervalId !== null) timers.clearInterval(intervalId);
    delayId = null;
    intervalId = null;
  };
  const hold = {
    active: () => holding !== null,
    start() {
      if (holding) return false;
      const app = getApp();
      if (padIsInert(app)) return false;
      pendingThemeHint = null;
      holding = app;
      app.key_down(TOUCH_PAD_ACTIONS.fire, false);
      delayId = timers.setTimeout(() => {
        delayId = null;
        intervalId = timers.setInterval(() => {
          const current = getApp();
          if (current !== holding || padIsInert(current)) {
            hold.stop();
            return;
          }
          current.key_down(TOUCH_PAD_ACTIONS.fire, true);
        }, FIRE_REPEAT_INTERVAL_MS);
      }, FIRE_REPEAT_DELAY_MS);
      return true;
    },
    stop() {
      clear();
      if (!holding) return false;
      const app = holding;
      holding = null;
      try {
        app.key_up(TOUCH_PAD_ACTIONS.fire);
      } catch (_) {
        /* app freed mid-hold (reboot) — nothing left to release */
      }
      return true;
    },
  };
  return hold;
}

function padButton(target) {
  return target && target.closest ? target.closest("[data-pad-action]") : null;
}

/**
 * Wire the pad. Pointer presses act on `pointerdown` and never move focus
 * (keyboard play stays on the canvas); keyboard activation of a focused pad
 * button (`click` with `detail === 0`) taps and refocuses the canvas.
 */
export function bindTouchPad(root, { getApp, canvas, hold }) {
  if (!root || typeof root.addEventListener !== "function") return () => {};
  const onPointerDown = (e) => {
    const button = padButton(e.target);
    if (!button || button.disabled) return;
    if (typeof e.preventDefault === "function") e.preventDefault();
    const action = button.dataset.padAction;
    if (action === "fire") {
      if (hold.start() && button.classList) button.classList.add("is-held");
    } else {
      pressPadAction(getApp(), action);
    }
  };
  const release = (e) => {
    const button = padButton(e.target);
    if (!button || button.dataset.padAction !== "fire") return;
    hold.stop();
    if (button.classList) button.classList.remove("is-held");
  };
  const onClick = (e) => {
    const button = padButton(e.target);
    if (!button || e.detail !== 0) return;
    pressPadAction(getApp(), button.dataset.padAction);
    refocusPlaySurface(canvas);
  };
  const onContextMenu = (e) => {
    if (padButton(e.target) && typeof e.preventDefault === "function") e.preventDefault();
  };
  const handlers = [
    ["pointerdown", onPointerDown],
    ["pointerup", release],
    ["pointercancel", release],
    ["pointerleave", release],
    ["click", onClick],
    ["contextmenu", onContextMenu],
  ];
  // pointerleave does not bubble; capture it on the pad root.
  for (const [type, fn] of handlers) root.addEventListener(type, fn, type === "pointerleave");
  return () => {
    hold.stop();
    for (const [type, fn] of handlers) {
      root.removeEventListener(type, fn, type === "pointerleave");
    }
  };
}

function setIfChanged(el, attr, value) {
  if (!el || typeof el.getAttribute !== "function") return;
  if (el.getAttribute(attr) !== value) el.setAttribute(attr, value);
}

/** Per-frame pad state: disabled while inert, `aria-pressed` on toggles. */
export function syncTouchPad(app, root) {
  if (!root || typeof root.querySelectorAll !== "function") return;
  const inert = padIsInert(app);
  for (const button of root.querySelectorAll("[data-pad-action]")) {
    if (button.disabled !== inert) button.disabled = inert;
  }
  const panel = root.querySelector('[data-pad-action="panel"]');
  if (panel && app && typeof app.screen_name === "function") {
    setIfChanged(panel, "aria-pressed", String(app.screen_name() === "options"));
  }
  const arm = root.querySelector('[data-pad-action="arm"]');
  if (arm && app && typeof app.armed === "boolean") {
    setIfChanged(arm, "aria-pressed", String(app.armed));
  }
}

/** Show/hide the pad and keep its header toggle's `aria-pressed` in sync. */
export function setTouchPadVisible(pad, toggle, visible) {
  if (pad) pad.hidden = !visible;
  if (toggle && typeof toggle.setAttribute === "function") {
    toggle.setAttribute("aria-pressed", String(Boolean(visible)));
  }
  if (!visible) fireHold.stop();
  return Boolean(visible);
}

/** @internal The page's shared hold-to-fire controller (tests drive it). */
export function touchPadFireHold() {
  return fireHold;
}

/**
 * Fullscreen. The button fullscreens the canvas + touch-pad wrapper;
 * nothing re-instantiates WASM, so game state, demo, and sound carry over.
 * Fullscreen is never written to the URL or saved prefs.
 * An installed home-screen app (`display-mode: standalone`, or iOS
 * `navigator.standalone`) is already edge-to-edge, so the button stays hidden.
 */
export const FULLSCREEN_LABELS = Object.freeze({ enter: "Fullscreen", exit: "Exit fullscreen" });

/** True when the page is an installed standalone app (already full-screen). */
export function isStandaloneDisplay(win, nav) {
  const query =
    win && typeof win.matchMedia === "function" ? win.matchMedia("(display-mode: standalone)") : null;
  return Boolean((query && query.matches) || (nav && nav.standalone));
}

/** Largest CSS size that fits the viewport at the canvas's own aspect ratio. */
export function fitCanvasSize({
  viewportWidth,
  viewportHeight,
  canvasWidth,
  canvasHeight,
  reservedHeight = 0,
}) {
  const availableHeight = Math.max(0, viewportHeight - reservedHeight);
  if (!(viewportWidth > 0) || !(availableHeight > 0) || !(canvasWidth > 0) || !(canvasHeight > 0)) {
    return { width: 0, height: 0 };
  }
  const scale = Math.min(viewportWidth / canvasWidth, availableHeight / canvasHeight);
  return {
    width: Math.floor(canvasWidth * scale),
    height: Math.floor(canvasHeight * scale),
  };
}

export function isFullscreenSupported(doc, stage) {
  return Boolean(
    doc && doc.fullscreenEnabled && stage && typeof stage.requestFullscreen === "function"
  );
}

/** Keep the button's pressed state and label in sync with the document. */
export function syncFullscreenButton(button, active) {
  if (!button) return;
  const pressed = String(Boolean(active));
  if (typeof button.getAttribute !== "function" || button.getAttribute("aria-pressed") !== pressed) {
    button.setAttribute("aria-pressed", pressed);
  }
  const label = active ? FULLSCREEN_LABELS.exit : FULLSCREEN_LABELS.enter;
  if (button.textContent !== label) button.textContent = label;
}

/** Size the canvas for fullscreen, or clear the inline size to restore the page layout. */
export function applyFullscreenCanvasSize(canvas, { active, viewportWidth, viewportHeight, pad, gap = 0 }) {
  if (!canvas || !canvas.style) return;
  if (!active) {
    canvas.style.width = "";
    canvas.style.height = "";
    return;
  }
  const padOpen = Boolean(pad && !pad.hidden);
  const reservedHeight = padOpen ? (pad.offsetHeight || 0) + gap : 0;
  const { width, height } = fitCanvasSize({
    viewportWidth,
    viewportHeight,
    canvasWidth: canvas.width,
    canvasHeight: canvas.height,
    reservedHeight,
  });
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
}

/**
 * Wire the Fullscreen button. Hidden when the Fullscreen API is unavailable
 * or the page is already standalone (Add to Home Screen). A hidden button has
 * no click listener, so it cannot request fullscreen.
 * Any exit route (button, browser Esc, API) arrives as `fullscreenchange`,
 * which restores the layout, syncs the button, and refocuses the canvas.
 * Returns `refit` (re-size while fullscreen, e.g. after the pad opens) and `unbind`.
 */
export function bindFullscreen({ doc, win, nav, button, stage, canvas, pad, gap = 12 }) {
  const inert = { refit() {}, unbind() {} };
  if (!button) return inert;
  const supported = isFullscreenSupported(doc, stage) && !isStandaloneDisplay(win, nav);
  button.hidden = !supported;
  if (!supported) return inert;

  const isActive = () => doc.fullscreenElement === stage;
  const resize = () =>
    applyFullscreenCanvasSize(canvas, {
      active: isActive(),
      viewportWidth: win && win.innerWidth,
      viewportHeight: win && win.innerHeight,
      pad,
      gap,
    });
  const onChange = () => {
    syncFullscreenButton(button, isActive());
    resize();
    refocusPlaySurface(canvas);
  };
  const onClick = () => {
    const request = isActive() ? doc.exitFullscreen() : stage.requestFullscreen();
    // A refused request (no user gesture, policy) leaves the page as it was.
    if (request && typeof request.catch === "function") {
      request.catch(() => refocusPlaySurface(canvas));
    }
  };
  const refit = () => {
    if (isActive()) resize();
  };

  syncFullscreenButton(button, isActive());
  button.addEventListener("click", onClick);
  doc.addEventListener("fullscreenchange", onChange);
  if (win && typeof win.addEventListener === "function") win.addEventListener("resize", refit);
  return {
    refit,
    unbind() {
      button.removeEventListener("click", onClick);
      doc.removeEventListener("fullscreenchange", onChange);
      if (win && typeof win.removeEventListener === "function") win.removeEventListener("resize", refit);
    },
  };
}

/** Glue and wasm URLs carrying the same `?v=` so a deploy never pairs new glue with an old binary. */
export function wasmAssetUrls(buildId, base) {
  const v = encodeURIComponent(buildId);
  return {
    glue: new URL(`./pkg/ua571_web.js?v=${v}`, base).href,
    wasm: new URL(`./pkg/ua571_web_bg.wasm?v=${v}`, base).href,
  };
}

export async function boot() {
  const generation = ++bootGeneration;
  const status = document.getElementById("status");
  const announce = document.getElementById("status-announce");
  const canvas = document.getElementById("ua571");
  const opts = readOptions();
  const stale = () => generation !== bootGeneration;

  applyPageTheme(opts.theme);
  teardown();
  status.textContent = "Loading WebAssembly…";
  writeLiveRegion(announce, status.textContent);

  try {
    let created = null;
    if (bootInstanceLoader) {
      created = await bootInstanceLoader(opts);
    } else {
      // Cache-bust glue and wasm with the same BUILD_ID; fetch the wasm in parallel with the glue import.
      const { BUILD_ID } = await import(`./build-id.js?v=${Date.now()}`);
      if (stale()) return;
      const urls = wasmAssetUrls(BUILD_ID, import.meta.url);
      const wasmFetch = fetch(urls.wasm);
      wasmFetch.catch(() => {});
      const wasm = await import(urls.glue);
      if (stale()) return;
      await wasm.default({ module_or_path: wasmFetch });
      if (stale()) return;
      showVersion(wasm.pkg_version());
      created = new wasm.Ua571Web(
        "ua571",
        opts.theme,
        opts.scale,
        opts.demo,
        opts.skipBoot,
        opts.sound,
        opts.systemMode,
        opts.weaponStatus,
        opts.iffStatus
      );
    }
    // A newer boot may have started during an await. Free only the instance
    // this call created — teardown of the latest boot owns the live listeners.
    if (stale()) {
      freeWasmInstance(created);
      return;
    }

    app = created;
    lastPrefs = snapshotPrefs(prefsContext());

    onKey = (e) => {
      if (
        handleThemeKeyDown(e, {
          select: document.getElementById("theme"),
          status,
          sync: syncPagePrefs,
        })
      ) {
        if (!applyThemeLive(app, document.getElementById("theme").value)) boot();
        return;
      }
      handleGameKeyDown(app, e);
    };
    onKeyUp = (e) => {
      if (app) app.key_up(e.code);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKeyUp);

    const pauseLoop = () => {
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
    };
    const loop = () => {
      if (!app) return;
      if (
        typeof document !== "undefined" &&
        !shouldAdvanceFrame(document.hidden)
      ) {
        raf = 0;
        return;
      }
      syncChromeFromApp(app, {
        status,
        announce,
        demo: document.getElementById("demo"),
        sound: document.getElementById("sound"),
      });
      syncTouchPad(app, document.getElementById("touch-pad"));
      syncPagePrefs();
      raf = requestAnimationFrame(loop);
    };
    const loopCtl = {
      pause: pauseLoop,
      resume: () => {
        if (app && !raf) raf = requestAnimationFrame(loop);
      },
    };
    onVisibility = () => {
      if (document.hidden) fireHold.stop();
      applyDocumentVisibility(Boolean(document.hidden), app, loopCtl);
    };
    document.addEventListener("visibilitychange", onVisibility);
    if (document.hidden) {
      onVisibility();
    } else {
      raf = requestAnimationFrame(loop);
    }

    if (shouldRefocusAfterBoot(document.activeElement)) {
      refocusPlaySurface(canvas);
    }
  } catch (err) {
    if (stale()) return;
    console.error(err);
    const hostname =
      typeof location !== "undefined" && location ? location.hostname : "";
    status.textContent = wasmLoadFailureStatus(hostname);
    writeLiveRegion(announce, status.textContent);
  }
}

/** Everything `syncPrefs` reads from: live app, page form, storage (URL defaults to the window's). */
function prefsContext() {
  const el = (id) => document.getElementById(id);
  return {
    app,
    els: {
      theme: el("theme"),
      scale: el("scale"),
      sound: el("sound"),
      skipBoot: el("skipBoot"),
      demo: el("demo"),
    },
    storage: pageStorage(),
    touchPad: touchPadChoice,
  };
}

/** The only prefs write: every chrome listener, hotkey and frame calls this. */
function syncPagePrefs() {
  lastPrefs = syncPrefs(prefsContext(), lastPrefs);
}

export function startPage() {
  const canvas = document.getElementById("ua571");
  const skip = document.querySelector("a.skip-link");
  if (skip) {
    skip.addEventListener("click", (e) => {
      handleSkipToPlaySurface(e, document.getElementById("ua571"));
    });
  }
  bindPlaySurfaceRefocus(document.querySelector(".controls"), canvas);

  const pad = document.getElementById("touch-pad");
  const padToggle = document.getElementById("touchPadToggle");
  bindTouchPad(pad, { getApp: () => app, canvas, hold: fireHold });
  const fullscreen = bindFullscreen({
    doc: document,
    win: window,
    nav: typeof navigator !== "undefined" ? navigator : undefined,
    button: document.getElementById("fullscreenToggle"),
    stage: document.getElementById("console-stage"),
    canvas,
    pad,
  });
  touchPadChoice = readStoredPrefs(pageStorage()).touchPad;
  let padVisible = setTouchPadVisible(
    pad,
    padToggle,
    initialTouchPadVisible(
      { touchPad: touchPadChoice },
      typeof window.matchMedia === "function" &&
        window.matchMedia(TOUCH_PAD_DEFAULT_QUERY).matches
    )
  );
  if (padToggle) {
    padToggle.addEventListener("click", () => {
      padVisible = setTouchPadVisible(pad, padToggle, !padVisible);
      touchPadChoice = padVisible;
      syncPagePrefs();
      // Opening/closing the pad in fullscreen changes the room left for the canvas.
      fullscreen.refit();
    });
  }

  document.getElementById("restart").addEventListener("click", () => {
    boot();
  });

  // Scale still re-creates the WASM app (canvas pixels are not CSS); theme is
  // applied live when the app is available.
  document.getElementById("theme").addEventListener("change", (e) => {
    syncPagePrefs();
    applyPageTheme(e.target.value);
    if (!applyThemeLive(app, e.target.value)) boot();
  });
  document.getElementById("scale").addEventListener("change", () => {
    syncPagePrefs();
    boot();
  });

  document.getElementById("sound").addEventListener("change", async (e) => {
    if (!app) syncPagePrefs();
    else {
      await applySoundChoice(app, e.target.checked, {
        sound: e.target,
        status: document.getElementById("status"),
        announce: document.getElementById("status-announce"),
      });
      // Sync after the unlock: a failed one turned sound back off, store that.
      syncPagePrefs();
      refocusPlaySurface(document.getElementById("ua571"));
    }
  });

  document.getElementById("demo").addEventListener("change", (e) => {
    if (app) app.set_demo(e.target.checked);
    syncPagePrefs();
    if (app) refocusPlaySurface(document.getElementById("ua571"));
  });

  document.getElementById("skipBoot").addEventListener("change", () => {
    syncPagePrefs();
    boot();
  });

  const els = {
    theme: document.getElementById("theme"),
    scale: document.getElementById("scale"),
    sound: document.getElementById("sound"),
    skipBoot: document.getElementById("skipBoot"),
    demo: document.getElementById("demo"),
  };
  hydrateChromePrefs({
    storage: pageStorage(),
    search: location.search,
    els,
  });
  applyPageTheme(document.getElementById("theme").value);
  // Hydrated (possibly shared-link) values are the baseline: nothing is written
  // until the user changes something.
  lastPrefs = snapshotPrefs(prefsContext());

  boot();
}

if (typeof document !== "undefined") {
  startPage();
}
