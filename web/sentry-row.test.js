import assert from "node:assert/strict";
import { test } from "node:test";
import { readSentry, sentryPresentation } from "./sentry-row.js";

const unit = {
  rounds: 500,
  capacity: 500,
  armed: false,
  online: true,
  linkOk: true,
  critical: false,
  selected: true,
};

test("overview copies WASM values and frees every snapshot", () => {
  let frees = 0;
  const app = {
    sentry_overview(index) {
      assert.equal(index, 2);
      return {
        ...unit,
        link_ok: false,
        free() {
          frees++;
        },
      };
    },
  };
  assert.deepEqual(readSentry(app, 2), { ...unit, linkOk: false });
  assert.equal(frees, 1);
  assert.equal(readSentry({ sentry_overview: () => undefined }, 9), null);
});

test("unit labels, ammo gauge and selection reflect per-unit telemetry", () => {
  const view = sentryPresentation(
    { ...unit, rounds: 125, armed: true },
    2,
    "fire",
    false,
  );
  assert.equal(view.percent, 25);
  assert.equal(view.weapon, "ARMED");
  assert.equal(view.selection, "SELECTED");
  assert.equal(view.disabled, false);
  assert.match(view.label, /Sentry C \(S3\), ARMED, 125 of 500 rounds/);
  assert.equal(
    sentryPresentation({ ...unit, selected: false }, 0, "options", false)
      .selection,
    "SELECT UNIT",
  );
});

test("faults remain visible even on armed units, with availability taking precedence", () => {
  const armed = { ...unit, armed: true };
  for (const [change, caption] of [
    [{ critical: true }, "AMMO CRITICAL"],
    [{ critical: true, rounds: 0 }, "EMPTY"],
    [{ linkOk: false, rounds: 0 }, "LINK DOWN"],
    [{ online: false, linkOk: false }, "OFFLINE"],
  ]) {
    const view = sentryPresentation({ ...armed, ...change }, 0, "fire", false);
    assert.equal(view.caption, caption);
    assert.equal(view.alert, true);
    assert.equal(view.weapon, "ARMED");
  }
});

test("POST and Quit disable selection; a fresh app re-enables it", () => {
  const boot = sentryPresentation(unit, 0, "boot", false);
  assert.equal(boot.disabled, true);
  assert.equal(boot.caption, "INITIALIZING");
  const quit = sentryPresentation(unit, 0, "fire", true);
  assert.equal(quit.disabled, true);
  assert.equal(quit.selection, "RESTART TO RESUME");
  assert.equal(sentryPresentation(unit, 0, "options", false).disabled, false);
});

test("row sync updates changed units, skips unchanged DOM writes, and resets on restart", async () => {
  const { createSentryRow } = await import("./sentry-row.js");
  const buttons = [];
  let writes = 0;
  let selectedByClick = null;
  const row = {
    scrollLeft: 0,
    append(button) { buttons.push(button); },
    getBoundingClientRect: () => ({ left: 0, right: 200 }),
  };
  const root = { hidden: true, querySelector: () => row };
  const originalDocument = globalThis.document;
  globalThis.document = {
    createElement() {
      const nodes = new Map();
      return {
        dataset: {},
        attributes: {},
        querySelector(selector) {
          if (!nodes.has(selector)) nodes.set(selector, { style: {}, dataset: {}, textContent: "" });
          return nodes.get(selector);
        },
        setAttribute(name, value) { writes++; this.attributes[name] = value; },
        addEventListener(name, handler) { this[name] = handler; },
        getBoundingClientRect: () => ({ left: 200, right: 400 }),
      };
    },
  };
  try {
    const units = Array.from({ length: 4 }, (_, i) => ({ ...unit, selected: i === 0 }));
    let screen = "options";
    let frees = 0;
    const app = {
      should_quit: false,
      screen_name: () => screen,
      sentry_overview: (i) => ({ ...units[i], link_ok: units[i].linkOk, free() { frees++; } }),
    };
    const controller = createSentryRow(root, (i) => { selectedByClick = i; });
    controller.sync(app);
    assert.equal(root.hidden, false);
    assert.equal(buttons[0].attributes["aria-pressed"], "true");
    const firstWrites = writes;
    controller.sync(app);
    assert.equal(writes, firstWrites);
    assert.equal(frees, 8);
    buttons[2].click();
    assert.equal(selectedByClick, 2);
    units[0].selected = false;
    units[2] = { ...units[2], selected: true, rounds: 125, armed: true };
    controller.sync(app);
    assert.equal(buttons[2].querySelector(".ammo-fill").style.width, "25%");
    assert.equal(buttons[0].querySelector(".rounds-value").textContent, 500);
    assert.equal(buttons[2].attributes["aria-pressed"], "true");
    assert.ok(row.scrollLeft > 0);
    app.should_quit = true;
    controller.sync(app);
    assert.ok(buttons.every((button) => button.disabled));
    controller.reset();
    assert.equal(root.hidden, true);
    app.should_quit = false;
    screen = "boot";
    controller.sync(app);
    assert.ok(buttons.every((button) => button.disabled));
    screen = "options";
    controller.sync(app);
    assert.ok(buttons.every((button) => !button.disabled));
  } finally {
    if (originalDocument === undefined) delete globalThis.document;
    else globalThis.document = originalDocument;
  }
});
