/**
 * Chrome live-region tests (no WASM, no jsdom).
 * Drive the exported helpers with a WASM-shaped stub and a #status node.
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  handleGameKeyDown,
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
