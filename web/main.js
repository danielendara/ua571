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
/** Last values written to localStorage, so persistOptionsIfChanged only
 * writes on an actual change (reset on every (re)boot). */
let lastPersistedOptions = { systemMode: null, weaponStatus: null, iffStatus: null };
/** One-shot status after T cycles theme (cleared on the next game key). */
let pendingThemeHint = null;

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

/** Query params that are present override stored prefs for this visit. */
export function prefsFromSearch(search) {
  const p =
    typeof search === "string"
      ? new URLSearchParams(search)
      : search || new URLSearchParams();
  const out = {};
  if (p.get("theme")) out.theme = p.get("theme");
  if (p.get("scale")) out.scale = p.get("scale");
  if (p.has("sound")) out.sound = p.get("sound") === "1";
  if (p.has("boot")) out.skipBoot = p.get("boot") === "0";
  return out;
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
export function handleThemeKeyDown(e, { select, storage, status, readOpts }) {
  if (!e || e.code !== "KeyT" || e.repeat) return false;
  if (isChromeTarget(e.target)) return false;
  if (!select) return false;

  const theme = cycleThemeSelect(select);
  if (storage && readOpts) {
    persistChromeFromForm(storage, readOpts());
  }
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

async function boot() {
  const status = document.getElementById("status");
  const canvas = document.getElementById("ua571");
  const opts = readOptions();

  applyPageTheme(opts.theme);
  teardown();
  status.textContent = "Loading WebAssembly…";

  try {
    // Cache-bust JS (and relative wasm URL via import.meta.url) after each deploy.
    const { BUILD_ID } = await import(`./build-id.js?v=${Date.now()}`);
    const wasm = await import(`./pkg/ua571_web.js?v=${BUILD_ID}`);
    await wasm.default();
    showVersion(wasm.pkg_version());

    app = new wasm.Ua571Web(
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
    console.error(err);
    status.textContent =
      "Failed to load WASM. Run: ./scripts/build-web.sh  then serve the web/ folder.";
  }
}

function persistPagePrefs() {
  if (typeof localStorage === "undefined") return;
  persistChromeFromForm(localStorage, readOptions());
}

function startPage() {
  const canvas = document.getElementById("ua571");
  const skip = document.querySelector("a.skip-link");
  if (skip) {
    skip.addEventListener("click", (e) => {
      handleSkipToPlaySurface(e, document.getElementById("ua571"));
    });
  }
  bindPlaySurfaceRefocus(document.querySelector(".controls"), canvas);

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
      app.set_sound(e.target.checked);
      if (e.target.checked) {
        try {
          await app.unlock_audio();
        } catch (_) {
          /* autoplay policy — next key still retries */
        }
      }
      refocusPlaySurface(document.getElementById("ua571"));
    }
  });

  document.getElementById("demo").addEventListener("change", (e) => {
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
  };
  hydrateChromePrefs({
    storage: typeof localStorage !== "undefined" ? localStorage : null,
    search: location.search,
    els,
  });
  // Demo is session/deep-link only — not persisted.
  const p = new URLSearchParams(location.search);
  if (p.get("demo") === "1") document.getElementById("demo").checked = true;
  applyPageTheme(document.getElementById("theme").value);

  boot();
}

if (typeof document !== "undefined") {
  startPage();
}
