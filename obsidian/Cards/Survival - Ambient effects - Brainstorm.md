Hub card. The world feels static while the player is idle — torches don't flicker, water doesn't ripple, grass doesn't sway, light doesn't dither. This card collects the candidate effects, the Miyoo render budget that constrains them, and the design decisions that branch into the four follow-up cards.

The four follow-ups are:

- [[Survival - Ambient effects - Bayer dither lighting]]
- [[Survival - Ambient effects - Glyph cycle framework and demo]]
- [[Survival - Ambient effects - Weather state and wind vector]]
- [[Survival - Ambient effects - Smoke particles and FOV]]

Order: 1 and 2 are independent. 3 must land before 4 (smoke consumes the wind vector).

## Miyoo budget we're working inside

Sources: `AGENTS.md`, `src/main.rs:78-82, 920-1062, 1019-1046`, `src/render.rs:9-65`.

- mmiyoo SDL2 no-ops `SetTextureColorMod`, `SetTextureAlphaMod`, texture `BlendMode`, `SetVSync`. Textures only ARGB8888 / RGB565. 32×32 source rects get dropped.
- `MMIYOO_RenderPresent` is a non-blocking page flip. Pacing is `thread::sleep(TARGET_FRAME - elapsed)`; `TARGET_FRAME` is `16_667 µs` (60 fps cap) although commit `cbab4e8` set 30 fps target for Miyoo (see [[Survival - Miyoo 30 FPS target]]).
- Render path: compose CPU `Surface` → diff cells (`prev_cells`) → blit only changed cells → upload one dirty rect → present once. Surface-level color mod (`SDL_SetSurfaceColorMod`) works fine (software path); only texture-level color mod is broken.
- `light_intensity: u8` already recomputes per-cell each frame — proves the "transient per-cell overlay" pattern.
- Viewport is 40×30 = 1200 cells, each 16×16 ARGB = 1 KB. Full refresh ≈ 1.2 MB texture upload + ~1200 surface blits/frame. Cortex-A7 @ ~1.2 GHz single-thread → on the edge of a 30 fps budget.
- Working assumption (guess until measured): keep total effect-driven dirty cells under ~200/frame to leave headroom for scrolling, FOV recompute, and UI.

## Idea catalog

### Lighting

- **L1 — Static Bayer dither over the existing smooth falloff.** No time component. Same dirty-cell cost as today.
- **L2 — Animated dither pulse.** Shift Bayer threshold by `(tick / 8) % 16`. Every lit cell becomes dirty when the step changes; ≈ 10/frame avg per torch.
- **L3 — Two-color dither warm core vs cool rim.** Same cost as L1.
- **L4 — Sub-cell glyph dithering (`█▓▒░`).** Collides with terrain glyphs; would need an empty-cell rule.

### Fire

- **F1 — Light-source flicker.** ±10–15% intensity, warm hue shift, seeded by `(fire_id, tick / 2)`. 0 extra dirty cells (rides on light recompute).
- **F2 — Fire glyph cycle.** `*` → `▲` → `↑` → `*` every 4–6 frames. 1 cell per fire per step.
- **F3 — Smoke drift.** `'` `°` `,` rising, lifetime 3 cells. ~1–2/frame per fire. Sim-coupled (FOV penalty).
- **F4 — Ember sparks.** Single-frame yellow `.` every ~0.5s. Trivial.
- **F5 — Heat haze.** Swap cells above a fire with a neighbor's glyph for 1 frame. Risks looking like a bug.

### Wind

- **W1 — Per-cell grass sway.** ~200 grass cells × 1 Hz / 30 fps ≈ 7 dirty/frame.
- **W2 — Gust wave.** Wavefront across viewport every ~5s, ~30 cells bursty.
- **W3 — Drifting leaves / debris.** 1 cell at a time.
- **W4 — HUD wind indicator.** Tiny corner glyph.

### Water

- **WA1 — Ripple cycle on stream / pond.** ~4 dirty/frame.
- **WA2 — Directional stream flow.** Per-segment flow direction.
- **WA3 — Pond reflection shimmer (night).** 1 cell at a time.

### Weather / day-night (longer horizon)

- **D1 — Time-of-day tint.** Step every few seconds, not every frame. **Subsumed by [[Survival - Seasons and flora - Brainstorm]]** — the seasonal palette card carries the broader (cross-domain) time/season tint render path. D1 stays as a label for the diurnal half but is no longer a separate work item.
- **D2 — Rain.** ~20 particles → ~20 dirty/frame moving.
- **D3 — Lightning flash.** Whole-screen pulse; very rare.

## Decisions

- **Scope:** all four categories (lighting, fire, wind, water) eventually.
- **Budget:** no cap during design — maximally ambient. Plan to add a graphics-level setting later (**deferred** until Miyoo measurements force it).
- **Tick source:** wall-clock `render_tick` (advances every frame, including in menus).
- **Effect model:** hybrid. Pure functions of `(coords, tick, seed)` for static-y effects (dither, flicker, sway, ripples). Particle entities with positions + lifetimes only where motion across cells is essential (smoke, sparks, drifting debris).
- **Light dither style:** per-pixel inside each 16×16 cell, **4×4 Bayer with 4 brightness levels**.
- **Fire glyphs:** two-step — draft the real `GlyphCycle` code path first, then a standalone explorer binary to A/B many candidates. See [[Survival - Ambient effects - Glyph cycle framework and demo]].
- **Sim coupling:** smoke obscures FOV via a single-cell visibility penalty (fixed amount, no stacking). Heat haze stays cosmetic. Wind is weather-state-controlled (calm/breezy/gusty/storm + direction); smoke + future fire-spread read from it.
- **Glyph pool for explorer:** wide CP437 set (vertical / pointy / wispy) plus a curated hand-picked list; random subsets draw from the union.
- **Weather state machine:** ship as full infrastructure now, save-persistent. See [[Survival - Ambient effects - Weather state and wind vector]].
- **Grass sway:** independent ambient — does **not** respond to wind state. Wind affects smoke and future drifting debris.
- **Particle persistence:** smoke/sparks/debris are pure render state — not saved. Regenerate from world conditions on load.
- **Module layout:** new top-level `src/effects.rs` from day one, with submodules under `src/effects/` (`light.rs`, `fire.rs`, `weather.rs`, `smoke.rs`, `glyph_cycle.rs`) as each grows.
- **Water flow:** no directional flow. Isotropic ripple shimmer on streams and ponds.

## What this card is not

Not a plan to ship every idea above. The four follow-up cards are the actual planned work; the rest of the catalog (sparks, heat haze, gust waves, leaves, water ripples, time-of-day tint, rain, lightning) stays in this hub as a backlog to mine later.

References: [[Survival - FOV]], [[Survival - Fire Making]], [[Survival - Miyoo 30 FPS target]], [[FOV and lighting]].
