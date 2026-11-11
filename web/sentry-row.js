const STATE_FIELDS = ["rounds", "capacity", "armed", "online", "linkOk", "critical", "selected"];
const LETTERS = ["A", "B", "C", "D"];

/** Copy out scalar values before releasing the wasm-bindgen snapshot. */
export function readSentry(app, index) {
  const snapshot = app.sentry_overview(index);
  if (!snapshot) return null;
  try {
    return {
      rounds: snapshot.rounds,
      capacity: snapshot.capacity,
      armed: snapshot.armed,
      online: snapshot.online,
      linkOk: snapshot.link_ok,
      critical: snapshot.critical,
      selected: snapshot.selected,
    };
  } finally {
    snapshot.free();
  }
}

export function sentryPresentation(unit, index, screen, quit) {
  const caption =
    screen === "boot"
      ? "INITIALIZING"
      : !unit.online
        ? "OFFLINE"
        : !unit.linkOk
          ? "LINK DOWN"
          : unit.rounds === 0
            ? "EMPTY"
            : unit.critical
              ? "AMMO CRITICAL"
              : "REMOTE / STANDBY";
  const weapon = unit.armed ? "ARMED" : "SAFE";
  return {
    caption,
    weapon,
    disabled: screen === "boot" || quit,
    alert: !unit.online || !unit.linkOk || unit.critical || unit.rounds === 0,
    percent:
      unit.capacity > 0
        ? Math.max(0, Math.min(100, (unit.rounds / unit.capacity) * 100))
        : 0,
    selection: quit
      ? "RESTART TO RESUME"
      : screen === "boot"
        ? "INITIALIZING"
        : unit.selected
          ? "SELECTED"
          : "SELECT UNIT",
    label: `Sentry ${LETTERS[index]} (S${index + 1}), ${weapon}, ${unit.rounds} of ${unit.capacity} rounds, ${caption}`,
  };
}

/** Mount stable controls once; only write DOM when a unit's state changes. */
export function createSentryRow(root, onSelect) {
  const row = root.querySelector(".sentry-row");
  const cards = LETTERS.map((letter, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "sentry-unit";
    button.disabled = true;
    button.innerHTML = `
      <span class="unit-heading"><strong>${letter}</strong><span>UA 571-C<span class="unit-number">SENTRY 0${index + 1}</span></span><span class="unit-light" aria-hidden="true"></span></span>
      <span class="sentry-illustration"><img src="sentry-gun.png" alt="" width="640" height="427" decoding="async" /></span>
      <span class="unit-caption"></span>
      <span class="unit-telemetry"><span>ROUNDS<strong><span class="rounds-value"></span><span class="rounds-capacity"></span></strong></span><span class="unit-safe unit-weapon"></span></span>
      <span class="ammo-track" aria-hidden="true"><span class="ammo-fill"></span></span>
      <span class="selection-label"></span>`;
    button.addEventListener("click", () => onSelect(index));
    row.append(button);
    return {
      button,
      caption: button.querySelector(".unit-caption"),
      rounds: button.querySelector(".rounds-value"),
      capacity: button.querySelector(".rounds-capacity"),
      weapon: button.querySelector(".unit-weapon"),
      fill: button.querySelector(".ammo-fill"),
      selection: button.querySelector(".selection-label"),
      last: null,
    };
  });
  let selectedIndex = -1;

  return {
    reset() {
      root.hidden = true;
      selectedIndex = -1;
      for (const card of cards) {
        card.last = null;
        card.button.disabled = true;
      }
    },
    sync(app) {
      const screen = app.screen_name();
      const quit = app.should_quit;
      root.hidden = false;
      for (const [index, card] of cards.entries()) {
        const unit = readSentry(app, index);
        if (!unit) continue;
        const previous = card.last;
        if (previous && previous.screen === screen && previous.quit === quit &&
            STATE_FIELDS.every((field) => previous.unit[field] === unit[field])) continue;
        card.last = { unit, screen, quit };
        const view = sentryPresentation(unit, index, screen, quit);
        card.button.disabled = view.disabled;
        card.button.setAttribute("aria-pressed", String(unit.selected));
        card.button.setAttribute("aria-label", view.label);
        card.button.dataset.alert = String(view.alert);
        card.caption.textContent = view.caption;
        card.rounds.textContent = unit.rounds;
        card.capacity.textContent = ` / ${unit.capacity}`;
        card.weapon.textContent = view.weapon;
        card.weapon.dataset.armed = String(unit.armed);
        card.fill.style.width = `${view.percent}%`;
        card.selection.textContent = view.selection;
        if (unit.selected && selectedIndex !== index) {
          selectedIndex = index;
          // Reveal keyboard/demo selection within the horizontal row without
          // scrolling the page away from the console or moving focus.
          const bounds = card.button.getBoundingClientRect();
          const viewport = row.getBoundingClientRect();
          if (bounds.left < viewport.left)
            row.scrollLeft -= viewport.left - bounds.left;
          else if (bounds.right > viewport.right)
            row.scrollLeft += bounds.right - viewport.right;
        }
      }
    },
  };
}
