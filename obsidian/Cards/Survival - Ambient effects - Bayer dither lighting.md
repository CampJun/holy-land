Per-pixel Bayer dither replaces the smooth color blend used by the FOV/light path. First of four ambient-effect cards branched from the [[Survival - Ambient effects - Brainstorm]] decisions.

## Goal

Lit cells today blend smoothly toward an unlit color via per-cell `light_intensity: u8` (`world.rs:331`, `main.rs:1019-1046`). Replace the smooth interpolation with a **4-level ordered dither** computed per-pixel inside each 16×16 cell, using a 4×4 Bayer matrix tiled 4×4 times across the cell. Aesthetic target: DCSS / Brogue / Dwarf-Fortress shading, no banding from 8-bit color quantization, intentional "retro" texture.

## Why per-pixel inside the cell, not per-cell

Per-cell quantization snaps each cell to one of N levels — cleanest on paper, but with only 4 levels across a 5-cell radius the steps are very visible. Per-pixel dither inside each cell preserves the spatial frequency of the gradient: the 16×16 cell mixes "bright" and "dark" pixels in proportion to its intensity, so a viewer sees an apparent smooth gradient made of crisp pixels. Costs one extra surface blit *or* a stamped-mask atlas — see "Implementation sketch" below.

## Spec

- **Bayer matrix:** 4×4, normalized to `0..15`. Constant `BAYER4: [[u8; 4]; 4]` in `render.rs`.
- **Levels:** 4 (off / dim / mid / bright). Picking N=4 to match the matrix dimension means each adjacent pair of levels is perfectly bridged by exactly one threshold step in the matrix.
- **Per-pixel rule** for a cell at intensity `i ∈ 0..=255`:
  - `level_low = i * 4 / 256` (the lower of the two bracketing levels)
  - `frac = (i * 4) % 256` (how far between low and low+1)
  - For pixel `(px, py)` in `0..16`: `threshold = BAYER4[px & 3][py & 3] * 16` (scaled to `0..240`)
  - Pixel picks color for `level_low + 1` if `frac > threshold`, else `level_low`.
- **Palette:** 4 entries indexed by level. Initial guess: black → cool dark blue-grey → neutral mid → warm yellow. Per-cell hue can shift the warm endpoint (see [[Survival - Ambient effects - Smoke particles and FOV]] and fire flicker work).

## Implementation sketch

Two viable paths — pick during the draft:

1. **Stamped mask atlas (preferred for Miyoo).** Pre-bake 4 mask surfaces (one per Bayer threshold step) at 16×16. In `draw_glyph`, after the glyph blit, surface-blit the appropriate mask × intensity-color. Adds one surface blit per dirty lit cell; zero per-pixel computation in the hot path.
2. **Per-pixel composite in `draw_glyph`.** Loop over 256 pixels and write the dithered color into `framebuf` directly when the cell is lit. Cleanest, most flexible, but adds ~256 ops per dirty lit cell.

Both ride the existing per-cell diff: a cell only gets re-blitted when its `(glyph, fg, bg, light_intensity)` tuple changes (`main.rs:1052`). Adding intensity to the diff key means intensity-only changes still trigger one blit — which is what we want.

## Files touched

- `src/render.rs` — add `BAYER4` constant, `dithered_pixel(intensity, px, py) -> Color` helper, optional pre-baked mask surfaces.
- `src/main.rs:1019-1046` — swap the smooth blend for a call to the new helper. Probably no other call-site changes.
- `src/effects/light.rs` (new, per [[Survival - Ambient effects - Brainstorm]] module layout) — house `BAYER4` and the level palette here if it grows beyond a few lines.

## Cost

- Per dirty lit cell: +1 surface blit (mask path) OR +256 ops (per-pixel path).
- Lit area at night with one torch ≈ 80 cells. Already-dirty cells stay 80 — no extra dirtying.
- An optional time-pulse variant (shift Bayer threshold by `(tick / 8) % 16`) would dirty those 80 cells every 8 frames → ~10 dirty cells/frame avg. Cheap. Defer to a follow-up if the static dither lands well.

## Verification

- Desktop screenshot of a torch at night before / after — visible quantized rings should resolve into a clean dithered gradient.
- Miyoo deploy + screenshot. Confirm the texture upload's dirty rect for an idle scene with one fire still matches the lit area, not the whole viewport.
- `cargo test --release` — none expected to be touched, but run to confirm.

## Open

- Whether to add the time-pulse variant in this card or split it into a follow-up.
- Palette selection — should the 4 levels be configurable per biome later?

References: [[Survival - FOV]], [[FOV and lighting]], [[Survival - Fire Making]], [[Survival - Ambient effects - Brainstorm]].
