# Web sentry overview

The MVP uses the approved deployment-bay layout: four realistic illustrated
sentries below the WASM console and above the keyboard instructions.

- A–D map to existing sentries S1–S4.
- Rust exports independent ammo, capacity, armed, datalink, availability,
  critical, and selection snapshots. JavaScript releases each snapshot.
- Clicking a unit uses the existing number-key command, stopping demo.
- Keyboard and demo selections update the row; firing and reload update ammo.
- Selection is disabled during POST and after Quit; restart resets the row.
- All four themes share the same neutral artwork. Phones scroll horizontally;
  keyboard/demo selection reveals the selected card without moving focus.
- The row remains outside the fullscreen console stage.

## Artwork

`web/sentry-gun.png` is a 640-pixel, alpha-channel PNG (about 189 KB), reused
by all four cards. Generated with the built-in image tool from the approved
concept; no source photograph is included. No runtime 3D dependency.

Generation prompt: one photorealistic fictional movie sentry gun matching the
approved concept; weathered olive metal, angular receiver, long single vented
barrel pointing right, side ammunition drum, swivel neck, three-legged stand;
three-quarter perspective, realistic lighting, full gun with margins;
transparent background, no floor, interface, lettering, labels, or border.

## Validation

Run `./scripts/verify.sh`. The overview tests cover snapshot ownership,
per-unit isolation, selection, fault precedence, and ammo percentages.
Manually check click/keyboard selection, demo, firing/reload, POST,
Quit/Restart, themes, and narrow-screen scrolling against the live WASM app.

The original SVG study and interactive mockups are development artifacts,
not part of the production page or deployment manifest.
