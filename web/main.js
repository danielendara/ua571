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
/** Last values written to localStorage, so persistOptionsIfChanged only
 * writes on an actual change (reset on every (re)boot). */
let lastPersistedOptions = { systemMode: null, weaponStatus: null, iffStatus: null };
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
    typeof localStorage !== "undefined" ? localStorage : null
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

function applyPageTheme(theme) {
  if (typeof document === "undefined" || !document.body) return;
  document.body.dataset.theme = theme || "yellow";
}

function showVersion(v) {
  const wrap = document.getElementById("app-version-wrap");
  const el = document.getElementById("app-version");
  if (!wrap || !el || !v) return;
  el.textContent = `v${v}`;
  el.href = `https://github.com/danielendara/ua571/releases/tag/v${v}`;
  wrap.hidden = false;
}

/** localStorage key for last chrome prefs (not demo). */
export const PREFS_STORAGE_KEY = "ua571.chrome";

export function readStoredPrefs(storage) {
  if (!storage || typeof storage.getItem !== "function") return {};
  try {
    const raw = storage.getItem(PREFS_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {};
    const out = {};
    if (typeof parsed.theme === "string" && parsed.theme) out.theme = parsed.theme;
    if (parsed.scale != null && String(parsed.scale) !== "") {
      out.scale = String(parsed.scale);
    }
    if (typeof parsed.sound === "boolean") out.sound = parsed.sound;
    if (typeof parsed.skipBoot === "boolean") out.skipBoot = parsed.skipBoot;
    // Not validated against the known variant names here — the WASM
    // constructor's parse_* helpers already fall back to that type's
    // default on anything unrecognized, so a stale/invalid stored value
    // can never fail boot.
    if (typeof parsed.systemMode === "string" && parsed.systemMode) {
      out.systemMode = parsed.systemMode;
    }
    if (typeof parsed.weaponStatus === "string" && parsed.weaponStatus) {
      out.weaponStatus = parsed.weaponStatus;
    }
    if (typeof parsed.iffStatus === "string" && parsed.iffStatus) {
      out.iffStatus = parsed.iffStatus;
    }
    return out;
  } catch (_) {
    return {};
  }
}

export function writeStoredPrefs(storage, prefs) {
  if (!storage || typeof storage.setItem !== "function") return;
  const next = { ...readStoredPrefs(storage), ...prefs };
  delete next.demo;
  storage.setItem(PREFS_STORAGE_KEY, JSON.stringify(next));
}

/** Built-in chrome defaults — omitted from the share URL. */
export const DEFAULT_CHROME_PREFS = {
  theme: "yellow",
  scale: "3",
  sound: false,
  skipBoot: false,
  demo: false,
};

/** Query params that are present override stored prefs for this visit. */
export function prefsFromSearch(search) {
  const p =
    typeof search === "string"
      ? new URLSearchParams(search)
      : search || new URLSearchParams();
  const out = {};
  if (p.get("theme")) out.theme = p.get("theme");
  if (p.get("scale")) out.scale = p.get("scale");
  // Present `sound` wins: `1` on, anything else (including `0`) off.
  // Omitted `sound` leaves LS / HTML default (muted).
  if (p.has("sound")) out.sound = p.get("sound") === "1";
  if (p.has("boot")) out.skipBoot = p.get("boot") === "0";
  if (p.get("demo") === "1") out.demo = true;
  return out;
}

/**
 * Inverse of `prefsFromSearch`: only non-default chrome keys.
 * Default muted sound is omitted (not `sound=0`); inbound `sound=0` still
 * means off. Demo is URL-only (`demo=1` when on).
 */
export function searchFromPrefs(prefs) {
  const p = new URLSearchParams();
  const theme = prefs && prefs.theme;
  if (theme && theme !== DEFAULT_CHROME_PREFS.theme) p.set("theme", theme);
  const scale =
    prefs && prefs.scale != null && String(prefs.scale) !== ""
      ? String(prefs.scale)
      : "";
  if (scale && scale !== DEFAULT_CHROME_PREFS.scale) p.set("scale", scale);
  if (prefs && prefs.sound) p.set("sound", "1");
  if (prefs && prefs.skipBoot) p.set("boot", "0");
  if (prefs && prefs.demo) p.set("demo", "1");
  const q = p.toString();
  return q ? `?${q}` : "";
}

/** `history.replaceState` the share query; no-ops without a history object. */
export function syncShareUrl(prefs, location, history) {
  const loc =
    location || (typeof window !== "undefined" ? window.location : null);
  const hist =
    history || (typeof window !== "undefined" ? window.history : null);
  if (!loc || !hist || typeof hist.replaceState !== "function") return "";
  const search = searchFromPrefs(prefs);
  const path = loc.pathname || "/";
  const hash = loc.hash || "";
  const next = `${path}${search}${hash}`;
  const current = `${path}${loc.search || ""}${hash}`;
  if (next !== current) {
    try {
      hist.replaceState(hist.state ?? null, "", next);
    } catch (_) {
      /* file:// or sandboxed — keep chrome usable */
    }
  }
  return search;
}

export function applyPrefsToElements(prefs, els) {
  if (!prefs || !els) return;
  if (prefs.theme && els.theme) els.theme.value = prefs.theme;
  if (prefs.scale != null && els.scale) els.scale.value = String(prefs.scale);
  if (typeof prefs.sound === "boolean" && els.sound) {
    els.sound.checked = prefs.sound;
  }
  if (typeof prefs.skipBoot === "boolean" && els.skipBoot) {
    els.skipBoot.checked = prefs.skipBoot;
  }
  if (typeof prefs.demo === "boolean" && els.demo) {
    els.demo.checked = prefs.demo;
  }
}

/** Stored prefs first, then `?` params when present. */
export function hydrateChromePrefs({ storage, search, els }) {
  applyPrefsToElements(readStoredPrefs(storage), els);
  applyPrefsToElements(prefsFromSearch(search), els);
}

export function persistChromeFromForm(storage, opts) {
  writeStoredPrefs(storage, {
    theme: opts.theme,
    scale: String(opts.scale),
    sound: Boolean(opts.sound),
    skipBoot: Boolean(opts.skipBoot),
  });
}

export function isChromeTarget(el) {
  return Boolean(
    el &&
      el.closest &&
      el.closest("input, select, button, a, label, textarea")
  );
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
 * T key: cycle theme, persist chrome prefs, repaint page chrome, show hint.
 * Returns `{ theme, hint }` when handled, else `false`.
 */
export function handleThemeKeyDown(
  e,
  { select, storage, status, readOpts, location, history }
) {
  if (!e || e.code !== "KeyT" || e.repeat) return false;
  if (isChromeTarget(e.target)) return false;
  if (!select) return false;

  const theme = cycleThemeSelect(select);
  const opts = readOpts ? readOpts() : { theme };
  if (storage && readOpts) {
    persistChromeFromForm(storage, opts);
  }
  syncShareUrl(opts, location, history);
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

/**
 * After chrome `change` (or a button click), put focus back on the canvas
 * so keyboard play is not trapped in the header controls.
 */
export function bindPlaySurfaceRefocus(chromeRoot, canvas) {
  if (!chromeRoot || !canvas) return () => {};
  const onChange = () => {
    refocusPlaySurface(canvas);
  };
  const onClick = (e) => {
    const t = e && e.target;
    if (t && t.closest && t.closest("button")) refocusPlaySurface(canvas);
  };
  chromeRoot.addEventListener("change", onChange);
  chromeRoot.addEventListener("click", onClick);
  return () => {
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
  return writeLiveRegion(els.status, next);
}

/**
 * System mode / weapon status / IFF have no chrome `change` event (they're
 * changed via key presses, not a checkbox/select) — write through
 * immediately by comparing against `last` each frame and persisting only on
 * an actual change, same "changing a control writes through immediately"
 * contract theme/scale/sound/skipBoot get from their own `change` listeners.
 * Returns the (possibly updated) `last` object for the caller to keep.
 */
export function persistOptionsIfChanged(app, storage, last) {
  if (!app) return last;
  const next = {
    systemMode: app.system_mode,
    weaponStatus: app.weapon_status,
    iffStatus: app.iff_status,
  };
  if (
    next.systemMode === last.systemMode &&
    next.weaponStatus === last.weaponStatus &&
    next.iffStatus === last.iffStatus
  ) {
    return last;
  }
  writeStoredPrefs(storage, next);
  return next;
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
    writeLiveRegion(els.status, pendingThemeHint || formatChromeStatus(app));
  }
  return { on: Boolean(app.sound_enabled), started };
}

export function handleGameKeyDown(app, e) {
  // Let the HTML chrome (checkboxes, selects, links) keep native keys.
  if (isChromeTarget(e.target)) return false;
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
 * On-screen touch pad (#132). Every button sends the same `KeyboardEvent.code`
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
 * button (`click` with `detail === 0`) taps and refocuses the canvas (#84).
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
 * Fullscreen (#136). The button fullscreens the canvas + touch-pad wrapper;
 * nothing re-instantiates WASM, so game state, demo, and sound carry over.
 * Fullscreen is never written to the URL or saved prefs.
 */
export const FULLSCREEN_LABELS = Object.freeze({ enter: "Fullscreen", exit: "Exit fullscreen" });

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
 * Wire the Fullscreen button. Hidden when the Fullscreen API is unavailable.
 * Any exit route (button, browser Esc, API) arrives as `fullscreenchange`,
 * which restores the layout, syncs the button, and refocuses the canvas.
 * Returns `refit` (re-size while fullscreen, e.g. after the pad opens) and `unbind`.
 */
export function bindFullscreen({ doc, win, button, stage, canvas, pad, gap = 12 }) {
  const inert = { refit() {}, unbind() {} };
  if (!button) return inert;
  const supported = isFullscreenSupported(doc, stage);
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

export async function boot() {
  const generation = ++bootGeneration;
  const status = document.getElementById("status");
  const canvas = document.getElementById("ua571");
  const opts = readOptions();
  const stale = () => generation !== bootGeneration;

  applyPageTheme(opts.theme);
  teardown();
  status.textContent = "Loading WebAssembly…";

  try {
    let created = null;
    if (bootInstanceLoader) {
      created = await bootInstanceLoader(opts);
    } else {
      // Cache-bust JS (and relative wasm URL via import.meta.url) after each deploy.
      const { BUILD_ID } = await import(`./build-id.js?v=${Date.now()}`);
      if (stale()) return;
      const wasm = await import(`./pkg/ua571_web.js?v=${BUILD_ID}`);
      if (stale()) return;
      await wasm.default();
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
    lastPersistedOptions = { systemMode: null, weaponStatus: null, iffStatus: null };

    onKey = (e) => {
      if (
        handleThemeKeyDown(e, {
          select: document.getElementById("theme"),
          storage:
            typeof localStorage !== "undefined" ? localStorage : null,
          status,
          readOpts: readOptions,
        })
      ) {
        boot();
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
      if (typeof document !== "undefined" && document.hidden) {
        raf = 0;
        return;
      }
      syncChromeFromApp(app, {
        status,
        demo: document.getElementById("demo"),
        sound: document.getElementById("sound"),
      });
      syncTouchPad(app, document.getElementById("touch-pad"));
      if (typeof localStorage !== "undefined") {
        lastPersistedOptions = persistOptionsIfChanged(
          app,
          localStorage,
          lastPersistedOptions
        );
      }
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

    refocusPlaySurface(canvas);
  } catch (err) {
    if (stale()) return;
    console.error(err);
    const hostname =
      typeof location !== "undefined" && location ? location.hostname : "";
    status.textContent = wasmLoadFailureStatus(hostname);
  }
}

function persistPagePrefs() {
  const opts = readOptions();
  if (typeof localStorage !== "undefined") {
    persistChromeFromForm(localStorage, opts);
  }
  syncShareUrl(opts);
}

function sharePagePrefs() {
  syncShareUrl(readOptions());
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
    button: document.getElementById("fullscreenToggle"),
    stage: document.getElementById("console-stage"),
    canvas,
    pad,
  });
  let padVisible = setTouchPadVisible(
    pad,
    padToggle,
    typeof window.matchMedia === "function" &&
      window.matchMedia(TOUCH_PAD_DEFAULT_QUERY).matches
  );
  if (padToggle) {
    padToggle.addEventListener("click", () => {
      padVisible = setTouchPadVisible(pad, padToggle, !padVisible);
      // Opening/closing the pad in fullscreen changes the room left for the canvas.
      fullscreen.refit();
    });
  }

  document.getElementById("restart").addEventListener("click", () => {
    boot();
  });

  // Theme/scale must re-create the WASM app (canvas pixels are not CSS).
  document.getElementById("theme").addEventListener("change", (e) => {
    persistPagePrefs();
    applyPageTheme(e.target.value);
    boot();
  });
  document.getElementById("scale").addEventListener("change", () => {
    persistPagePrefs();
    boot();
  });

  document.getElementById("sound").addEventListener("change", async (e) => {
    persistPagePrefs();
    if (app) {
      await applySoundChoice(app, e.target.checked, {
        sound: e.target,
        status: document.getElementById("status"),
      });
      refocusPlaySurface(document.getElementById("ua571"));
    }
  });

  document.getElementById("demo").addEventListener("change", (e) => {
    sharePagePrefs();
    if (app) {
      app.set_demo(e.target.checked);
      refocusPlaySurface(document.getElementById("ua571"));
    }
  });

  document.getElementById("skipBoot").addEventListener("change", () => {
    persistPagePrefs();
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
    storage: typeof localStorage !== "undefined" ? localStorage : null,
    search: location.search,
    els,
  });
  applyPageTheme(document.getElementById("theme").value);

  boot();
}

if (typeof document !== "undefined") {
  startPage();
}
