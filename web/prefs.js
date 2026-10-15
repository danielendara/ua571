/**
 * Chrome prefs and share-URL state for the UA 571-C page: localStorage
 * reads/writes, `?theme=` style query params, and the form <-> prefs glue.
 * No DOM or WASM at module level, so Node tests (and main.js) can import it.
 */

/** localStorage key for last chrome prefs (not demo). */
export const PREFS_STORAGE_KEY = "ua571.chrome";

/** `localStorage`, or null when the browser blocks or lacks it (the getter itself can throw). */
export function pageStorage(win = globalThis) {
  try {
    return (win && win.localStorage) || null;
  } catch (_) {
    return null;
  }
}

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
    if (typeof parsed.touchPad === "boolean") out.touchPad = parsed.touchPad;
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
  if (!storage || typeof storage.setItem !== "function") return false;
  const next = { ...readStoredPrefs(storage), ...prefs };
  delete next.demo;
  try {
    storage.setItem(PREFS_STORAGE_KEY, JSON.stringify(next));
    return true;
  } catch (_) {
    return false;
  }
}

/** Built-in chrome defaults — omitted from the share URL. */
const DEFAULT_CHROME_PREFS = {
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

/** True when `<select>` has an option for `value` (mocks without `options` accept anything). */
export function selectHasValue(select, value) {
  if (!select) return false;
  if (!select.options) return true;
  return Array.from(select.options, (o) => o.value).includes(value);
}

export function applyPrefsToElements(prefs, els) {
  if (!prefs || !els) return;
  if (prefs.theme && els.theme) {
    const theme = String(prefs.theme).toLowerCase();
    if (selectHasValue(els.theme, theme)) els.theme.value = theme;
  }
  if (prefs.scale != null && els.scale) {
    const scale = String(prefs.scale).trim();
    if (selectHasValue(els.scale, scale)) els.scale.value = scale;
  }
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
 * M / D flip sound and demo inside WASM; setting a checkbox's `.checked` in
 * code fires no `change`, so compare against `last` each frame. Sound goes to
 * storage and the share URL; demo only to the share URL (writeStoredPrefs
 * drops it). Returns the (possibly updated) `last`.
 */
export function persistKeyTogglesIfChanged(app, storage, last, { location, history, readOpts } = {}) {
  if (!app) return last;
  const next = { sound: Boolean(app.sound_enabled), demo: Boolean(app.demo_active) };
  if (next.sound === last.sound && next.demo === last.demo) return last;
  if (next.sound !== last.sound && storage) {
    writeStoredPrefs(storage, { sound: next.sound });
  }
  syncShareUrl(readOpts ? readOpts() : next, location, history);
  return next;
}
