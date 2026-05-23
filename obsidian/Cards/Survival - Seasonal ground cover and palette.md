The visual half of seasons. Two coupled layers: a season-aware `TerrainDef` palette table (the base bg/fg per terrain shifts with the season) and a per-cell `GroundCover` overlay (snow, fallen leaves, leaf litter).

Depends on [[Survival - Calendar and seasons]].

## Season-aware TerrainDef

Current shape (`src/world.rs:175–286`):

```rust
pub struct TerrainDef {
    pub save_key: &'static str,
    pub name: &'static str,
    pub glyph: u8,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub walkable: bool,
    pub blocks_sight: bool,
}
```

Becomes:

```rust
pub struct TerrainDef {
    pub save_key: &'static str,
    pub name: &'static str,
    pub glyph: u8,
    pub palette: [(/*fg*/ [u8;3], /*bg*/ [u8;3]); 4], // indexed by Season as usize
    pub walkable: bool,
    pub blocks_sight: bool,
}

impl TerrainDef {
    pub fn fg(&self, s: Season) -> [u8;3] { self.palette[s as usize].0 }
    pub fn bg(&self, s: Season) -> [u8;3] { self.palette[s as usize].1 }
}
```

Authored palette (starting draft — tune in playtest):

| Terrain | Spring fg/bg | Summer fg/bg | Autumn fg/bg | Winter fg/bg |
|---|---|---|---|---|
| `Grass` | `[80,150,55]` / `[35,70,30]` | `[55,120,40]` / `[25,55,25]` | `[140,110,45]` / `[60,45,20]` | `[180,190,200]` / `[110,120,130]` |
| `BareDirt` | `[120,90,55]` / `[60,45,28]` | `[140,100,55]` / `[70,50,30]` | `[110,80,45]` / `[55,40,25]` | `[170,170,170]` / `[90,95,100]` |
| `SandShore` | `[200,180,130]` / `[140,120,80]` | `[210,190,135]` / `[150,125,80]` | `[190,170,120]` / `[130,110,75]` | `[210,215,220]` / `[150,160,170]` |
| `TreeTrunk` | `[110,145,60]` (canopy olive) / dark | `[60,100,40]` (dense) / dark | `[180,110,40]` (copper) / dark | `[150,135,110]` (bare bark) / dark |
| `StreamWater` | `[80,130,180]` / `[40,75,120]` | `[80,130,180]` / `[40,75,120]` | `[70,110,150]` / `[35,65,100]` | `[150,170,200]` / `[80,110,150]` (cool) |
| `PondWater` | (same as Stream, slight desat) | … | … | … (future: `Frozen` variant) |
| `Wall` | invariant | invariant | invariant | invariant |

Per-tree-species seasonal tints replace the existing `TREE_TINT_VARIANTS` (`world.rs:200–208`) — see [[Survival - Species variety - trees and undergrowth]] for the per-species table. The TerrainDef.TreeTrunk row above is the species-agnostic fallback.

## GroundCover layer

New field on `CellState`:

```rust
pub enum GroundCover { None, Snow, FallenLeaves, LeafLitter }

pub struct CellState {
    // existing fields…
    pub ground_cover: GroundCover,
}
```

### Variants

- **`Snow`** — winter only. Bg lerps 60% toward `[230, 235, 245]`. Lays at first winter dawn (Dec 21) on every loaded outdoor cell not under dense canopy (canopy = within 1 cell of a TreeTrunk + cell rolls < 0.4 against a per-cell canopy hash). Clears at first spring dawn (Mar 21).
- **`FallenLeaves`** — autumn only. Bg lerps 35% toward `[140, 70, 30]`. Spawns over autumn at a 1-cell radius around every TreeTrunk that's deciduous (Oak/Ash/Hazel/Rowan — not Holly). Spawn rate scales with autumn day-of-season (none on Sep 23; max by Nov 1). Covered by Snow when winter starts; the FallenLeaves are *replaced* by Snow, not overlaid — restore on Spring dawn? No: cleared, leaves rot. (Decay justified.)
- **`LeafLitter`** — permanent, not seasonal. Bg lerps 25% toward `[60, 45, 25]`. Placed during chunkgen on every grass cell within 1 cell of a TreeTrunk. Persists through all seasons (in winter, the Snow lerp paints over it visually; the cell still holds `LeafLitter` underneath, so when snow clears in spring it's visible again).
- **`None`** — default; render path skips the lerp pass.

### Precedence

When two effects overlap (e.g. LeafLitter cell during winter), the per-cell `ground_cover` field stores only one variant. Rule: ephemeral wins over permanent in render, but the stored value reflects the ephemeral. At winter dawn, `LeafLitter` cells flip to `Snow`. At spring dawn, those cells flip back to `LeafLitter`. Tracked by an auxiliary `underlying_cover` field? Simpler: the cell is `LeafLitter` permanently; the *render path* checks `season == Winter && cell.outdoors` and paints Snow on top without mutating state. Snow then is purely a *render-time* effect, not a stored field — except for FallenLeaves which IS stored because it accumulates over autumn day-by-day.

Revised model:
- `LeafLitter` is **stored** at chunkgen.
- `FallenLeaves` is **stored**, spawned at autumn dawns, cleared at winter dawn.
- `Snow` is **render-time-only** based on `season == Winter && is_outdoor(cell)`. Avoids the precedence question entirely and saves schema bytes.

So `GroundCover` enum shrinks: `{ None, FallenLeaves, LeafLitter }`. Snow is computed from `(season, terrain.is_outdoor)` at render time.

## Render path

In `main.rs:1035–1067`, after the terrain `bg = tint_color(terrain.bg(season), brightness)` line:

```rust
// terrain seasonal bg already applied
let bg = match cell.ground_cover {
    GroundCover::None => bg,
    GroundCover::FallenLeaves => lerp_rgb(bg, [140,70,30], 0.35),
    GroundCover::LeafLitter   => lerp_rgb(bg, [60,45,25],  0.25),
};
let bg = if season == Season::Winter && terrain.is_outdoor() {
    lerp_rgb(bg, [230,235,245], 0.60)
} else {
    bg
};
let bg = tint_color(bg, brightness); // existing day-night tint applies last
```

Two lerps + the existing brightness multiply per dirty cell. mmiyoo surface-level color mod handles this fine (`render.rs:9–65`); only texture color mod is broken.

## Save

`CellState.ground_cover: GroundCover` round-trips via `chunks_dirty` in `RunSave` (already proposed in [[Survival - Save schema v2]]). `#[serde(default)]` → `None`. No new SCHEMA_VERSION beyond v2.

## Tests

- Round-trip: save with `FallenLeaves` on a few cells → load → cells preserve.
- Determinism: same seed → same `LeafLitter` placement after chunkgen.
- Season flip: fast-forward to Mar 21 dawn → all `FallenLeaves` cells become `None`.
- Render diff: at Dec 21 dawn, every outdoor dirty cell shows the snow lerp.

## Out of scope

- Frost (early winter speckle) — deferred to a polish pass.
- Snow depth / footprints — future card; would require `Snow` to become a stored variant with a depth byte.
- Frozen pond/stream as a new `TerrainKind` — Cornwall slice-2 territory.
- Mud / wet-after-rain cover — depends on weather (see [[Survival - Ambient effects - Weather state and wind vector]]).

References: [[Survival - Calendar and seasons]], [[Survival - Tile generation slice 1]] (LeafLitter placement runs in `chunkgen.rs:34`), [[Survival - Species variety - trees and undergrowth]] (deciduous-vs-evergreen determines FallenLeaves spawn), [[Survival - Save schema v2]], [[Survival - Day night cycle]] (brightness tint applies after season tint), [[Survival - Ambient effects - Brainstorm]] (D1 Time-of-day tint is subsumed by this card's render path).
