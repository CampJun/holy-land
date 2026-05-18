# CP437 atlas map — `assets/cp437_16x16.png`

The atlas is **custom** — many byte positions hold game sprites rather
than the literal CP437 glyph. This file is the ground-truth reference
for what each byte renders as in our atlas, plus our current
assignments.

## How to inspect

In-game: press **D** (X face button) to open the **CP437 glyph
palette** overlay. Browse with dpad; the header shows the current
byte. **B** or **D** to close.

When you spot a useful sprite, write its byte index here next to a
description, then update `items.rs::ItemDef::def()` or
`world.rs::TerrainKind::def()` to use it.

## Current assignments

### Terrain (`world.rs`)

| Kind | Byte | Current glyph |
|---|---|---|
| Grass | `0x2E` | `.` |
| BareDirt | `0x2E` | `.` |
| SandShore | `0x2E` | `.` |
| TreeTrunk | `0x06` | `♠` (BLACK SPADE SUIT — reads as canopy) |
| StreamWater | `0x7E` | `~` |
| PondWater | `0x7E` | `~` |
| Wall | `0x23` | `#` |

### Items (`items.rs`)

| Kind | Byte | Current glyph | Notes |
|---|---|---|---|
| Axe | `0x50` | `P` | Could be more axe-like |
| Knife | `0x2D` | `-` | Could be a dagger sprite |
| Pack | `0x5B` | `[` | |
| Tent | `0x1E` | `▲` (BLACK UP-POINTING TRIANGLE) | Reads well |
| Bedroll | `0x3D` | `=` | |
| CookingPan | `0x4F` | `O` | |
| Waterskin | `0x75` | `u` | |
| FlintAndSteel | `0x21` | `!` | |
| Herb | `0x2A` | `*` | Could be a leaf/flower sprite |
| Twig | `0x2C` | `,` | |
| Stick | `0x2F` | `/` | |
| Firewood | `0x3D` | `=` | |
| GrassBlade | `0x22` | `"` | User noted: "between cent (0x9B) and yen (0x9D) looks like grass" — that's `0x9C`. Inspect with the palette and update if confirmed. |
| Stone | `0x2A` | `*` | User noted: "we use asterisk for rocks when there is already a stone sprite". Find the stone sprite via the palette and update. |
| MossPatch | `0x25` | `%` | |
| Mud | `0x25` | `%` | |
| Ration | `0x25` | `%` | |

### Other glyphs in use

| Where | Byte | Glyph |
|---|---|---|
| Player | `0x40` | `@` |
| Lit-fire override (main.rs render) | `0x2A` | `*` |
| Multi-turn progress bar (filled) | `0xDB` | `█` |
| Multi-turn progress bar (empty) | `0xB1` | `▒` |
| Pause / command / info menu cursor | `0x3E` | `>` |
| HUD: Thirst | `0xF7` | `≈` |
| HUD: Hunger | `0x25` | `%` |
| HUD: Sleep | `0x7A` | `z` |
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
