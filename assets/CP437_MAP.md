# CP437 atlas map — `assets/cp437_16x16.png`

The atlas is **custom** — many byte positions hold game sprites rather
than the literal CP437 glyph. This file is the ground-truth reference
for what each byte renders as in our atlas, plus our current
assignments.

## Picking an atlas

The binary loads one atlas at boot, chosen by `atlas.txt` in the same
folder as the binary (Miyoo: `App/HolyLand/atlas.txt`; desktop: next to
the built executable, e.g. `target/release/atlas.txt`). Recognized keys:

- `cp437` — `assets/cp437_16x16.png` (default; documented below).
- `aesomatica` — `assets/Aesomatica_16x16.png` (alternate art set).

Edit `atlas.txt` (FTP for Miyoo, text editor for desktop) and relaunch.
Unknown / missing value falls back to `cp437`. The chosen atlas is
logged at boot — check `holyland.log` for the `atlas:` line to confirm.

Both atlases must share the same byte → sprite layout for the game's
glyph references (items, terrain, frame chars) to render correctly.

## Rendering: alpha + color

The atlas is RGBA. Magenta `(255, 0, 255)` is the chromakey — those
pixels get alpha=0 in `render.rs::load_atlas`. **Every other pixel's
RGB is preserved**, so the atlas's color and grayscale shading
survive the load. The blit in `draw_glyph` applies `color_mod fg` and
`alpha_mod fg.a`, so per-pixel:

- white-on-magenta sprite × colored fg → tinted silhouette
  (unchanged behavior for standard CP437 chars).
- colored sprite × white fg → atlas color preserved (use this for
  custom sprites like the chicken leg, mug, stone, trees, grass).
- grayscale sprite × colored fg → shaded tint (highlights survive).

**Picking fg for a new sprite:** if the atlas pixel was drawn in its
intended color, use a near-white `fg` (e.g. `[230, 230, 230]`) so the
artist's color shows. Day/night tinting still multiplies `fg` by the
clock-driven brightness, so dimming works either way.

## How to inspect

In-game: press **Esc / Start** to open the pause menu, then select
**CP437 glyph palette (dev)**. Browse with dpad; the header shows the
current byte. **B** to close (returns to game).

When you spot a useful sprite, write its byte index here next to a
description, then update `items.rs::ItemDef::def()` or
`world.rs::TerrainKind::def()` to use it.

## Current assignments

### Terrain (`world.rs`)

| Kind | Byte | Notes |
|---|---|---|
| Grass | `0x9C` | Custom grass-tuft sprite (atlas-colored). Sparse-dot logic in main.rs renders blank for ~75% of cells. |
| BareDirt | `0x2E` | `.` |
| SandShore | `0x2E` | `.` |
| TreeTrunk | `0x05` / `0x06` / `0x17` / `0x18` | Four canopy variants picked per cell via `tree_variant_index` hash. `TREE_VARIANT_GLYPHS` in world.rs. |
| StreamWater | `0x7E` | `~` |
| PondWater | `0x7E` | `~` |
| Wall | `0x23` | `#` |

**Unused tree sprites (reserved):** `0xB5`, `0xC6` are dead trees.
Future `TerrainKind::DeadTree` (chopped stumps / burnt-out groves)
would use these.

### Items (`items.rs`)

| Kind | Byte | Current glyph | Notes |
|---|---|---|---|
| Axe | `0x50` | `P` (TODO: more axe-like sprite) |
| Knife | `0x2D` | `-` (TODO: dagger sprite) |
| Pack | `0x5B` | `[` |
| Tent | `0x1E` | `▲` (BLACK UP-POINTING TRIANGLE) — reads well |
| Bedroll | `0x3D` | `=` |
| CookingPan | `0x4F` | `O` |
| Waterskin | `0x75` | `u` |
| FlintAndSteel | `0x21` | `!` |
| Herb | `0xE7` | Custom herb / small-plant sprite (atlas-colored). |
| Twig | `0x2C` | `,` (monochrome) |
| Stick | `0x2F` | `/` (monochrome) |
| Firewood | `0x16` | Custom 3-log pile sprite (atlas-colored). |
| GrassBlade | `0x22` | `"` |
| Stone | `0x07` | Custom stone sprite (atlas-colored). |
| MossPatch | `0x25` | `%` |
| Mud | `0x25` | `%` |
| Ration | `0xE0` | Custom chicken-leg sprite (atlas-colored). Also used as the HUD hunger meter symbol. |

### Other glyphs in use

| Where | Byte | Glyph |
|---|---|---|
| Player | `0x40` | `@` |
| Lit-fire override (main.rs render) | `0x2A` | `*` |
| Multi-turn progress bar (filled) | `0xDB` | `█` |
| Multi-turn progress bar (empty) | `0xB1` | `▒` |
| Pause / command / info menu cursor | `0x3E` | `>` |
| HUD: Thirst | `0x14` | Custom mug sprite (atlas-colored). |
| HUD: Hunger | `0xE0` | Custom chicken-leg sprite (matches Ration item). |
| HUD: Sleep | `0xE9` | Custom bed sprite (atlas-colored). |
| HUD: Warmth | `0x0F` | `☼` |

## Notes from inspection

Add observations here as you browse the palette in-game. Format:

```
0x..  description (what it looks like in our atlas)
```

For example:

```
0x9C  small grass tufts (custom — not the standard £ glyph)
0xFE  filled square — could be a rock/boulder
0xFA  small dot — pebble-sized
```

Once a byte's identity is confirmed, update the relevant `def()` arm
in `items.rs` or `world.rs` and reload (or delete the save) to see
the change.

## Picking a sprite — checklist

1. Open palette in-game (X button).
2. Navigate to the candidate byte; note its hex value from the header.
3. Update the relevant `def()` arm in `items.rs` / `world.rs`.
4. `cargo build` (or run); delete save if chunkgen needs to re-roll.
5. Verify the sprite reads correctly at normal play distance, in
   day-tint, in dim-explored memory, and under the lit-fire override
   if applicable.
6. Update the "Current assignments" table above.
