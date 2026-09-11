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
  handleGameKeyDown,
  handleSkipToPlaySurface,
  hydrateChromePrefs,
  persistChromeFromForm,
  prefsFromSearch,
  readStoredPrefs,
  refocusPlaySurface,
  shouldAdvanceFrame,
  syncChromeFromApp,
  writeLiveRegion,
  writeStoredPrefs,
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
  assert.deepEqual(prefsFromSearch("?demo=1"), {});
  assert.equal(prefsFromSearch("?sound=1").sound, true);
  assert.equal(prefsFromSearch("?boot=0").skipBoot, true);
  const els = chromeEls({ theme: "yellow", sound: false });
  applyPrefsToElements({ theme: "amber", sound: true }, els);
  assert.equal(els.theme.value, "amber");
  assert.equal(els.sound.checked, true);
  assert.equal(els.scale.value, "3");
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
