Extend `src/chunkgen.rs:34` with deterministic noise fields, coverage-driven tree placement, noise-weighted species and undergrowth, and an explicit playability fixup. Imports the **small, biome-agnostic** pieces of the Wyrdlands PRD `08-procedural-forest-worldgen-prd.md` while staying inside the current single-biome (OakWoodland-equivalent) chunkgen. Sub-biome dispatch and hydrology are explicitly deferred to Cornwall slice-2 (`Cornwall-World.md §6`).

Depends on [[Survival - Species variety - trees and undergrowth]] (defines the vocabulary this card places). Doesn't strictly need [[Survival - Calendar and seasons]] but season cards are likely shipped first.

## What we import from the Wyrdlands PRD

| Wyrdlands PRD section | Imported now? | Notes |
|---|---|---|
| §Generation Pipeline pt 1: noise fields (elevation, ruggedness, canopy potential, moisture) | **Yes** | Reduced set: just `canopy_density` + `moisture`. `elevation` deferred until Cornwall biome stack exposes Z-axis. |
| §Pipeline pt 2: hydrology carve | **No** | Authored stream/pond skeleton stays. Cornwall slice-2 introduces hydrology when multi-biome rivers land. |
| §Pipeline pt 3: sub-biome affinity | **No** | Single-biome until Cornwall slice-2's `BiomeId` dispatch. |
| §Pipeline pt 4: terrain realization | **Yes** | Place TreeTrunk + species via noise-weighted probability bands. Replaces today's flat 10–20 count loop. |
| §Pipeline pt 5: playability/fixup | **Yes** | Reserve central spawn-safe disc + enforce major traversable component. |
| §Data Contracts (`WorldgenConfig`) | **Partial** | Just `Width`, `Height`, `Seed`, `TargetTreeCoverageMin/Max`. No `BiomePreset`, no `EnableHydrology`, no `Depth` yet. |
| §Determinism Policy | **Yes** | Same `(world_seed, chunk_coord)` → byte-equivalent tile layout. Already a property of current chunkgen; test stays. |
| §Ecological Acceptance Metrics | **Partial** | Total tree coverage in `[45%, 70%]` enforced. Sub-biome metrics skipped (single biome). Wet-zone tree-density correlation implicit via moisture noise weighting. |

## Pipeline (revised `generate_chunk`)

```
fn generate_chunk(coord: ChunkCoord, world_seed: u64) -> Chunk:
    rng = chunk_rng(coord, world_seed);  // existing SplitMix64 mixer

    // 1. Noise fields (new)
    canopy = value_noise_2d(coord, world_seed ^ 0xC4_C0_BA_BE);   // u8 per cell
    moisture = value_noise_2d(coord, world_seed ^ 0x_M0_15_7E);   // u8 per cell

    // 2. Skeleton (existing — stream + pond authored)
    apply_skeleton_stream_and_pond(chunk, coord);

    // 3. Tree placement loop (replaces existing 10–20 random count)
    target_coverage = lerp(0.45, 0.70, canopy_mean / 255.0);  // chunk-wide target
    while tree_count / total_cells < target_coverage:
        pick a non-skeleton grass cell weighted by canopy[cell] high + moisture[cell]
        species = species_roll(canopy[cell], moisture[cell], rng):
            high-canopy:                 Oak (heaviest weight)
            high-canopy + moist:         + Hazel (understory partner)
            low-canopy + dry:            Holly, Gorse (gorse not a tree — separate)
            edge / mid-canopy:           Ash, Rowan
        place TreeTrunk + cell.tree_species = species
        spawn rate caps in outer ring (visibility) and zero in central spawn disc

    // 4. Decoration placement (new — undergrowth)
    for each grass cell not central spawn-safe disc:
        decoration_weights = compute from (canopy[cell], moisture[cell]):
            shaded + moist:              Fern, Moss heavy
            shaded + dry:                Bracken
            open + dry:                  Gorse
            edge (canopy gradient high): Bramble
            else:                        no decoration most of the time
        if rng.d100() < per-cell decoration chance (~25%):
            cell.decoration = roll_weighted(decoration_weights);

    // 5. LeafLitter ground cover (new — invariant)
    for each grass cell within 1 cell of a TreeTrunk:
        cell.ground_cover = LeafLitter;

    // 6. Existing per-cell debris roll (preserved)
    apply_debris_table(chunk, rng);  // chunkgen.rs:238–297

    // 7. Playability fixup (new — Wyrdlands PRD §5)
    reserve_central_spawn_disc(chunk, radius=4);  // strip TreeTrunk + blocking decoration
    ensure_major_traversable_component(chunk);    // flood-fill; if disconnected, carve corridor
```

## Noise field implementation

Value noise (deterministic from seed). Cheap, ARM-friendly:

```rust
fn value_noise_2d(coord: ChunkCoord, seed: u64) -> [u8; CHUNK_W * CHUNK_H] {
    // 4×4 lattice per chunk; cubic-or-linear interpolation between lattice corners.
    // Lattice values are SplitMix64(seed, chunk_x, chunk_y, lattice_i, lattice_j).
    // Output u8 per cell, ~0.5 ms per chunk on Cortex-A7.
}
```

Two fields per chunk → ~2 KB extra per Chunk in RAM. Discardable — recompute from `(world_seed, chunk_coord)` on chunk-load; do not persist.

Chunk-boundary continuity: lattice extends one cell into each neighbor so the noise is smooth across chunk seams. Wyrdlands PRD §Future Extension Hooks "chunk boundary stitching" is satisfied by this.

## Coverage band

| canopy_mean / 255 | target tree coverage |
|---|---|
| 0.0 | 0.45 (PRD lower bound) |
| 1.0 | 0.70 (PRD upper bound) |
| linear blend in between | |

Coverage = `tree_cells / total_grass_cells`. Walls, water, skeleton features excluded from the denominator.

If the placement loop fails to hit the target after `2 * target_count` rolls (because all the high-weight cells are taken), accept the current count and move on. Always within `[0.35, 0.75]` actual coverage.

## Playability fixup

- **Spawn disc.** Central 9×9 disc (radius 4 around `(CHUNK_W/2, CHUNK_H/2)` = `(20, 15)`) is stripped of `TreeTrunk` and `Gorse`. Existing slice-1 player spawn at `(20, 15)`.
- **Connected component.** Flood-fill from spawn over all walkable cells (terrain.walkable && !decoration.blocks_pass). Compare component size to total walkable count. If < 80%, carve a 1-cell-wide corridor between the spawn component and the largest unconnected component (knock down trees / gorse). Repeat until ≥ 80%.

## Determinism contract

- `(world_seed, chunk_coord) → Chunk` byte-equivalent.
- Same noise fields, same species rolls, same decoration placements, same debris.
- Playability fixup is deterministic (flood-fill order is row-major; corridor carve picks the lexicographically-first cell on the boundary).
- Test: `generate_chunk((0,0), 42)` twice → byte-identical `Chunk`.

## Performance

- Per-chunk generation: ~5–10 ms on desktop (~30–60 ms estimated on Cortex-A7) — well within the chunk-load budget. Chunks load on the order of seconds apart during follow-cam scroll (3×3 ring kept warm by `ensure_player_ring()` in `world.rs:607–613`).
- No per-frame cost.

## Module layout

`src/chunkgen.rs` extends in place; new private module `src/chunkgen/noise.rs` holds `value_noise_2d` and the lattice helpers. `src/flora.rs` (introduced by [[Survival - Species variety - trees and undergrowth]]) holds the species weighting tables; `chunkgen.rs` calls into it.

## Tests

- Determinism: same seed/coord → byte-equivalent chunk (existing test extended to cover species + decorations).
- Coverage: histogram of tree coverage across 1000 random seeds is in `[0.40, 0.75]`.
- Connectivity: ≥ 80% walkable cells reachable from spawn for all 1000 seeds.
- Species distribution: high-canopy noise correlates with Oak frequency; high-moisture with Hazel; open/dry with Holly/Gorse.

## What we explicitly defer to Cornwall slice-2

These belong on the **Cornwall** roadmap (`Cornwall-World.md §6`), not on the Survival kanban:

- Multi-biome polygon overmap (`assets/biomes.json`).
- Sub-biome affinity (AncientEvergreen / MossyOak / DartmoorGranite / etc.).
- Hydrology pass (river/stream procgen across biomes).
- Per-biome temperature/FOV/wind tables.
- BlightedWaste overlay (couples with `wound_intensities` from `Arthurian-Wound.md`).
- Per-biome forage tables / encounters.

This card stays implementable inside the current single-biome chunkgen and is **forward-compatible** with the biome dispatch: the species + decoration weight functions become biome-keyed lookups later, but the noise + coverage + playability machinery is reused.

## Out of scope

- Z-axis / elevation features (cliffs, slopes, overlooks).
- Per-cell flow direction for streams (rejected in [[Survival - Ambient effects - Brainstorm]] anyway).
- Wandering NPCs / fauna placement at chunkgen.

References: Wyrdlands PRD `~/projects/Wyrdlands/docs/prds/08-procedural-forest-worldgen-prd.md`, [[Survival - Tile generation slice 1]] (the chassis), [[Survival - Species variety - trees and undergrowth]] (vocabulary), [[Survival - Plant lifecycle and seasonal foraging]] (operates on what this card places), [[Cornwall-World]] (the biome layer that will eventually wrap this).
