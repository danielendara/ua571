/**
 * Boots the UA 571-C WASM module against the page canvas.
 * Expects `./pkg/ua571_web.js` from `scripts/build-web.sh`.
 */

let raf = 0;
let app = null;
let onKey = null;

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

    app = new wasm.Ua571Web(
      "ua571",
      opts.theme,
      opts.scale,
      opts.demo,
      opts.skipBoot,
      opts.sound
    );

    onKey = (e) => {
      switch (e.code) {
        case "ArrowUp":
        case "ArrowDown":
        case "ArrowLeft":
        case "ArrowRight":
        case "Space":
          e.preventDefault();
          break;
        default:
          break;
      }
      if (app) app.key_down(e.code);
    };
    window.addEventListener("keydown", onKey);

    const loop = () => {
      if (!app) return;
      app.frame();
      const soundBox = document.getElementById("sound");
      if (soundBox) soundBox.checked = app.sound_enabled;
      status.textContent = `${app.screen_name().toUpperCase()} · ${app.status_line()}`;
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);

    canvas.focus();
  } catch (err) {
    console.error(err);
    status.textContent =
      "Failed to load WASM. Run: ./scripts/build-web.sh  then serve the web/ folder.";
  }
}

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

document.getElementById("sound").addEventListener("change", (e) => {
  if (app) {
    app.set_sound(e.target.checked);
    document.getElementById("ua571").focus();
  }
});

// Optional deep-link query params
(() => {
  const p = new URLSearchParams(location.search);
  if (p.get("theme")) document.getElementById("theme").value = p.get("theme");
  if (p.get("scale")) document.getElementById("scale").value = p.get("scale");
  if (p.get("demo") === "1") document.getElementById("demo").checked = true;
  if (p.get("sound") === "1") document.getElementById("sound").checked = true;
  if (p.get("boot") === "0") document.getElementById("skipBoot").checked = true;
  applyPageTheme(document.getElementById("theme").value);
})();

boot();
