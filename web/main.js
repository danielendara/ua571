/**
 * Boots the UA 571-C WASM module against the page canvas.
 * Expects `./pkg/ua571_web.js` from `scripts/build-web.sh`.
 * Chrome helpers are exported for Node tests (no WASM).
 */

let raf = 0;
let app = null;
let onKey = null;
let onKeyUp = null;

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

export function isChromeTarget(el) {
  return Boolean(
    el &&
      el.closest &&
      el.closest("input, select, button, a, label, textarea")
  );
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
  return writeLiveRegion(els.status, formatChromeStatus(app));
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

    const loop = () => {
      if (!app) return;
      syncChromeFromApp(app, {
        status,
        demo: document.getElementById("demo"),
        sound: document.getElementById("sound"),
      });
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);

    refocusPlaySurface(canvas);
  } catch (err) {
    console.error(err);
    status.textContent =
      "Failed to load WASM. Run: ./scripts/build-web.sh  then serve the web/ folder.";
  }
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
    applyPageTheme(e.target.value);
    boot();
  });
  document.getElementById("scale").addEventListener("change", () => {
    boot();
  });

  document.getElementById("sound").addEventListener("change", async (e) => {
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
    boot();
  });

  // Optional deep-link query params
  const p = new URLSearchParams(location.search);
  if (p.get("theme")) document.getElementById("theme").value = p.get("theme");
  if (p.get("scale")) document.getElementById("scale").value = p.get("scale");
  if (p.get("demo") === "1") document.getElementById("demo").checked = true;
  if (p.get("sound") === "1") document.getElementById("sound").checked = true;
  if (p.get("boot") === "0") document.getElementById("skipBoot").checked = true;
  applyPageTheme(document.getElementById("theme").value);

  boot();
}

if (typeof document !== "undefined") {
  startPage();
}
