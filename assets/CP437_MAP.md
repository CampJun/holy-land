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
| TreeTrunk | `0x05` / `0x06` / `0x17` / `0x18` | Four canopy variants picked per cell via `tree_variant_index` hash. `TREE_VARIANT_GLYPHS` in world.rs. DF aligns: 0x05/0x06/0x18/0xE2 are all DF tree glyphs. |
| StreamWater | `0x7E` | `~` (DF: flowing water) |
| PondWater | `0x7E` | `~` (DF would use 0xF7 ≈ for standing water — pre-existing divergence). |
| Wall | `0x23` | `#` — out-of-chunk sentinel only; not the "city wall" tile. DF reserves `#` for floor grates. |
| StoneWall | `0xB2` | `▓` — DF "partially-dug rock" / unsmoothed stone. City walls, cathedral / castle silhouettes. |
| WoodWall | `0xB1` | `▒` — denser shade than Floor. No strong DF analog for a single non-directional wood wall. |
| Floor | `0x2E` | `.` — DF rough floor. Interior placeholder until material variants land. |
| CobbleRoad | `0xF7` | `≈` — DF "rough-stone road/bridge". Cobbled city streets (High St, Fore St, ...). |

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

## Dwarf Fortress reference

DF is the convention other CP437 roguelikes inherit from — picking a
sprite that matches DF's slot makes the glyph self-explanatory to
anyone who's played a roguelike before. Use this table when assigning
a new tile/item/feature; deviate only when our atlas's custom art
makes the DF pick worse.

Source: [dwarffortresswiki.org/index.php/Tilesets](https://dwarffortresswiki.org/index.php/Tilesets).
Subset relevant to a Cornwall / 1300-AD setting; full mapping on the
wiki.

### Terrain / structures

| Byte | Glyph | DF meaning |
|------|-------|------------|
| `0x05` `0x06` `0x18` `0xE2` | `♣` `♠` `↑` `Γ` | Trees (broadleaf / conifer / generic) |
| `0x07` | `•` | Mined-out / rough floor; river source |
| `0x0A` | `◙` | Tree trunk interior |
| `0x23` | `#` | Floor grates; smoothed stone variants |
| `0x2B` | `+` | Smooth / constructed floor; block bridge / road |
| `0x2E` / `0x2C` / `0x27` | `.` `,` `'` | Rough floor; grasses |
| `0x3C` / `0x3E` / `0x58` | `<` `>` `X` | Stairs up / down / up-down |
| `0x5E` | `^` | Trap; volcano |
| `0x5F` | `_` | Channel designation |
| `0x7E` | `~` | Flowing water; sand; dirt road; furrowed soil |
| `0xB0` `0xB1` `0xB2` | `░` `▒` `▓` | Partially-dug rock (rough stone) |
| `0xBA` | `║` | Smooth/constructed wall (vertical); wooden door |
| `0xC5` | `┼` | **Door** (primary glyph) |
| `0xCD` | `═` | Smooth wall (horizontal); planted crops |
| `0xCE` | `╬` | Smooth wall + **fortifications** (battlements) |
| `0xDB` | `█` | Ice wall; dig-designated; trade depot |
| `0xF0` | `≡` | Bars; metal doors; activity zones |
| `0xF7` | `≈` | Water / magma / snow / sand / farm plot / **rough-stone road** |
| `0xF8` | `°` | Sea foam; eggs; bowl; mortar |
| `0xFA` | `·` | Seeds; open space; terrain at lower elevation |
| `0xFE` | `■` | Blocks; minecarts; map vault |

### Vegetation

| Byte | Glyph | DF meaning |
|------|-------|------------|
| `0x05` | `♣` | Quarry bush leaves; blossoms; flowers-on-grass |
| `0x06` | `♠` | **Plump helmet mushroom**; leaf items |
| `0xA9` | `⌐` | Withered plants |
| `0xCD` | `═` | Planted crops (farm tile, growing) |
| `0xE7` | `τ` | **Sapling**; pig tail; cave wheat; rat weed |
| `0xE8` | `Φ` | Sweet pod; bloated tuber; kobold bulb (root crops) |

### Furniture / interiors (for later)

| Byte | Glyph | DF meaning |
|------|-------|------------|
| `0xD1` | `╤` | Table |
| `0xD2` | `╥` | Chair; throne |
| `0xE3` | `π` | Cabinet; display case |
| `0xE5` | `σ` | Anvil; metalsmith's workshop |
| `0xE9` | `Θ` | **Bed** |
| `0xEA` | `Ω` | **Statue** |
| `0xF6` | `÷` | Barrel; still; ashery |

### Items / loot (selected)

| Byte | Glyph | DF meaning |
|------|-------|------------|
| `0x03` | `♥` | Berries; dimple cups |
| `0x04` | `♦` | Cut gems |
| `0x0F` | `☼` | Unmined gem cluster; raw glass; masterpiece tag |
| `0x16` | `▬` | Logs |
| `0x24` | `$` | Coins |
| `0x25` | `%` | Prepared meals; fruits; buds |
| `0x2F` | `/` | Weapons; bolts; pestle |
| `0xAD` | `¡` | Flask; waterskin; pouch |
| `0xFE` | `■` | Stone blocks |

### Known conflicts vs. our atlas

- **0x06 ♠** is our `TreeTrunk` variant AND DF's plump-helmet mushroom.
  If we ever add a mushroom *terrain* tile we'll have to either drop
  0x06 from `TREE_VARIANT_GLYPHS` or accept the divergence (palette /
  context disambiguates in practice).
- **0x9C** is our custom grass-tuft sprite, not the DF £ glyph.
- **0xE7 τ** is currently our `Herb` item (custom sprite). DF uses
  0xE7 for saplings — when `Decoration::Sapling` lands a final glyph
  we should either reuse 0xE7 (and refresh the herb sprite elsewhere)
  or pick a different sapling byte.

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
