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
  };
}

function teardown() {
  if (raf) cancelAnimationFrame(raf);
  raf = 0;
  if (onKey) {
    window.removeEventListener("keydown", onKey);
    onKey = null;
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
    const wasm = await import("./pkg/ua571_web.js");
    await wasm.default();

    app = new wasm.Ua571Web(
      "ua571",
      opts.theme,
      opts.scale,
      opts.demo,
      opts.skipBoot
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

document.getElementById("theme").addEventListener("change", (e) => {
  applyPageTheme(e.target.value);
});

// Optional deep-link query params
(() => {
  const p = new URLSearchParams(location.search);
  if (p.get("theme")) document.getElementById("theme").value = p.get("theme");
  if (p.get("scale")) document.getElementById("scale").value = p.get("scale");
  if (p.get("demo") === "1") document.getElementById("demo").checked = true;
  if (p.get("boot") === "0") document.getElementById("skipBoot").checked = true;
  applyPageTheme(document.getElementById("theme").value);
})();

boot();
