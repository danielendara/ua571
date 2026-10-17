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

const stringPref = (v) => (typeof v === "string" && v ? v : undefined);
const boolPref = (v) => (typeof v === "boolean" ? v : undefined);

/** Form <-> pref glue for a `<select>` / checkbox element named `name`. */
const selectApply = (name, norm) => (v, els) => {
  if (!v || !els || !els[name]) return;
  const value = norm(v);
  if (selectHasValue(els[name], value)) els[name].value = value;
};
const checkApply = (name) => (v, els) => {
  if (typeof v === "boolean" && els && els[name]) els[name].checked = v;
};

/**
 * The only list of prefs, in stored-JSON key order. Each entry says where the
 * value lives (`read`), where it persists (`store`: localStorage, `url`: share
 * query param or null) and how each side encodes it. `read` returns undefined
 * when the value is not known yet (no app, pad never toggled); `syncPrefs`
 * then leaves the pref alone. `stored` validates a stored value, `fromUrl`
 * and `toUrl(value, default)` are the query-param codec (`toUrl` null = omit), `apply` pushes
 * a hydrated value into the form. Not validated against known variant names
 * for systemMode / weaponStatus / iffStatus: the WASM constructor's parse_*
 * helpers fall back to that type's default, so a stale value can't fail boot.
 */
export const PREFS = {
  theme: {
    default: "yellow",
    url: "theme",
    store: true,
    read: ({ els }) => (els && els.theme ? els.theme.value : undefined),
    stored: stringPref,
    fromUrl: (p) => p.get("theme") || undefined,
    toUrl: (v, d) => (v && v !== d ? v : null),
    apply: selectApply("theme", (v) => String(v).toLowerCase()),
  },
  scale: {
    default: "3",
    url: "scale",
    store: true,
    read: ({ els }) =>
      els && els.scale ? String(Number(els.scale.value) || 3) : undefined,
    stored: (v) => (v != null && String(v) !== "" ? String(v) : undefined),
    fromUrl: (p) => p.get("scale") || undefined,
    toUrl: (v, d) =>
      v != null && String(v) !== "" && String(v) !== d ? String(v) : null,
    apply: selectApply("scale", (v) => String(v).trim()),
  },
  sound: {
    default: false,
    url: "sound",
    store: true,
    read: ({ app, els }) =>
      app ? app.sound_enabled : els && els.sound ? els.sound.checked : undefined,
    stored: boolPref,
    // Present `sound` wins: `1` on, anything else (including `0`) off.
    // Omitted `sound` leaves LS / HTML default (muted).
    fromUrl: (p) => (p.has("sound") ? p.get("sound") === "1" : undefined),
    toUrl: (v) => (v ? "1" : null),
    apply: checkApply("sound"),
  },
  skipBoot: {
    default: false,
    url: "boot",
    store: true,
    read: ({ els }) => (els && els.skipBoot ? els.skipBoot.checked : undefined),
    stored: boolPref,
    fromUrl: (p) => (p.has("boot") ? p.get("boot") === "0" : undefined),
    toUrl: (v) => (v ? "0" : null),
    apply: checkApply("skipBoot"),
  },
  // URL-only: a shared link starts the demo, a reload never remembers it.
  demo: {
    default: false,
    url: "demo",
    store: false,
    read: ({ app, els }) =>
      app ? app.demo_active : els && els.demo ? els.demo.checked : undefined,
    fromUrl: (p) => (p.get("demo") === "1" ? true : undefined),
    toUrl: (v) => (v ? "1" : null),
    apply: checkApply("demo"),
  },
  // Undefined until the user toggles the pad: an absent key lets the
  // (pointer: coarse) media query decide, so `ctx.touchPad` is only the choice.
  touchPad: {
    default: undefined,
    url: null,
    store: true,
    read: ({ touchPad }) => touchPad,
    stored: boolPref,
  },
  systemMode: {
    default: "",
    url: null,
    store: true,
    read: ({ app }) => (app ? app.system_mode : undefined),
    stored: stringPref,
  },
  weaponStatus: {
    default: "",
    url: null,
    store: true,
    read: ({ app }) => (app ? app.weapon_status : undefined),
    stored: stringPref,
  },
  iffStatus: {
    default: "",
    url: null,
    store: true,
    read: ({ app }) => (app ? app.iff_status : undefined),
    stored: stringPref,
  },
};

export function readStoredPrefs(storage) {
  if (!storage || typeof storage.getItem !== "function") return {};
  try {
    const raw = storage.getItem(PREFS_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {};
    const out = {};
    for (const [key, def] of Object.entries(PREFS)) {
      if (!def.store) continue;
      const v = def.stored(parsed[key]);
      if (v !== undefined) out[key] = v;
    }
    return out;
  } catch (_) {
    return {};
  }
}

/** Merge `prefs` into storage (only `store` keys, in PREFS order). Prefer `syncPrefs`. */
export function writeStoredPrefs(storage, prefs) {
  if (!storage || typeof storage.setItem !== "function") return false;
  const merged = { ...readStoredPrefs(storage), ...prefs };
  const next = {};
  for (const [key, def] of Object.entries(PREFS)) {
    if (def.store && merged[key] !== undefined) next[key] = merged[key];
  }
  try {
    storage.setItem(PREFS_STORAGE_KEY, JSON.stringify(next));
    return true;
  } catch (_) {
    return false;
  }
}

/** Query params that are present override stored prefs for this visit. */
export function prefsFromSearch(search) {
  const p =
    typeof search === "string"
      ? new URLSearchParams(search)
      : search || new URLSearchParams();
  const out = {};
  for (const [key, def] of Object.entries(PREFS)) {
    if (!def.url) continue;
    const v = def.fromUrl(p);
    if (v !== undefined) out[key] = v;
  }
  return out;
}

/**
 * Inverse of `prefsFromSearch`: only non-default keys.
 * Default muted sound is omitted (not `sound=0`); inbound `sound=0` still
 * means off. Demo is URL-only (`demo=1` when on).
 */
export function searchFromPrefs(prefs) {
  const p = new URLSearchParams();
  for (const [key, def] of Object.entries(PREFS)) {
    if (!def.url || !prefs) continue;
    const v = def.toUrl(prefs[key], def.default);
    if (v != null) p.set(def.url, v);
  }
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
  for (const [key, def] of Object.entries(PREFS)) {
    if (def.apply && prefs[key] != null) def.apply(prefs[key], els);
  }
}

/** Stored prefs first, then `?` params when present. */
export function hydrateChromePrefs({ storage, search, els }) {
  applyPrefsToElements(readStoredPrefs(storage), els);
  applyPrefsToElements(prefsFromSearch(search), els);
}

/** Current value of every pref; unknown reads keep their `last` value. */
export function snapshotPrefs(ctx, last = {}) {
  const next = { ...last };
  for (const [key, def] of Object.entries(PREFS)) {
    const v = def.read(ctx);
    if (v !== undefined) next[key] = v;
  }
  return next;
}

/**
 * The one writer: snapshot every pref from `ctx` ({ app, els, storage,
 * location, history, touchPad }), and when something differs from `last`
 * write just the changed `store` keys and replace the share URL. Returns the
 * new snapshot (or `last` itself when nothing changed). Seed `last` with
 * `snapshotPrefs` so hydrating a shared link writes nothing.
 */
export function syncPrefs(ctx, last) {
  const next = snapshotPrefs(ctx, last);
  const changed = Object.keys(PREFS).filter((k) => next[k] !== last[k]);
  if (!changed.length) return last;
  const stored = {};
  for (const key of changed) if (PREFS[key].store) stored[key] = next[key];
  if (Object.keys(stored).length) writeStoredPrefs(ctx.storage, stored);
  if (changed.some((k) => PREFS[k].url)) {
    syncShareUrl(next, ctx.location, ctx.history);
  }
  return next;
}
