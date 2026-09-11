/**
 * Chrome live-region tests (no WASM, no jsdom).
 * Drive the exported helpers with a WASM-shaped stub and a #status node.
 */
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  applyDocumentVisibility,
  handleGameKeyDown,
  shouldAdvanceFrame,
  syncChromeFromApp,
  writeLiveRegion,
} from "./main.js";

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
function mockApp() {
  let demo = false;
  let hint = null;
  let frames = 0;
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
    },
    get demo_active() {
      return demo;
    },
    get sound_enabled() {
      return false;
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
      return `S1 · 500 rds · AUTO-REMOTE · SAFE · ${mode} · MUTE${extra}`;
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
    set_hidden(h) {
      this.hidden = h;
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

  assert.equal(shouldAdvanceFrame(true), false);
  const paused = applyDocumentVisibility(true, app, loopCtl);
  assert.equal(paused.paused, true);
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
