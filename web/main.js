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

function readOptions() {
  return {
    theme: document.getElementById("theme").value,
    scale: Number(document.getElementById("scale").value) || 3,
    demo: document.getElementById("demo").checked,
    skipBoot: document.getElementById("skipBoot").checked,
    sound: document.getElementById("sound").checked,
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
  return writeLiveRegion(els.status, formatChromeStatus(app));
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
      opts.sound
    );

    onKey = (e) => {
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

    canvas.focus();
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
      document.getElementById("ua571").focus();
    }
  });

  document.getElementById("demo").addEventListener("change", (e) => {
    if (app) {
      app.set_demo(e.target.checked);
      document.getElementById("ua571").focus();
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
