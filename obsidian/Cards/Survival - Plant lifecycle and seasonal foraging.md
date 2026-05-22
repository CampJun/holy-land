Plant state machine + dawn-tick scheduler + the chop-vs-regrow feedback loop. Defines the *behavior* over time for the species and decorations whose *vocabulary* is established in [[Survival - Species variety - trees and undergrowth]].

Depends on [[Survival - Calendar and seasons]] (the dawn tick + `Season` discriminator), [[Survival - Species variety - trees and undergrowth]] (the `Decoration` enum + species list), and the schema-v2 bump in [[Survival - Save schema v2]].

## State machine

```rust
pub enum PlantState {
    Sprout,    // visible but not harvestable
    Mature,    // harvestable parts (e.g. fern fronds) available
    Fruiting,  // bears berries/nuts — drives forage drops to adjacent cells
    Dormant,   // visible but dim tint, no harvest
    Dead,      // about to clear; despawns next dawn
}
```

Stored inside each `Decoration::*` variant (see species card). For plants with no seasonal variation (e.g. `Moss`), the variant omits `state` entirely — moss is always Mature until harvested.

## Per-species schedule

Schedules expressed as `(species, season-or-date-band) → state`. Evaluated at dawn-tick from `calendar_day` and `species`.

| Plant | Spring (Mar 21+) | Summer (Jun 21+) | Autumn (Sep 23+) | Winter (Dec 21+) |
|---|---|---|---|---|
| Herb (existing item, treated as `Decoration::Herb` for lifecycle) | Sprout → Mature | Mature | Dormant | Dead → cleared |
| Fern | Sprout → Mature | Mature | Mature (copper tint) | Dormant (sparse glyph) |
| Bracken | Sprout → Mature | Mature | Mature (copper) | Dead → husk, no harvest |
| Bramble | Mature | Mature → Fruiting (mid-July) | Fruiting → Mature (late Sep) | Mature (bare canes) |
| Gorse | Mature (flowering — yellow tint) | Mature | Mature | Mature |
| Moss | Mature year-round | Mature | Mature | Mature |
| Saplings (Oak/Hazel/Ash/Rowan) | Sprout/Mature based on age | … | … | Dormant |
| Mushroom (transient) | — | — | spawn weighting peaks Oct (LeafLitter cells) | — |
| Trees (mast drops via parent species) | — | — | Acorn/Hazelnut/RowanBerry drop | — |

Holly is evergreen Mature year-round (no `state` field).

## Tick cadence

Existing hook in `main.rs:862–887` already crosses dawn. We extend that path:

```
on_dawn(calendar_day):
    1. season_now = season_of(calendar_day)
    2. if season_now != season_at_last_dawn:
        emit SeasonChanged event
        for each loaded chunk:
            for each cell:
                update GroundCover per season (FallenLeaves spawn/clear, etc.)
                update Decoration.state per (species, calendar_day) lookup
    3. else:
        for each loaded chunk:
            for each cell with a Decoration carrying state:
                if state.next_transition_day <= calendar_day:
                    advance state
    4. mast drops:
        for each TreeTrunk cell whose species is Fruiting on calendar_day:
            roll on neighbors-grass-cells for an Acorn/Hazelnut/RowanBerry item drop
            seeded per (chunk_seed, cell_index, calendar_day) — deterministic
    5. mushroom spawns (Sep 23 – Nov 10):
        for each LeafLitter cell:
            if rng.d100() < daily_spawn_chance:
                place Decoration::Mushroom { kind, expires_day: calendar_day + 3 }
```

Per-chunk; loaded chunks only. Unloaded chunks fast-forward at chunk-load via the lazy-fast-forward path below.

## Lazy fast-forward on chunk-load

Per Wyrdlands PRD §Determinism, plant state at any day must be a pure function of `(species, planted_day, calendar_day, per-cell seed)` — no path dependence except via harvest events.

```
on_chunk_load(chunk, current_day):
    let last_eval_day = chunk.last_lifecycle_eval_day; // u32, stored in ChunkSave
    if last_eval_day == current_day: return;
    for each cell with a Decoration:
        deterministic_evaluate(decoration, last_eval_day, current_day);
        // for mast/mushroom: skip — drops only land while chunk is loaded.
    chunk.last_lifecycle_eval_day = current_day;
```

Mast / mushroom drops do **not** retroactively spawn for the days the chunk was unloaded — only player-visible chunks accumulate forage. This is intentional and matches roguelike norms (otherwise the player could trivially farm by walking away and back).

## Stump regrowth loop

When a tree is chopped (existing `ChopTree` verb from [[Survival - Tile generation slice 1]]):

```
on_chop_tree(cell, calendar_day):
    cell.terrain = TerrainKind::BareDirt;   // current behavior is Grass — switch to BareDirt
    cell.tree_species = None;
    cell.decoration = Decoration::Sapling {
        species: <chopped species>,        // sapling of the same species
        planted_day: calendar_day,
    };
    drop log/firewood items per species table
```

Sapling lifecycle (per dawn):

```
sapling_state(species, planted_day, current_day):
    age = current_day - planted_day
    match (species, age):
        (Hazel, age >= 30):           promote to TerrainKind::TreeTrunk { species: Hazel }
        (Rowan, age >= 45):           promote
        (Ash,   age >= 45):           promote
        (Oak,   age >= 90):           promote
        (Holly, age >= 60):           promote
        _:                            still Sapling
    On promotion: cell.terrain = TreeTrunk; cell.tree_species = species; cell.decoration = None.
    Also: at promotion, terrain mutation written to RunSave.terrain_mutations.
```

Age thresholds are deliberately short in *game days* — at the 24h-per-real-time rate, even 90 days is reasonable inside a long playthrough (Cornwall-Pilgrim.md §5.6 — playthroughs span multiple in-game years). They're tuneable constants in `src/flora.rs`.

## Seasonal foraging table (UI hook)

The currently-foragable set is derivable from `(calendar_day, loaded chunks)`. Useful for an "in season" hint on the foraging menu and for NPC dialog ("the mushrooms are out"). Exposed as:

```rust
pub fn forage_in_season(season: Season) -> Vec<ItemKind> {
    match season {
        Season::Spring => vec![Herb, FernFrond, /* early greens */],
        Season::Summer => vec![Herb, BrambleFruit, FernFrond, BrackenStraw, ...],
        Season::Autumn => vec![Hazelnut, RowanBerry, Acorn, Mushroom, BrambleFruit (early), ...],
        Season::Winter => vec![/* mostly empty; Moss, Gorse, Bramble canes */],
    }
}
```

Cornwall-Pilgrim.md §5.6 + §6.5 endorse this shape.

## Save

`Decoration` variants carry their `state` and `planted_day` via standard serde. `ChunkSave` gains `last_lifecycle_eval_day: u32` with `#[serde(default)]` → 0. Saplings are first-class decorations; trees use `cell.tree_species` (per [[Survival - Species variety - trees and undergrowth]]). All inside the schema-v2 bump.

## Tests

- Deterministic transition: same seed → same state at any calendar_day. Round-trip via save + reload at different days.
- Stump regrowth: ChopTree → wait 30 days → cell is TreeTrunk again (for Hazel). Verify terrain_mutations are written.
- Lazy fast-forward: load chunk on day 100, then on day 200 — second load advances state without intermediate ticks.
- Mast drop determinism: same `(seed, day, cell)` → same drop count / item kind.
- Mushroom expiry: spawn on day N → on day N+4, decoration is cleared.
- Foraging table at known dates (e.g. 1 Aug 1300 should include Herb + BrambleFruit; 21 Dec should include essentially nothing).

## Out of scope

- Player **planting** (sow seeds, tend) — future agriculture card.
- Coppicing / pollarding — multi-cycle harvest from a single tree without chopping. Deferred.
- Tree disease, fire-spread-to-trees, deadfall — separate systems; see [[Survival - Ambient effects - Weather state and wind vector]] for future fire-spread coupling.
- Soil quality / fertility — not modeled.
- Animal mast browsing (deer eat acorns) — deferred to fauna card.

References: [[Survival - Species variety - trees and undergrowth]], [[Survival - Calendar and seasons]], [[Survival - Seasonal ground cover and palette]] (FallenLeaves spawn schedule is parallel to this card's deciduous schedule), [[Survival - Tile generation slice 1]] (ChopTree verb is here today), [[Survival - Save schema v2]], [[Cornwall-Pilgrim]] §5–§6, [[Survival - Skill system URW]] (Foraging skill checks at mature/fruiting harvest).
