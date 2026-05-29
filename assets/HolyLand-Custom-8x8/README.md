# HolyLand custom 8×8 item sprites

Hand-drawn icons that match the VEXED Mini-Medieval style (8×8 native, scaled 2× to 16×16 in-game).

## Files

- `Items.png` — **the sheet you draw on** (and the one the game embeds). 64×32 = an 8×4 grid of 8×8 cells. Background is pure magenta `#FF00FF`, which the renderer chromakeys to transparent. Leave magenta wherever you want see-through.
- `mini-medieval.gpl` — VEXED's 24-color palette. Load it in your editor and use **only** these colors.
- `_guide.png` — the grid blown up 8× so you can see the cell boundaries + slots. Reference only; do not ship.
- `_ref_bed.png`, `_ref_basket.png`, `_ref_woodlog.png` — scaled VEXED sprites for style/shading anchors.

## Slots to fill (col, row), 8×8 each

| cell | item | status / anchor |
|------|------|-----------------|
| (0,0) | backpack | empty — draw it; anchor `_ref_basket.png` (woven container) |
| (1,0) | bedroll  | empty — draw it; anchor `_ref_bed.png` (roll the blanket up) |
| (2,0) | campfire | **filled** with a hand-pixeled default (`holyland::CAMPFIRE`); redraw to taste |

The campfire (2,0) is already wired: lit firewood renders it, and it shows in the
in-game sprite picker as **"Fire"** (Items tab) so it's remappable. Backpack/bedroll
slots stay on the basket placeholder in `item_sprite()` until you draw them and we
repoint `ItemKind::Pack` / `ItemKind::Bedroll`.

## How to draw

1. Open `Items.png` in Aseprite / LibreSprite / Piskel (free, browser) / GIMP.
2. Load `mini-medieval.gpl` as the palette.
3. Draw one icon per slot above. Keep the magenta background untouched where transparent.
4. Save as `Items.png` (RGBA PNG, same 64×32 size) and hand it back.

Then the code side (new `Sheet::HolyLandItems` + `item_sprite()` remap) gets wired in — see the plan file.

Source palette: `../Mini-Medieval-Documented-8x8/Palette/Mini-Medieval-Palette-Text-Ref.md`.
