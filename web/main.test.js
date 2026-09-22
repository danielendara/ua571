/**
 * Chrome live-region tests (no WASM, no jsdom).
 * Drive the exported helpers with a WASM-shaped stub and a #status node.
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  applyDocumentVisibility,
  PREFS_STORAGE_KEY,
  applyPrefsToElements,
  bindPlaySurfaceRefocus,
  cycleThemeSelect,
  handleGameKeyDown,
  handleSkipToPlaySurface,
  handleThemeKeyDown,
  nextThemeValue,
  themeStatusHint,
  themeValuesFromSelect,
  hydrateChromePrefs,
  persistChromeFromForm,
  persistOptionsIfChanged,
  prefsFromSearch,
  searchFromPrefs,
  syncShareUrl,
  readStoredPrefs,
  refocusPlaySurface,
  shouldAdvanceFrame,
  syncChromeFromApp,
  writeLiveRegion,
  writeStoredPrefs,
  wasmLoadFailureStatus,
  startPage,
  applySoundChoice,
  boot,
  setBootInstanceLoaderForTests,
} from "./main.js";

const html = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "index.html"),
  "utf8"
);

function liveRegion(initial = "") {
  let text = initial;
  let writes = 0;
  return {
    id: "status",
    get textContent() {
      return text;
    },
    set textContent(value) {
      writes += 1;
      text = String(value);
    },
    writeCount() {
      return writes;
    },
  };
}

function canvasTarget() {
  return { closest: () => null };
}

/** Stub of ua571-web `Ua571Web` chrome getters + `KeyD` demo toggle. */
function mockApp(opts = {}) {
  let demo = false;
  let sound = false;
  let hint = null;
  let frames = 0;
  let link = opts.link ?? "LINK OK";
  return {
    frame() {
      frames += 1;
    },
    key_down(code, _repeat) {
      hint = null;
      if (code === "KeyD") {
        demo = !demo;
        hint = demo ? "Demo on" : "Demo off";
      }
      if (code === "KeyM") {
        sound = !sound;
        hint = sound ? "Sound on" : "Sound off";
      }
    },
    get demo_active() {
      return demo;
    },
    get sound_enabled() {
      return sound;
    },
    get should_quit() {
      return false;
    },
    screen_name() {
      return "options";
    },
    status_line() {
      const mode = demo ? "DEMO" : "MANUAL";
      const extra = hint ? ` · ${hint}` : "";
      const audio = sound ? "SND" : "MUTE";
      return `S1 · 500 rds · AUTO-REMOTE · SEARCH · SAFE · ${link} · ${mode} · ${audio}${extra}`;
    },
    frameCount() {
      return frames;
    },
  };
}

test("writeLiveRegion updates only when the text changes", () => {
  const status = liveRegion("OPTIONS · MANUAL");
  assert.equal(writeLiveRegion(status, "OPTIONS · MANUAL"), false);
  assert.equal(status.writeCount(), 0);
  assert.equal(writeLiveRegion(status, "OPTIONS · DEMO · Demo on"), true);
  assert.equal(status.textContent, "OPTIONS · DEMO · Demo on");
  assert.equal(status.writeCount(), 1);
  assert.equal(writeLiveRegion(status, "OPTIONS · DEMO · Demo on"), false);
  assert.equal(status.writeCount(), 1);
});

test("syncChromeFromApp writes #status only when chrome text changes", () => {
  const app = mockApp();
  const status = liveRegion("Loading WebAssembly…");
  const els = { status, demo: { checked: false }, sound: { checked: false } };

  assert.equal(syncChromeFromApp(app, els), true);
  assert.equal(status.writeCount(), 1);
  const first = status.textContent;
  assert.match(first, /^OPTIONS · /);
  assert.equal(app.frameCount(), 1);

  assert.equal(syncChromeFromApp(app, els), false);
  assert.equal(status.textContent, first);
  assert.equal(status.writeCount(), 1);
  assert.equal(app.frameCount(), 2);

  handleGameKeyDown(app, { code: "KeyD", repeat: false, target: canvasTarget() });
  assert.equal(syncChromeFromApp(app, els), true);
  assert.equal(status.writeCount(), 2);
  assert.notEqual(status.textContent, first);
});

test("hidden pauses/mutes; visible resumes without mutating game state", () => {
  const snapshot = { screen: "fire", rounds: 500, sound: true };
  const app = {
    hidden: false,
    soundPref: true,
    set_hidden(hidden) {
      this.hidden = hidden;
    },
    set_sound() {
      this.soundPref = !this.soundPref;
    },
  };
  let running = true;
  const loopCtl = {
    pause() {
      running = false;
    },
    resume() {
      running = true;
    },
  };

  const hidden = applyDocumentVisibility(true, app, loopCtl);
  assert.equal(hidden.paused, true);
  assert.equal(app.hidden, true);
  assert.equal(running, false);
  assert.equal(app.soundPref, true);
  assert.deepEqual(snapshot, { screen: "fire", rounds: 500, sound: true });

  const shown = applyDocumentVisibility(false, app, loopCtl);
  assert.equal(shown.paused, false);
  assert.equal(app.hidden, false);
  assert.equal(running, true);
  assert.equal(app.soundPref, true);
  assert.equal(shouldAdvanceFrame(false), true);
  assert.deepEqual(snapshot, { screen: "fire", rounds: 500, sound: true });
});

test("#status live region includes LINK OK / DOWN / OFFLINE", () => {
  for (const link of ["LINK OK", "LINK DOWN", "OFFLINE"]) {
    const app = mockApp({ link });
    const status = liveRegion("Loading WebAssembly…");
    const els = { status, demo: { checked: false }, sound: { checked: false } };
    syncChromeFromApp(app, els);
    assert.match(status.textContent, new RegExp(link));
    assert.match(status.textContent, /SEARCH/);
    assert.match(status.textContent, /MUTE/);
  }
});

test("skip-link in index.html targets the focusable canvas", () => {
  assert.match(html, /class="skip-link"/);
  assert.match(html, /href="#ua571"/);
  assert.match(html, /id="ua571"/);
  assert.match(html, /tabindex="0"/);
  const skipAt = html.indexOf('class="skip-link"');
  const canvasAt = html.indexOf('id="ua571"');
  assert.ok(skipAt >= 0 && skipAt < canvasAt, "skip-link must precede the canvas");
});

test("footer key legend mentions Esc toggle, matching pixel's help text (#94)", () => {
  const keysMatch = html.match(/<p class="keys">([\s\S]*?)<\/p>/);
  assert.ok(keysMatch, "expected a .keys legend in index.html");
  const legend = keysMatch[1];
  assert.match(legend, /<kbd>Esc<\/kbd>/);
  assert.match(legend.toLowerCase(), /esc<\/kbd>\s*toggle/);
});

test("footer key legend mentions T theme (#107)", () => {
  const keysMatch = html.match(/<p class="keys">([\s\S]*?)<\/p>/);
  assert.ok(keysMatch, "expected a .keys legend in index.html");
  const legend = keysMatch[1];
  assert.match(legend, /<kbd>T<\/kbd>/);
  assert.match(legend.toLowerCase(), /t<\/kbd>\s*theme/);
});

test("theme helpers cycle select options in Theme::ALL order", () => {
  const select = mockThemeSelect("yellow");
  assert.deepEqual(themeValuesFromSelect(select), [
    "yellow",
    "phosphor",
    "amber",
    "mono",
  ]);
  assert.equal(nextThemeValue("yellow", themeValuesFromSelect(select)), "phosphor");
  assert.equal(cycleThemeSelect(select), "phosphor");
  assert.equal(select.value, "phosphor");
  assert.equal(cycleThemeSelect(select), "amber");
  assert.equal(cycleThemeSelect(select), "mono");
  assert.equal(cycleThemeSelect(select), "yellow");
  assert.equal(themeStatusHint("amber"), "THEME AMBER");
  assert.equal(themeStatusHint("mono"), "THEME MONO");
});

test("KeyT cycles theme, persists, and shows THEME hint in #status (#107)", () => {
  const select = mockThemeSelect("yellow");
  const storage = memoryStorage();
  const status = liveRegion("OPTIONS · MANUAL");
  const readOpts = () => ({
    theme: select.value,
    scale: 3,
    sound: false,
    skipBoot: false,
  });

  const first = handleThemeKeyDown(
    { code: "KeyT", repeat: false, target: canvasTarget() },
    { select, storage, status, readOpts }
  );
  assert.deepEqual(first, { theme: "phosphor", hint: "THEME PHOSPHOR" });
  assert.equal(select.value, "phosphor");
  assert.equal(status.textContent, "THEME PHOSPHOR");
  assert.equal(readStoredPrefs(storage).theme, "phosphor");

  const second = handleThemeKeyDown(
    { code: "KeyT", repeat: false, target: canvasTarget() },
    { select, storage, status, readOpts }
  );
  assert.deepEqual(second, { theme: "amber", hint: "THEME AMBER" });
  assert.equal(status.textContent, "THEME AMBER");

  assert.equal(
    handleThemeKeyDown(
      { code: "KeyT", repeat: true, target: canvasTarget() },
      { select, storage, status, readOpts }
    ),
    false
  );
  assert.equal(select.value, "amber");

  assert.equal(
    handleThemeKeyDown(
      { code: "KeyT", repeat: false, target: { closest: () => ({}) } },
      { select, storage, status, readOpts }
    ),
    false
  );
  assert.equal(select.value, "amber");

  assert.equal(
    handleThemeKeyDown(
      { code: "KeyD", repeat: false, target: canvasTarget() },
      { select, storage, status, readOpts }
    ),
    false
  );

  // Clear the one-shot theme hint so later tests see normal chrome status.
  handleGameKeyDown(mockApp(), {
    code: "KeyD",
    repeat: false,
    target: canvasTarget(),
  });
});

test("skip-link focuses the canvas play surface", () => {
  let focused = false;
  let prevented = false;
  const canvas = {
    id: "ua571",
    focus() {
      focused = true;
    },
  };
  assert.equal(
    handleSkipToPlaySurface(
      {
        preventDefault() {
          prevented = true;
        },
      },
      canvas
    ),
    true
  );
  assert.equal(prevented, true);
  assert.equal(focused, true);
});

test("chrome change/button click refocuses the canvas", () => {
  let focused = 0;
  const canvas = {
    focus() {
      focused += 1;
    },
  };
  const listeners = {};
  const chromeRoot = {
    addEventListener(type, fn) {
      listeners[type] = fn;
    },
    removeEventListener(type) {
      delete listeners[type];
    },
  };

  const unbind = bindPlaySurfaceRefocus(chromeRoot, canvas);
  listeners.change();
  assert.equal(focused, 1);
  listeners.click({ target: { closest: (sel) => (sel === "button" ? {} : null) } });
  assert.equal(focused, 2);
  listeners.click({ target: { closest: () => null } });
  assert.equal(focused, 2, "select/checkbox click must not steal focus early");
  unbind();
  assert.equal(typeof listeners.change, "undefined");
});

test("refocusPlaySurface no-ops without a canvas", () => {
  assert.equal(refocusPlaySurface(null), false);
  assert.equal(refocusPlaySurface({}), false);
});

test("Demo on/off appears in #status after key d", () => {
  const app = mockApp();
  const status = liveRegion("Loading WebAssembly…");
  const demo = { checked: false };
  const els = { status, demo, sound: { checked: false } };

  syncChromeFromApp(app, els);
  assert.match(status.textContent, /MANUAL/);
  assert.equal(demo.checked, false);

  handleGameKeyDown(app, { code: "KeyD", repeat: false, target: canvasTarget() });
  syncChromeFromApp(app, els);
  assert.match(status.textContent, /Demo on/);
  assert.match(status.textContent, /DEMO/);
  assert.equal(demo.checked, true);

  handleGameKeyDown(app, { code: "KeyD", repeat: false, target: canvasTarget() });
  syncChromeFromApp(app, els);
  assert.match(status.textContent, /Demo off/);
  assert.match(status.textContent, /MANUAL/);
  assert.doesNotMatch(status.textContent, /Demo on/);
  assert.equal(demo.checked, false);
});

function memoryStorage(seed) {
  const map = { ...(seed || {}) };
  return {
    getItem(k) {
      return Object.prototype.hasOwnProperty.call(map, k) ? map[k] : null;
    },
    setItem(k, v) {
      map[k] = String(v);
    },
    map,
  };
}

function chromeEls(values = {}) {
  return {
    theme: { value: values.theme ?? "yellow" },
    scale: { value: values.scale ?? "3" },
    sound: { checked: values.sound ?? false },
    skipBoot: { checked: values.skipBoot ?? false },
    demo: { checked: values.demo ?? false },
  };
}

function mockLocation(search = "", pathname = "/", hash = "") {
  return { pathname, search, hash };
}

function mockHistory(loc) {
  const hist = {
    state: null,
    urls: [],
    replaceState(state, _title, url) {
      hist.state = state;
      hist.urls.push(String(url));
      const raw = String(url);
      const hashAt = raw.indexOf("#");
      loc.hash = hashAt >= 0 ? raw.slice(hashAt) : "";
      const noHash = hashAt >= 0 ? raw.slice(0, hashAt) : raw;
      const qAt = noHash.indexOf("?");
      loc.pathname = qAt >= 0 ? noHash.slice(0, qAt) : noHash;
      loc.search = qAt >= 0 ? noHash.slice(qAt) : "";
    },
  };
  return hist;
}

function mockThemeSelect(current = "yellow") {
  const values = ["yellow", "phosphor", "amber", "mono"];
  let value = current;
  return {
    options: values.map((v) => ({ value: v })),
    get value() {
      return value;
    },
    set value(v) {
      value = v;
    },
  };
}

test("persistChromeFromForm round-trips theme/scale/sound/skipBoot", () => {
  const storage = memoryStorage();
  persistChromeFromForm(storage, {
    theme: "amber",
    scale: 4,
    sound: true,
    skipBoot: true,
    demo: true,
  });
  const stored = readStoredPrefs(storage);
  assert.equal(stored.theme, "amber");
  assert.equal(stored.scale, "4");
  assert.equal(stored.sound, true);
  assert.equal(stored.skipBoot, true);
  assert.equal(stored.demo, undefined);
  assert.doesNotMatch(storage.map[PREFS_STORAGE_KEY], /demo/);
});

test("readStoredPrefs/writeStoredPrefs round-trip systemMode/weaponStatus/iffStatus", () => {
  const storage = memoryStorage();
  writeStoredPrefs(storage, {
    systemMode: "ManOverride",
    weaponStatus: "Armed",
    iffStatus: "Engaged",
  });
  const stored = readStoredPrefs(storage);
  assert.equal(stored.systemMode, "ManOverride");
  assert.equal(stored.weaponStatus, "Armed");
  assert.equal(stored.iffStatus, "Engaged");
});

test("readStoredPrefs ignores a non-string/empty option value", () => {
  const storage = memoryStorage({
    [PREFS_STORAGE_KEY]: JSON.stringify({ systemMode: "", weaponStatus: 5 }),
  });
  const stored = readStoredPrefs(storage);
  assert.equal(stored.systemMode, undefined);
  assert.equal(stored.weaponStatus, undefined);
});

test("persistOptionsIfChanged writes only when a value actually changes", () => {
  const storage = memoryStorage();
  let last = { systemMode: null, weaponStatus: null, iffStatus: null };
  const app = { system_mode: "AutoRemote", weapon_status: "Safe", iff_status: "Search" };

  last = persistOptionsIfChanged(app, storage, last);
  assert.deepEqual(readStoredPrefs(storage), {
    systemMode: "AutoRemote",
    weaponStatus: "Safe",
    iffStatus: "Search",
  });

  // No change — re-running must not re-serialize (nothing to assert on the
  // write itself here, but `last` must stay referentially the same object
  // the caller already has, matching the "only on actual change" contract).
  const unchanged = persistOptionsIfChanged(app, storage, last);
  assert.equal(unchanged, last);

  app.weapon_status = "Armed";
  last = persistOptionsIfChanged(app, storage, last);
  assert.equal(readStoredPrefs(storage).weaponStatus, "Armed");
  assert.equal(readStoredPrefs(storage).systemMode, "AutoRemote");
});

test("persistOptionsIfChanged is a no-op without an app", () => {
  const storage = memoryStorage();
  const last = { systemMode: null, weaponStatus: null, iffStatus: null };
  assert.equal(persistOptionsIfChanged(null, storage, last), last);
  assert.equal(storage.map[PREFS_STORAGE_KEY], undefined);
});

test("hydrateChromePrefs restores storage then query params win", () => {
  const storage = memoryStorage();
  writeStoredPrefs(storage, {
    theme: "phosphor",
    scale: "2",
    sound: true,
    skipBoot: true,
  });

  const restored = chromeEls();
  hydrateChromePrefs({ storage, search: "", els: restored });
  assert.equal(restored.theme.value, "phosphor");
  assert.equal(restored.scale.value, "2");
  assert.equal(restored.sound.checked, true);
  assert.equal(restored.skipBoot.checked, true);

  const overridden = chromeEls();
  hydrateChromePrefs({
    storage,
    search: "?theme=mono&scale=4&sound=0&boot=1",
    els: overridden,
  });
  assert.equal(overridden.theme.value, "mono");
  assert.equal(overridden.scale.value, "4");
  assert.equal(overridden.sound.checked, false);
  assert.equal(overridden.skipBoot.checked, false);
});

test("prefsFromSearch only sets keys that are present", () => {
  assert.deepEqual(prefsFromSearch(""), {});
  assert.deepEqual(prefsFromSearch("?demo=1"), { demo: true });
  assert.equal(prefsFromSearch("?sound=1").sound, true);
  assert.equal(prefsFromSearch("?sound=0").sound, false);
  assert.equal(prefsFromSearch("?boot=0").skipBoot, true);
  const els = chromeEls({ theme: "yellow", sound: false });
  applyPrefsToElements({ theme: "amber", sound: true, demo: true }, els);
  assert.equal(els.theme.value, "amber");
  assert.equal(els.sound.checked, true);
  assert.equal(els.demo.checked, true);
  assert.equal(els.scale.value, "3");
});

test("searchFromPrefs omits defaults and round-trips prefsFromSearch (#110)", () => {
  assert.equal(searchFromPrefs({}), "");
  assert.equal(
    searchFromPrefs({
      theme: "yellow",
      scale: 3,
      sound: false,
      skipBoot: false,
      demo: false,
    }),
    ""
  );
  assert.equal(searchFromPrefs({ theme: "amber" }), "?theme=amber");
  assert.equal(searchFromPrefs({ scale: 4 }), "?scale=4");
  assert.equal(searchFromPrefs({ sound: true }), "?sound=1");
  assert.equal(searchFromPrefs({ sound: false }), "");
  assert.equal(searchFromPrefs({ skipBoot: true }), "?boot=0");
  assert.equal(searchFromPrefs({ demo: true }), "?demo=1");
  assert.equal(
    searchFromPrefs({
      theme: "mono",
      scale: "4",
      sound: true,
      skipBoot: true,
      demo: true,
    }),
    "?theme=mono&scale=4&sound=1&boot=0&demo=1"
  );

  const share = "?theme=amber&scale=4&sound=1&boot=0&demo=1";
  assert.deepEqual(prefsFromSearch(share), {
    theme: "amber",
    scale: "4",
    sound: true,
    skipBoot: true,
    demo: true,
  });
  assert.equal(searchFromPrefs(prefsFromSearch(share)), share);
  // Inbound `sound=0` means off; outbound omits the muted default.
  assert.equal(prefsFromSearch("?sound=0").sound, false);
  assert.equal(searchFromPrefs(prefsFromSearch("?sound=0")), "");
});

test("KeyT writeback puts theme in the URL and still persists LS (#110)", () => {
  const select = mockThemeSelect("yellow");
  const storage = memoryStorage();
  const status = liveRegion("OPTIONS · MANUAL");
  const loc = mockLocation("");
  const hist = mockHistory(loc);
  const readOpts = () => ({
    theme: select.value,
    scale: 3,
    sound: false,
    skipBoot: false,
    demo: false,
  });

  handleThemeKeyDown(
    { code: "KeyT", repeat: false, target: canvasTarget() },
    { select, storage, status, readOpts, location: loc, history: hist }
  );
  assert.equal(select.value, "phosphor");
  assert.equal(readStoredPrefs(storage).theme, "phosphor");
  assert.match(loc.search, /theme=phosphor/);
  assert.equal(hist.urls.length, 1);

  handleGameKeyDown(mockApp(), {
    code: "KeyD",
    repeat: false,
    target: canvasTarget(),
  });
});

test("syncShareUrl writes sound/demo/boot snapshot without rAF or LS demo (#110)", () => {
  const storage = memoryStorage();
  persistChromeFromForm(storage, {
    theme: "yellow",
    scale: 3,
    sound: true,
    skipBoot: false,
    demo: true,
  });
  assert.equal(readStoredPrefs(storage).demo, undefined);
  assert.doesNotMatch(storage.map[PREFS_STORAGE_KEY], /demo/);
  assert.equal(readStoredPrefs(storage).sound, true);

  const loc = mockLocation("");
  const hist = mockHistory(loc);
  const opts = {
    theme: "yellow",
    scale: 3,
    sound: true,
    skipBoot: false,
    demo: true,
  };
  assert.equal(syncShareUrl(opts, loc, hist), "?sound=1&demo=1");
  assert.equal(loc.search, "?sound=1&demo=1");

  opts.sound = false;
  opts.demo = false;
  assert.equal(syncShareUrl(opts, loc, hist), "");
  assert.equal(loc.search, "");
  assert.equal(readStoredPrefs(storage).demo, undefined);

  opts.skipBoot = true;
  assert.equal(syncShareUrl(opts, loc, hist), "?boot=0");
  assert.match(loc.search, /boot=0/);
  assert.doesNotMatch(loc.search, /sound=/);

  // Unchanged URL must not replaceState again.
  const calls = hist.urls.length;
  assert.equal(syncShareUrl(opts, loc, hist), "?boot=0");
  assert.equal(hist.urls.length, calls);
});

test("URL chrome still wins over LS; later writeback is the new snapshot (#110)", () => {
  const storage = memoryStorage();
  writeStoredPrefs(storage, {
    theme: "phosphor",
    scale: "2",
    sound: true,
    skipBoot: true,
  });
  const els = chromeEls();
  hydrateChromePrefs({
    storage,
    search: "?theme=mono&scale=4",
    els,
  });
  assert.equal(els.theme.value, "mono");
  assert.equal(els.scale.value, "4");
  assert.equal(els.sound.checked, true);
  assert.equal(els.skipBoot.checked, true);

  const loc = mockLocation("?theme=mono&scale=4");
  const hist = mockHistory(loc);
  const next = {
    theme: els.theme.value,
    scale: els.scale.value,
    sound: false,
    skipBoot: els.skipBoot.checked,
    demo: false,
  };
  persistChromeFromForm(storage, next);
  syncShareUrl(next, loc, hist);
  assert.equal(loc.search, "?theme=mono&scale=4&boot=0");
  assert.doesNotMatch(loc.search, /sound=/);
  assert.equal(readStoredPrefs(storage).theme, "mono");
  assert.equal(readStoredPrefs(storage).sound, false);
  assert.equal(readStoredPrefs(storage).demo, undefined);
});

test("Sound on/off appears in #status after key m once", () => {
  const app = mockApp();
  const status = liveRegion("Loading WebAssembly…");
  const sound = { checked: false };
  const els = { status, demo: { checked: false }, sound };

  syncChromeFromApp(app, els);
  assert.match(status.textContent, /MUTE/);
  assert.doesNotMatch(status.textContent, /Sound on|Sound off/);
  assert.equal(sound.checked, false);
  const writesAfterIdle = status.writeCount();

  handleGameKeyDown(app, { code: "KeyM", repeat: false, target: canvasTarget() });
  syncChromeFromApp(app, els);
  assert.match(status.textContent, /Sound on/);
  assert.match(status.textContent, /SND/);
  assert.equal(sound.checked, true);
  assert.equal(status.writeCount(), writesAfterIdle + 1);

  // Same chrome text is not rewritten every frame (no live-region chatter).
  syncChromeFromApp(app, els);
  syncChromeFromApp(app, els);
  assert.equal(status.writeCount(), writesAfterIdle + 1);

  handleGameKeyDown(app, { code: "KeyM", repeat: false, target: canvasTarget() });
  syncChromeFromApp(app, els);
  assert.match(status.textContent, /Sound off/);
  assert.match(status.textContent, /MUTE/);
  assert.doesNotMatch(status.textContent, /Sound on/);
  assert.equal(sound.checked, false);
});

function installBootDom() {
  const listeners = { keydown: [], keyup: [], visibilitychange: [] };
  const els = {};
  function makeEl(id, extra = {}) {
    const node = {
      id,
      value: extra.value ?? "",
      checked: Boolean(extra.checked),
      textContent: extra.textContent ?? "",
      dataset: {},
      focusCount: 0,
      focus() {
        this.focusCount += 1;
      },
      addEventListener() {},
      removeEventListener() {},
    };
    els[id] = node;
    return node;
  }
  makeEl("status", { textContent: "Loading WebAssembly…" });
  makeEl("ua571");
  makeEl("theme", { value: "yellow" });
  makeEl("scale", { value: "3" });
  makeEl("demo");
  makeEl("skipBoot");
  makeEl("sound");

  const prev = {
    document: globalThis.document,
    window: globalThis.window,
    requestAnimationFrame: globalThis.requestAnimationFrame,
    cancelAnimationFrame: globalThis.cancelAnimationFrame,
  };
  const rafs = [];
  let nextRaf = 1;
  globalThis.document = {
    body: { dataset: {} },
    hidden: false,
    getElementById(id) {
      return els[id] || null;
    },
    addEventListener(type, fn) {
      (listeners[type] ||= []).push(fn);
    },
    removeEventListener(type, fn) {
      const list = listeners[type] || [];
      const i = list.indexOf(fn);
      if (i >= 0) list.splice(i, 1);
    },
  };
  globalThis.window = {
    addEventListener(type, fn) {
      (listeners[type] ||= []).push(fn);
    },
    removeEventListener(type, fn) {
      const list = listeners[type] || [];
      const i = list.indexOf(fn);
      if (i >= 0) list.splice(i, 1);
    },
  };
  globalThis.requestAnimationFrame = (cb) => {
    const id = nextRaf++;
    rafs.push({ id, cb });
    return id;
  };
  globalThis.cancelAnimationFrame = (id) => {
    const i = rafs.findIndex((r) => r.id === id);
    if (i >= 0) rafs.splice(i, 1);
  };
  return {
    els,
    listeners,
    rafs,
    restore() {
      if (prev.document === undefined) delete globalThis.document;
      else globalThis.document = prev.document;
      if (prev.window === undefined) delete globalThis.window;
      else globalThis.window = prev.window;
      if (prev.requestAnimationFrame === undefined) delete globalThis.requestAnimationFrame;
      else globalThis.requestAnimationFrame = prev.requestAnimationFrame;
      if (prev.cancelAnimationFrame === undefined) delete globalThis.cancelAnimationFrame;
      else globalThis.cancelAnimationFrame = prev.cancelAnimationFrame;
    },
  };
}

function wasmStandIn(id) {
  const inst = {
    id,
    freed: false,
    keys: [],
    frames: 0,
    free() {
      this.freed = true;
    },
    key_down(code) {
      this.keys.push(code);
    },
    key_up() {},
    frame() {
      this.frames += 1;
    },
    screen_name() {
      return "fire";
    },
    status_line() {
      return "READY";
    },
    get sound_enabled() {
      return false;
    },
    get demo_active() {
      return false;
    },
    get should_quit() {
      return false;
    },
    get system_mode() {
      return "";
    },
    get weapon_status() {
      return "";
    },
    get iff_status() {
      return "";
    },
  };
  return inst;
}

test("overlapping boots free the older instance and leave one key listener pair", async () => {
  const dom = installBootDom();
  const gates = [];
  const instances = [];
  setBootInstanceLoaderForTests(
    () =>
      new Promise((resolve) => {
        const inst = wasmStandIn(instances.length + 1);
        instances.push(inst);
        gates.push(() => resolve(inst));
      })
  );
  try {
    const first = boot();
    const second = boot();
    gates[0]();
    await first;
    assert.equal(instances[0].freed, true);
    assert.equal(dom.listeners.keydown.length, 0);
    gates[1]();
    await second;
    assert.equal(instances[1].freed, false);
    assert.equal(dom.listeners.keydown.length, 1);
    assert.equal(dom.listeners.keyup.length, 1);

    dom.listeners.keydown[0]({
      code: "KeyF",
      repeat: false,
      target: { closest: () => null },
    });
    assert.deepEqual(instances[0].keys, []);
    assert.deepEqual(instances[1].keys, ["KeyF"]);
  } finally {
    setBootInstanceLoaderForTests(null);
    dom.restore();
  }
});

test("an older boot that finishes last frees its instance and does not stack listeners", async () => {
  const dom = installBootDom();
  const gates = [];
  const instances = [];
  setBootInstanceLoaderForTests(
    () =>
      new Promise((resolve) => {
        const inst = wasmStandIn(instances.length + 1);
        instances.push(inst);
        gates.push(() => resolve(inst));
      })
  );
  try {
    const first = boot();
    const second = boot();
    gates[1]();
    await second;
    assert.equal(instances[1].freed, false);
    assert.equal(dom.listeners.keydown.length, 1);
    gates[0]();
    await first;
    assert.equal(instances[0].freed, true);
    assert.equal(instances[1].freed, false);
    assert.equal(dom.listeners.keydown.length, 1);
    assert.equal(dom.listeners.keyup.length, 1);
    dom.listeners.keydown[0]({
      code: "KeyM",
      repeat: false,
      target: { closest: () => null },
    });
    assert.deepEqual(instances[0].keys, []);
    assert.deepEqual(instances[1].keys, ["KeyM"]);
  } finally {
    setBootInstanceLoaderForTests(null);
    dom.restore();
  }
});

test("a single boot focuses the canvas and runs the frame loop", async () => {
  const dom = installBootDom();
  const inst = wasmStandIn(1);
  setBootInstanceLoaderForTests(async () => inst);
  try {
    await boot();
    assert.equal(inst.freed, false);
    assert.equal(dom.els.ua571.focusCount, 1);
    assert.equal(dom.listeners.keydown.length, 1);
    assert.equal(dom.listeners.keyup.length, 1);
    assert.equal(dom.rafs.length, 1);
    dom.rafs[0].cb();
    assert.equal(inst.frames, 1);
    assert.match(dom.els.status.textContent, /FIRE · READY/);
  } finally {
    setBootInstanceLoaderForTests(null);
    dom.restore();
  }
});

test("a failed boot shows the load-failure status and does not attach game listeners", async () => {
  const dom = installBootDom();
  const errors = [];
  const orig = console.error;
  console.error = (...args) => {
    errors.push(args);
  };
  setBootInstanceLoaderForTests(async () => {
    throw new Error("wasm missing");
  });
  try {
    await boot();
    assert.equal(dom.listeners.keydown.length, 0);
    assert.equal(dom.listeners.keyup.length, 0);
    assert.equal(dom.rafs.length, 0);
    assert.equal(dom.els.ua571.focusCount, 0);
    assert.match(dom.els.status.textContent, /Console did not load/);
    assert.match(dom.els.status.textContent, /Restart/);
    assert.doesNotMatch(dom.els.status.textContent, /build-web\.sh/);
    assert.equal(errors.length, 1);
    assert.doesNotMatch(dom.els.status.textContent, /Loading WebAssembly/);
  } finally {
    console.error = orig;
    setBootInstanceLoaderForTests(null);
    dom.restore();
  }
});

function mockSoundApp(unlock) {
  let sound = false;
  let hint = null;
  return {
    set_sound(on) {
      if (on !== sound) {
        sound = on;
        hint = on ? "Sound on" : "Sound off";
      }
    },
    sound_did_not_start() {
      sound = false;
      hint = "Sound did not start";
    },
    unlock_audio: unlock,
    get sound_enabled() {
      return sound;
    },
    screen_name() {
      return "options";
    },
    get should_quit() {
      return false;
    },
    status_line() {
      const audio = sound ? "SND" : "MUTE";
      return hint ? `S1 · ${audio} · ${hint}` : `S1 · ${audio}`;
    },
  };
}

test("turning sound on keeps Sound on when resume resolves", async () => {
  const app = mockSoundApp(async () => true);
  const status = liveRegion("Loading WebAssembly…");
  const sound = { checked: true };
  const result = await applySoundChoice(app, true, { sound, status });
  assert.equal(result.started, true);
  assert.equal(result.on, true);
  assert.equal(sound.checked, true);
  assert.equal(app.sound_enabled, true);
  assert.match(status.textContent, /Sound on/);
  assert.match(status.textContent, /SND/);
  assert.doesNotMatch(status.textContent, /did not start/);
});

test("AudioContext construction failure leaves sound off", async () => {
  const app = mockSoundApp(async () => false);
  const status = liveRegion("S1 · MUTE");
  const sound = { checked: true };
  await applySoundChoice(app, true, { sound, status });
  assert.equal(sound.checked, false);
  assert.equal(app.sound_enabled, false);
  assert.match(status.textContent, /did not start/i);
  assert.match(status.textContent, /MUTE/);
  assert.doesNotMatch(status.textContent, /Sound on/);
  assert.doesNotMatch(status.textContent, /Sound off/);
});

test("a rejected resume() leaves sound off and says it did not start", async () => {
  const app = mockSoundApp(async () => {
    throw new Error("resume rejected");
  });
  const status = liveRegion("S1 · MUTE");
  const sound = { checked: true };
  await applySoundChoice(app, true, { sound, status });
  assert.equal(sound.checked, false);
  assert.equal(app.sound_enabled, false);
  assert.match(status.textContent, /did not start/i);
  assert.match(status.textContent, /MUTE/);
  assert.doesNotMatch(status.textContent, /Sound on/);
  assert.doesNotMatch(status.textContent, /Sound off/);
});

test("unchecking Sound mutes and shows Sound off", async () => {
  const app = mockSoundApp(async () => true);
  const status = liveRegion("");
  const sound = { checked: true };
  await applySoundChoice(app, true, { sound, status });
  await applySoundChoice(app, false, { sound, status });
  assert.equal(sound.checked, false);
  assert.equal(app.sound_enabled, false);
  assert.match(status.textContent, /Sound off/);
  assert.match(status.textContent, /MUTE/);
  assert.doesNotMatch(status.textContent, /did not start/);
  assert.doesNotMatch(status.textContent, /Sound on/);
});

test("narrow chrome CSS wraps controls at 480px without overflow", () => {
  const css = readFileSync(new URL("./style.css", import.meta.url), "utf8");
  assert.match(css, /@media \(max-width:\s*480px\)/);
  const block = css.split(/@media \(max-width:\s*480px\)/)[1] || "";
  assert.match(block, /overflow-x:\s*hidden/);
  assert.match(block, /#ua571/);
  assert.match(block, /width:\s*100%/);
  assert.match(block, /grid-template-columns/);
  assert.match(block, /\.controls/);
  assert.match(block, /\.status/);
  assert.doesNotMatch(css, /overflow-x:\s*scroll/);
});

test("wasm load failure text is player-facing except on localhost", () => {
  assert.match(html, /id="status"/);
  assert.match(html, /role="status"/);
  assert.equal((html.match(/id="restart"/g) || []).length, 1);

  for (const host of ["ua571.danielendara.com", "example.com", ""]) {
    const text = wasmLoadFailureStatus(host);
    assert.match(text, /Restart/);
    assert.match(text, /did not load/i);
    assert.doesNotMatch(text, /build-web\.sh/);
  }
  for (const host of ["localhost", "127.0.0.1"]) {
    const text = wasmLoadFailureStatus(host);
    assert.match(text, /Restart/);
    assert.match(text, /\.\/scripts\/build-web\.sh/);
  }
});

function installBootPageDom() {
  const listeners = { keydown: [], keyup: [] };
  const els = {};
  function makeEl(id, extra = {}) {
    const handlers = [];
    const node = {
      id,
      value: extra.value ?? "",
      checked: Boolean(extra.checked),
      textContent: extra.textContent ?? "",
      dataset: {},
      hidden: extra.hidden ?? false,
      href: "",
      focus() {},
      addEventListener(type, fn) {
        handlers.push({ type, fn });
      },
      removeEventListener() {},
      click() {
        for (const h of handlers) if (h.type === "click") h.fn();
      },
    };
    els[id] = node;
    return node;
  }
  makeEl("status", { textContent: "Loading WebAssembly…" });
  makeEl("ua571");
  makeEl("theme", { value: "yellow" });
  makeEl("scale", { value: "3" });
  makeEl("demo");
  makeEl("skipBoot");
  makeEl("sound");
  makeEl("restart");
  makeEl("app-version-wrap", { hidden: true });
  makeEl("app-version");

  const prev = {
    document: globalThis.document,
    window: globalThis.window,
    location: globalThis.location,
    requestAnimationFrame: globalThis.requestAnimationFrame,
    cancelAnimationFrame: globalThis.cancelAnimationFrame,
  };
  const rafs = [];
  let nextRaf = 1;
  globalThis.document = {
    body: { dataset: {} },
    hidden: false,
    getElementById(id) {
      return els[id] || null;
    },
    querySelector() {
      return null;
    },
    addEventListener() {},
    removeEventListener() {},
  };
  globalThis.window = {
    addEventListener(type, fn) {
      (listeners[type] ||= []).push(fn);
    },
    removeEventListener(type, fn) {
      const list = listeners[type] || [];
      const i = list.indexOf(fn);
      if (i >= 0) list.splice(i, 1);
    },
    location: null,
    history: { replaceState() {}, state: null },
  };
  globalThis.requestAnimationFrame = (cb) => {
    const id = nextRaf++;
    rafs.push({ id, cb });
    return id;
  };
  globalThis.cancelAnimationFrame = (id) => {
    const i = rafs.findIndex((r) => r.id === id);
    if (i >= 0) rafs.splice(i, 1);
  };
  return {
    els,
    listeners,
    rafs,
    setHost(hostname) {
      globalThis.location = {
        hostname,
        search: "",
        pathname: "/",
        hash: "",
      };
      globalThis.window.location = globalThis.location;
    },
    restore() {
      const put = (key, value) => {
        if (value === undefined) delete globalThis[key];
        else globalThis[key] = value;
      };
      put("document", prev.document);
      put("window", prev.window);
      put("location", prev.location);
      put("requestAnimationFrame", prev.requestAnimationFrame);
      put("cancelAnimationFrame", prev.cancelAnimationFrame);
    },
  };
}

function bootStandIn() {
  return {
    free() {},
    key_down() {},
    key_up() {},
    frame() {},
    screen_name() {
      return "fire";
    },
    status_line() {
      return "READY";
    },
    get sound_enabled() {
      return false;
    },
    get demo_active() {
      return false;
    },
    get should_quit() {
      return false;
    },
    get system_mode() {
      return "";
    },
    get weapon_status() {
      return "";
    },
    get iff_status() {
      return "";
    },
  };
}

async function bootFailure(hostname) {
  const dom = installBootPageDom();
  dom.setHost(hostname);
  const errors = [];
  const orig = console.error;
  console.error = (...args) => {
    errors.push(args);
  };
  setBootInstanceLoaderForTests(async () => {
    throw new Error("wasm missing");
  });
  try {
    await boot();
    return { dom, errors };
  } finally {
    console.error = orig;
  }
}

test("a rejected boot on a public host mentions Restart and not the build script", async () => {
  const { dom, errors } = await bootFailure("ua571.danielendara.com");
  try {
    assert.match(dom.els.status.textContent, /Restart/);
    assert.match(dom.els.status.textContent, /did not load/i);
    assert.doesNotMatch(dom.els.status.textContent, /build-web\.sh/);
    assert.doesNotMatch(dom.els.status.textContent, /Loading WebAssembly/);
    assert.equal(errors.length, 1);
    assert.equal(dom.listeners.keydown?.length || 0, 0);
  } finally {
    setBootInstanceLoaderForTests(null);
    dom.restore();
  }
});

test("a rejected boot on localhost still mentions the build script", async () => {
  const { dom, errors } = await bootFailure("localhost");
  try {
    assert.match(dom.els.status.textContent, /Restart/);
    assert.match(dom.els.status.textContent, /\.\/scripts\/build-web\.sh/);
    assert.doesNotMatch(dom.els.status.textContent, /Loading WebAssembly/);
    assert.equal(errors.length, 1);
  } finally {
    setBootInstanceLoaderForTests(null);
    dom.restore();
  }
});

test("Restart after a failed load calls boot again and success replaces the failure", async () => {
  const dom = installBootPageDom();
  dom.setHost("ua571.danielendara.com");
  let calls = 0;
  const orig = console.error;
  console.error = () => {};
  setBootInstanceLoaderForTests(async () => {
    calls += 1;
    if (calls === 1) throw new Error("wasm missing");
    return bootStandIn();
  });
  try {
    startPage();
    await new Promise((resolve) => setTimeout(resolve, 0));
    assert.match(dom.els.status.textContent, /Restart/);
    assert.doesNotMatch(dom.els.status.textContent, /build-web\.sh/);
    assert.equal(calls, 1);

    dom.els.restart.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    assert.equal(calls, 2);
    assert.ok(dom.rafs.length >= 1);
    dom.rafs[dom.rafs.length - 1].cb();
    assert.match(dom.els.status.textContent, /FIRE · READY/);
    assert.doesNotMatch(dom.els.status.textContent, /did not load/i);
    assert.doesNotMatch(dom.els.status.textContent, /Loading WebAssembly/);
  } finally {
    console.error = orig;
    setBootInstanceLoaderForTests(null);
    dom.restore();
  }
});
