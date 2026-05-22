Tree species + interactive undergrowth. Replaces the current single-species `TreeTrunk` variant set with 5 named species, and adds a per-cell `Decoration` field for ferns, moss, bramble, bracken, gorse, and saplings — each interactive via dedicated harvest verbs.

Depends on [[Survival - Calendar and seasons]] (for season-aware tints) and the schema-v2 bump in [[Survival - Save schema v2]].

## Tree species

5 species, Cornwall-flavored per Cornwall-World.md §149 OakWoodland:

| Species | CP437 glyph | Deciduous | Forage yield (in season) | Chop yield |
|---|---|---|---|---|
| Oak     | `0x05 ♣` | yes | Acorn (Sep–Oct mast, drops as items) | 5 firewood, dense, slow chop |
| Hazel   | `0x18 ↑` | yes | Hazelnut (Sep–Oct) | 3 firewood, fast chop |
| Holly   | `0x06 ♠` | no  | none v1 (berries deferred) | 2 firewood, burns hot |
| Ash     | `0x17 ↨` | yes | none (future: keys) | 4 firewood, dries fast — best burn |
| Rowan   | `0x18 ↑` | yes | RowanBerry (Sep–Oct) | 2 firewood |

Replaces `TREE_VARIANT_GLYPHS` and `TREE_TINT_VARIANTS` at `world.rs:195–208`. Species stored per TreeTrunk cell.

### Storage

Option A: a new `TerrainKind::TreeTrunk { species: TreeSpecies }` variant. Cleanest semantically, but `TerrainKind` is currently a flat enum and the cell stores it as a single-byte discriminant — adding a payload changes the cell width.

Option B: keep `TerrainKind::TreeTrunk` flat, store species in a parallel `tree_species: Option<TreeSpecies>` field on `CellState` (None for non-tree cells, Some for trees). One byte per cell wasted on non-tree cells but matches the existing `decoration` and `ground_cover` field pattern.

**Choice: Option B.** Aligns with the per-cell field pattern already adopted for `ground_cover` and `decoration`. Saves the TerrainKind refactor for Cornwall slice-2 when biome-aware terrain might justify it anyway.

### Per-species seasonal tints

Replaces `TREE_TINT_VARIANTS` (`world.rs:200–208`) with a `[TreeSpecies; 5] × [Season; 4]` table:

| Species | Spring fg | Summer fg | Autumn fg | Winter fg |
|---|---|---|---|---|
| Oak | `[100,150,55]` | `[55,110,40]` | `[170,110,40]` | `[140,130,110]` bare |
| Hazel | `[140,170,80]` | `[100,150,60]` | `[200,150,50]` | `[150,140,110]` bare |
| Holly | `[40,100,40]` | `[35,95,35]` | `[40,100,40]` | `[40,100,40]` evergreen |
| Ash | `[120,160,70]` | `[80,130,55]` | `[180,140,60]` | `[155,145,125]` bare |
| Rowan | `[130,170,75]` | `[90,135,55]` | `[210,90,40]` | `[160,150,120]` bare |

Render path reads `cell.tree_species` and indexes `[species as usize][season as usize]`. Per-cell hash for per-species jitter (today's mechanism at `main.rs:990–995`) becomes a 4-RGB cluster around the species/season tint.

### Forage drops (mast)

When a tree's species + the current season + a per-tree hash say "this is a fruiting cell-week," the **chunkgen lifecycle pass** (or dawn re-eval — see [[Survival - Plant lifecycle and seasonal foraging]]) drops Acorns / Hazelnuts / RowanBerries onto adjacent cells as `ItemInstance`s. Player picks them up via existing `Pickup` action. Mast period and density per species.

## Undergrowth — decoration layer

New per-cell field `CellState.decoration: Decoration`:

```rust
pub enum Decoration {
    None,
    Fern { state: PlantState },
    Moss,                          // no state — moss is just there until harvested
    Bramble { state: PlantState }, // fruiting in summer
    Bracken { state: PlantState }, // browns hard in autumn
    Gorse { state: PlantState },   // flowers spring
    Sapling { species: TreeSpecies, planted_day: u32 },
    Mushroom { kind: MushroomKind, expires_day: u32 },
}
```

`PlantState` is defined in [[Survival - Plant lifecycle and seasonal foraging]]. This card defines the *vocabulary*; lifecycle card defines the *state machine*.

### Rendering

Decoration renders as an **overlay glyph on top of terrain bg** when no item occupies the cell (items still win for the foreground glyph). Per-decoration glyph + season tint:

| Decoration | Glyph | Walkable | Blocks LOS | Notes |
|---|---|---|---|---|
| `Fern` | `0xF0 ≡` | yes | no | Spring/summer green, autumn brown, winter sparse glyph swap to `0x2C ,` |
| `Moss` | `0x07 •` | yes | no | Year-round green, slightly bluer in winter (damp) |
| `Bramble` | `0x9D ¥` | yes (1.5× cost) | no | Spring green, summer green+berry, autumn deep red, winter bare |
| `Bracken` | `0x16 ▬` | yes | no | Summer dense green, autumn copper, winter brown husk |
| `Gorse` | `0x05 ♣` | **no — blocks pass + LOS** | yes | Spring + summer yellow flowers (warm tint), autumn/winter dark green spiny |
| `Sapling` | `0x18 ↑` | yes | no | Same as host tree species; tiny |
| `Mushroom` | `0xFA ·` (or `0x9B ø`) | yes | no | Beige/red depending on kind; transient |

Glyph priority per cell: `item > decoration > terrain glyph`. Walkable/blocks-sight: `terrain | decoration` (OR semantics; gorse and tree both block).

### Persistence

Decoration round-trips via `chunks_dirty` in `RunSave`. `#[serde(default)]` → `None`. Lifecycle state inside `Decoration::*` variants serializes via the existing `ItemInstanceSave` pattern.

## Interactive harvest verbs

User requirement: "moss harvested for a poultice, ferns cut down or dug up." Each verb targets `cell.decoration` on the player's cell or adjacent; uses [[Survival - Multi-turn action queue]] for non-instant costs.

| Verb | Target decoration | Time cost | Yield (items) | Decoration after | Skill |
|---|---|---|---|---|---|
| `HarvestMoss` | `Moss` | 30 s | `Moss × 1–2` | `None` | Foraging |
| `CutFern` | `Fern` (state ≥ Mature) | 10 s | `FernFrond × 2` | `None` | none |
| `DigFern` | `Fern` (state ≥ Mature) | 60 s | `FernRoot × 1` + `FernFrond × 1–2` | `None` | Foraging |
| `CutGorse` | `Gorse` | 120 s | `GorseFaggot × 1` (kindling, burns hot+fast) | `None` | none |
| `CutBracken` | `Bracken` | 20 s | `BrackenStraw × 3` (bedding) | `None` | none |
| `PickBrambleFruit` | `Bramble` (state == Fruiting) | 8 s | `BrambleFruit × 1–3` | `Bramble` (state stays Mature) | Foraging |
| `PickMushroom` | `Mushroom` | 4 s | `Mushroom { kind }` | `None` | Foraging |
| `PickHerb` (existing) | herb item — unchanged | — | — | — | Foraging |

PickBrambleFruit is the only verb that **doesn't remove** the decoration — the bush stays, the fruit is gone. Future: bramble re-fruits after N days (lifecycle).

Verb visibility in the [[Survival - Command menu]] hold-Y radial: directionally surfaces whichever harvest verb matches the current cell's decoration.

## New ItemKind variants

Added to `ItemKind` (`src/items.rs`):

- `Moss` (10 g) — poultice ingredient (future crafting recipe).
- `FernFrond` (15 g) — bedding / dye / minor herb.
- `FernRoot` (40 g) — medicinal (future crafting).
- `GorseFaggot` (250 g) — kindling. Burns hot, fast (`fuel_seconds: 600`). Compare Firewood `3600`.
- `BrackenStraw` (5 g per bundle) — sleep insulation: holding bracken under a Pitched bedroll improves sleep recovery (future hook).
- `BrambleFruit` (25 g) — edible, summer.
- `Hazelnut` (8 g) — edible, autumn. Stackable.
- `Acorn` (12 g) — edible only after leaching (future); raw for pigs (future livestock).
- `RowanBerry` (10 g) — edible, slight bitter; medicinal flavor.
- `Bilberry` (5 g) — edible, summer-autumn. Heath biome (Cornwall slice-2); reserved name.
- `Mushroom { kind: MushroomKind }` — `MushroomKind { Edible, Mild, Toxic, Hallucinogenic }`. Identification skill check at pickup or on Eat.

## Chunkgen placement

Refactored `src/chunkgen.rs:34–120` (covered fully in [[Survival - Forest density and species procgen]]) places:

- TreeTrunk cells with `cell.tree_species` populated by noise-weighted species roll.
- Decoration cells via per-cell weighted roll: Fern/Moss favored in shaded/wet (canopy noise > 0.5), Gorse/Bracken favored in open/dry, Bramble at woodland-edge cells.
- `Decoration::Sapling` is the regrowth result of chopping — not chunkgen-placed initially.

## Save

All cell-level fields (`tree_species`, `decoration`) ride `CellState` in `chunks_dirty`. Schema-v2 absorbs them. Test: round-trip a chunk with mixed species + decorations.

## Out of scope

- Bramble re-fruiting (state machine handles regrowth — implemented in [[Survival - Plant lifecycle and seasonal foraging]], not here).
- Holly berries (yield variant deferred — winter food source).
- Bilberry / heather (heath biome, Cornwall slice-2).
- Sea-pink / coastal flora (cliff biome, Cornwall slice-2).
- Tree felling direction / log dragging — see [[Survival - Drag mechanic]].
- New verb wiring details — verb table here; implementation in [[Survival - Command menu]] + [[Survival - Multi-turn action queue]] follow.

References: [[Survival - Seasonal ground cover and palette]] (FallenLeaves spawn keys off deciduous status defined here), [[Survival - Plant lifecycle and seasonal foraging]], [[Survival - Forest density and species procgen]], [[Survival - Calendar and seasons]], [[Survival - Tile generation slice 1]], [[Survival - Save schema v2]], [[Cornwall-World]] §149 (flora target), [[Survival - Command menu]], [[Survival - Multi-turn action queue]], [[Survival - Skill system URW]] (Foraging skill checks).
