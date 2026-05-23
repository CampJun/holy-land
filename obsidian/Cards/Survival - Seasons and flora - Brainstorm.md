Hub card. The world looks the same in January as in June; trees are a single placeholder species; herbs are a single static item; the calendar advances day-of-clock but no in-game date is ever surfaced. This card collects the seasonal-rendering work, the procgen-diversity work, and the gameplay loop they unlock — branching into the five follow-up cards below.

The five follow-ups are:

- [[Survival - Calendar and seasons]]
- [[Survival - Seasonal ground cover and palette]]
- [[Survival - Species variety - trees and undergrowth]]
- [[Survival - Plant lifecycle and seasonal foraging]]
- [[Survival - Forest density and species procgen]]

Order: 1 ships first (everything else reads `Season`). 2 and 3 are independent and can land in either order. 4 depends on 3 (lifecycle states attach to decorations + species). 5 depends on 3 (places species + decorations into chunks). All five share the schema-v2 bump in [[Survival - Save schema v2]].

## Anchors already in the project

- **Cornwall-Pilgrim.md §5** already nails the calendar shape: 365-day Gregorian (no leap year), Sarum liturgical, **start date 21 March 1300** (Easter, Spring). Solar season boundaries: Mar 21 / Jun 21 / Sep 23 / Dec 21. `src/calendar.rs` is already named as the planned module.
- **Cornwall-World.md §149** lists the OakWoodland flora set we're building toward: oak, hazel, holly, fern, mossy boulder + acorn, hazel, mushroom forage.
- **Cornwall-Pilgrim.md §6.5** specifies seasonal foraging: herbs scarce winter, berries summer, mushrooms autumn.
- **Cornwall-World.md §369** plans per-biome ambient-temperature offset combined with calendar season — this card unlocks the "season" half.
- **Wyrdlands PRD `08-procedural-forest-worldgen-prd.md`** is the long-horizon reference for forest procgen. We import the *small* parts now (deterministic noise fields for canopy/moisture, coverage-band targets, playability fixup) and defer the *biome layer* parts (sub-biome dispatch, hydrology pass, multi-biome polygon overmap) to Cornwall slice-2 (see [[Survival - Forest density and species procgen]]).

## Miyoo budget impact

Sources: `AGENTS.md`, `src/main.rs:912–1067`, `src/render.rs:9–65`, [[Survival - Miyoo 30 FPS target]].

- **Per-frame cost is unchanged.** Season tint is a function of `(terrain, season)` evaluated per cell in the existing render loop, replacing today's static `TerrainDef.fg/bg` lookup. Same dirty-cell diff path.
- **GroundCover adds one more bg lerp per dirty cell** (lerp from terrain bg toward ground-cover bg). Surface-level color mod works on mmiyoo; only texture color mod is broken. Negligible.
- **Season change is rare** — at most one transition every ~92 in-game days, which during a multi-turn time-skip is one full-screen redraw event. Acceptable.
- **Plant lifecycle ticks only at dawn** (existing hook at `main.rs:862–887`), so the only per-frame cost is the conditional render path for fruiting plants (an item already on the cell). No new per-frame work.
- **Procgen runs at chunk-load**, not per-frame. Adding noise fields + species/decoration passes adds ~tens of ms one-time per chunk; not a frame issue.

## Idea catalog

### Calendar surface

- **C1 — Status-bar date.** Tiny corner glyph block, e.g. `21 Mar · Spring`. Updated only on day rollover.
- **C2 — Feast-day banner.** When the calendar hits a Sarum feast day, a one-frame banner ("Lammas — harvest begins"). Cornwall-Pilgrim.md §5.5 has the date list.
- **C3 — Sleep-until-date verb.** Extends [[Survival - Multi-turn action queue]] Sleep verb with a date-typed target (sleep until Pentecost). Defer.

### Seasonal palette

- **P1 — Per-terrain Season→Palette table.** Authored per terrain; full control. **Chosen.**
- **P2 — Base × season-tint multiply.** Cheaper to author but Grass→white in winter requires lerp-toward (multiply can't lighten). Rejected.
- **P3 — Hybrid: default lerp + per-terrain override.** Pragmatic middle. Rejected in favor of full author control because the snow/fall/spring/summer palette is the whole user-facing payoff of this feature.

### Ground cover

- **G1 — Snow.** Whitens bg outdoors in winter. Lays at first winter dawn; clears at first spring dawn.
- **G2 — FallenLeaves.** Orange/red bg under and around trees in autumn. Spawns over autumn; covered by snow in winter; decays in spring.
- **G3 — LeafLitter.** Permanent brown bg under canopy regardless of season. Drives mushroom-spawn weighting in [[Survival - Plant lifecycle and seasonal foraging]].
- **G4 — Frost** (deferred). Dawn-only blue-white speckle in late autumn / early winter. Skipped for v1 to limit variants.

### Species + undergrowth

- **S1 — Tree species: Oak, Hazel, Holly, Ash, Rowan.** Glyph + per-species-per-season tint. Drives forage tables.
- **S2 — Undergrowth decoration layer.** New `CellState.decoration` field (Fern / Moss / Bramble / Bracken / Gorse / Sapling / None). Interactive: harvest verbs target the decoration; harvest yields drop as items; decoration is set to None on full harvest.
- **S3 — Mushrooms as transient cell items** (not decorations). Spawn on LeafLitter in autumn, persist ~3 days, disappear. See lifecycle card.

### Procgen diversity

- **PG1 — Deterministic noise fields (canopy + moisture).** Value-noise from `(world_seed, chunk_coord)`. Drives species and undergrowth placement weights.
- **PG2 — Coverage-band targets.** Tree coverage in `[45%, 70%]` per Wyrdlands PRD; replaces today's 10–20 fixed count.
- **PG3 — Playability fixup.** Enforce major traversable component + central spawn-safe disc. Wyrdlands PRD §Pipeline pt 5.
- **PG4 — Sub-biome dispatch / hydrology.** Deferred to Cornwall slice-2 ([[Cornwall-World]] biome polygons).

### Lifecycle

- **L1 — Plant state machine.** `PlantState { Sprout / Mature / Fruiting / Dormant / Dead, planted_day }`. Per-species transition schedule.
- **L2 — Stump regrowth loop.** Chopped tree spawns `Decoration::Sapling { planted_day }`; promotes back to TreeTrunk after N days. Closes the chop-vs-regrow feedback.
- **L3 — Lazy fast-forward.** Plants in unloaded chunks tick at chunk-load by computing the deterministic state for `current_day - planted_day`. No per-frame world-wide sim.

## Decisions

- **Calendar shape:** Gregorian 365-day (no leap), Sarum, start 21 Mar 1300. From Cornwall-Pilgrim.md §5; no re-litigation.
- **Color hook:** per-cell `CellState.ground_cover` layer **plus** season-aware `TerrainDef` table (`bg_for_season(s)` / `fg_for_season(s)`). Author both. Render: terrain seasonal bg → lerp toward ground-cover bg.
- **Ground cover variants for v1:** Snow, FallenLeaves, LeafLitter. Skip Frost.
- **Procgen scope now:** species variety + undergrowth + noise-driven placement + coverage targets + playability fixup. Defer sub-biomes and hydrology to Cornwall slice-2.
- **Lifecycle:** full state-machine sim, dawn-ticked, lazy-fast-forward at chunk-load. Stumps regrow via `Decoration::Sapling`.
- **Undergrowth model:** decoration field on CellState (interactive), with harvest verbs that drop yields as items (split "plant" from "yield"). User explicitly wants moss → poultice ingredient, fern cut/dug, etc.
- **Tick cadence:** dawn (`main.rs:862–887` already crosses dawn). One hook, all season + lifecycle work runs there.
- **Schema:** all changes ride the existing [[Survival - Save schema v2]] bump. No separate v3.
- **Module layout:** `src/calendar.rs` (already named in Cornwall-Pilgrim.md), `src/flora.rs` (new — species + decoration + lifecycle), `src/chunkgen.rs` (extended in place).

## What this card is not

Not a plan to ship every idea above. The five follow-up cards are the actual planned work. Feast-day banner (C2), sleep-until-date (C3), and frost (G4) stay in this hub as backlog. The Cornwall slice-2 biome work (sub-biomes, hydrology, polygon overmap) is tracked separately in `Cornwall-World.md` and is **not** part of this hub.

References: [[Cornwall-World]], [[Cornwall-Pilgrim]], [[Survival - Tile generation slice 1]], [[Survival - Day night cycle]], [[Survival - Save schema v2]], [[Survival - Ambient effects - Brainstorm]] (D1 Time-of-day tint subsumed here), [[Survival - Game time clock and needs]], [[Survival - FOV]].
