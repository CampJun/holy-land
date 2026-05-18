Per-cell item storage and the chunk substrate for slice 1.

## Slice-1 scope
- Single 40×30 chunk visible; no neighbor chunks loaded yet.
- `World { chunks: HashMap<ChunkCoord, Box<Chunk>>, seed: u64, clock_seconds: u64, player: Entity, ... }`.
- `Chunk` carries a `cells: Vec<Cell>` indexed `y * width + x`; each `Cell { terrain: TerrainKind, items: Vec<ItemInstance> }`.
- Per-cell item-list defaults to empty; CDDA's research showed even 100×100 active tiles with average sparse items costs ≪1 MB — Miyoo's 256 MB RAM makes this a non-issue at slice-1 scale.

## Verbs operating on cells (slice 1)
- Pickup: `world.cell_at_mut(x, y).items.drain_into(player_inventory, predicate)`.
- Drop: `player_inventory.move_item_to(world.cell_at_mut(x, y))`.
- Inspect (for command menu): `world.cell_at(x, y).items.iter()`.

## Pristine vs dirty (forward-looking)
- Each `Chunk` carries a `dirty: bool`. Slice 1 only has one chunk so eviction doesn't fire.
- Slice 2 onward: pristine chunks regenerate from `(seed, cx, cy)`; dirty chunks serialize to disk.

## Why this comes first
The `tile_at(i64, i64)` API in `world.rs` was already shaped for chunks. This card is the storage layer that makes everything else (debris, dropping, fire entities, structures) work.

Reference: `[[Survival - Save schema v2]]`, `[[Survival - Tile generation slice 1]]`.
