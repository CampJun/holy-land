Slice-1 forest tile: authored skeleton + seeded population.

## Dimensions
- 40×30 cells, fits the 640×480 viewport exactly.
- No camera scroll in slice 1.

## Authored skeleton (every seed identical)
- Stream enters from the N edge, snakes down to a pond near center.
- Pond ~10×8 cells of pond-water.
- 5–8 water-loving trees on the pond perimeter.
- Player spawn at tile center (`cell 20, 15`).

## Seeded population (varies by seed)
- 10–20 additional tree cells in the outer ring of the tile.
- 3–5 herb-patch cells anywhere on grass tiles.
- Per-cell debris roll keyed by terrain type (table below).

## Cell debris table (forest-floor cells)
| Roll | Item | Weight each |
|---|---|---|
| 50% chance: 1–3 twigs | twig | 5 g |
| 40%: 1–2 grass blades | grass blade | 2 g |
| 20%: 1 stone | stone | 200 g |
| 15%: 1 moss patch | moss | 10 g |
| 10% (if adjacent to stream/pond): 1 mud | mud | 300 g |

Each roll independent; cells can have multiple categories.

## Cell types (slice 1)
- `Grass` (forest-floor)
- `StreamWater` (drinkable, fillable)
- `PondWater` (drinkable, fillable, fishable)
- `TreeTrunk` (blocks pass + LOS, choppable)
- `HerbPatch` (overlay on grass; pickable)
- `BareDirt`
- `SandShore` (adjacent to pond)

## Determinism
- `(world_seed, chunk_coord)` → deterministic chunk layout via a seeded RNG (use `rand_pcg` or similar small-footprint RNG).
- Test: regenerate same chunk twice from same seed → byte-identical.

## Module
`src/chunkgen.rs` — `fn generate_chunk(coord: ChunkCoord, seed: u64) -> Chunk`. Skeleton is hardcoded for slice-1's `(0, 0)` chunk; slice 2+ generalizes to biome-aware authoring.

Reference: `[[Survival - Chunk and per-cell items]]`, `[[Survival - ItemInstance and weights]]`.
