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
  bindPlaySurfaceRefocus,
  handleGameKeyDown,
  handleSkipToPlaySurface,
  refocusPlaySurface,
  syncChromeFromApp,
  writeLiveRegion,
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
