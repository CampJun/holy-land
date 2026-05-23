---

kanban-plugin: board

---

## Ideas

- [ ] [[Survival - Ambient effects - Brainstorm]]
	Hub card for ambient effects (lighting, fire, wind, water). Catalog of candidates + decisions. Branches into the four follow-up cards below.
- [ ] [[Survival - Ambient effects - Bayer dither lighting]]
	Per-pixel 4×4 Bayer dither (4 levels) inside each 16×16 cell, replacing the smooth FOV/light blend at `main.rs:1019-1046`. Same dirty-cell cost as today.
- [ ] [[Survival - Ambient effects - Glyph cycle framework and demo]]
	`GlyphCycle` effect type + a standalone `cargo --example glyph_cycle_explorer` binary for A/B-ing fire-glyph cycle candidates on desktop and Miyoo.
- [ ] [[Survival - Ambient effects - Weather state and wind vector]]
	`WeatherState { Calm/Breezy/Gusty/Storm, Dir8 wind_dir, u8 intensity }` state machine. Save-persistent (schema bump + migration). Consumers: smoke drift, future fire-spread, future leaves/rain.
- [ ] [[Survival - Ambient effects - Smoke particles and FOV]]
	Transient `SmokeField` particle list emitted from lit fires; drifts on the weather wind vector. Each smoke cell adds a fixed single-cell visibility penalty in `recompute_fov`. Not saved.
- [ ] [[Survival - Seasons and flora - Brainstorm]]
	Hub card for seasons + procedural flora (calendar, ground cover, species variety, plant lifecycle, forest procgen). Calendar shape already settled by Cornwall-Pilgrim.md §5 (Gregorian 365-day, Sarum, start 21 Mar 1300). Branches into the five follow-up cards below.


## Drafts

- [ ] [[Survival - Game time clock and needs]]
	Clock + needs decay + HUD shipped phase 4. Death gate shipped phase 14: `DEATH_ENABLED=true`; any-need-at-zero triggers a `* DEAD *` overlay with cause-of-death epitaph (Thirst/Hunger/Cold/Exhaustion priority); A starts a new run (wipes `run.cbor`, regens world, keeps meta xp/affinity), Start quits. Active multi-turn cancels on death. Remaining: +0.5/min tent warmth precision (kept under this card; not phase-blocking).
- [ ] [[Survival - Drag mechanic]]
	Whole-log drag, chop at destination. 2× movement cost while dragging.
- [ ] [[Survival - Save schema v2]]
	Schema bump for the redesign. Friendly-reject v1 saves. Absorbs the seasons/flora additions (`calendar_day`, `CellState.ground_cover`, `CellState.tree_species`, `CellState.decoration`, `ChunkSave.last_lifecycle_eval_day`).
- [ ] [[Survival - Miyoo 30 FPS target]]
	Drop Miyoo to 30 FPS, keep desktop at 60. Cfg-split `TARGET_FRAME` in `main.rs:43`. Battery + pacer consistency win; pacer oversleep slop is ~30% of budget at 60 FPS but ~10% at 33 ms target so frames actually land more consistently. Watch out for `MULTI_TURN_GAME_SEC_PER_FRAME` (`world.rs:55`) — at 30 FPS all multi-turn wall-clock durations double; bump the constant on ARM or leave it.
- [ ] [[Survival - Seasonal ground cover and palette]]
	Season-aware `TerrainDef` table (per-terrain Season→Palette) replaces flat `fg/bg`. Per-cell `GroundCover { None, FallenLeaves, LeafLitter }` field on CellState; Snow is render-time-only from `(season == Winter && outdoor)`. Two extra bg lerps per dirty cell; mmiyoo surface color mod handles it.
- [ ] [[Survival - Species variety - trees and undergrowth]]
	Tree species (Oak/Hazel/Holly/Ash/Rowan) via `CellState.tree_species`; per-species seasonal tints replace `TREE_TINT_VARIANTS`. Undergrowth as `CellState.decoration` (Fern/Moss/Bramble/Bracken/Gorse/Sapling/Mushroom) with interactive harvest verbs: HarvestMoss → poultice ingredient, CutFern/DigFern, CutGorse, CutBracken, PickBrambleFruit, PickMushroom. New ItemKind variants for yields.
- [ ] [[Survival - Plant lifecycle and seasonal foraging]]
	`PlantState { Sprout/Mature/Fruiting/Dormant/Dead }` state machine driven by `(species, calendar_day)`. Dawn-tick advances loaded chunks; chunk-load lazy fast-forwards. Stump regrowth: ChopTree spawns `Decoration::Sapling`; sapling promotes to TreeTrunk after N days (30 Hazel, 90 Oak). Mast drops (acorns, hazelnuts, rowanberries) on fruiting tree neighbors in autumn; mushrooms transient on LeafLitter cells.
- [ ] [[Survival - Forest density and species procgen]]
	Extends `chunkgen.rs:34` with deterministic value-noise fields (canopy + moisture), coverage-band targets (45–70% per Wyrdlands PRD §08), noise-weighted species/decoration placement, central spawn-disc reservation, and connectivity fixup. Single-biome only — sub-biome dispatch + hydrology deferred to Cornwall slice-2.
- [ ] [[Survival - Cornwall overmap and biome dispatch]]
	Slice-2 layer above `chunkgen.rs`: a coarse `~64×64` overmap of region-cells (`Biome { Moor, Oakwood, Hazelwood, Coast, Heath, Village, Tor, Bog }`) drives per-chunk species tables + hydrology. CDDA-inspired (see `/home/campjun/Documents/Cataclysm-DDA-0.H-RELEASE/src/overmap.cpp` + `overmap_noise.cpp`) but shrunk hard for Miyoo: enum-keyed, no JSON pipeline, no `oter_id` indirection. New `src/overmap.rs`: `OvermapCell { biome, elevation, moisture, has_road, has_river }`, `generate(world_seed) -> [[OvermapCell; 64]; 64]` runs at world-init in this fixed order:
	1. **Authored coast silhouette** — Cornwall's outline + Tamar boundary stamped from a baked-in 64×64 land/sea mask (period-accurate north + south coast, Lizard, Land's End, Mount's Bay). Same every seed; ensures Tintagel-on-north-coast invariant. Pre-painted Coast cells around the silhouette.
	2. **Elevation + moisture noise** (2 Simplex passes) over land cells only.
	3. **Sink fill** — naive iterative Planchon-Darboux: any land cell lower than all 8 neighbors gets raised by ε; repeat to fixed point (<10 passes on 4096 cells). Without this, step 4 dies in local minima.
	4. **Hydrology** — pre-place ~6 fixed river sources at high-elevation interior cells (Tamar, Fowey, Camel, Fal, Lynher, Helford headwaters); A* each to nearest coast cell with cost = `1 − gradient` (prefers downhill, allows uphill at penalty when needed). Cells on the path get `has_river = true`. Cheaper and more legible than D8 flow-accumulation for a handful of named rivers.
	5. **Biome assignment** — explicit Whittaker-style table on `(elevation, moisture, coast_distance)`: `coast_distance ≤ 1` → Coast; `elev > 0.75 ∧ moisture < 0.4` → Tor; `elev < 0.2 ∧ moisture > 0.7` → Bog; `moisture > 0.6` → Oakwood; `moisture mid` → Hazelwood; `moisture low ∧ elev low` → Heath; default → Moor. Fully deterministic lookup, no extra noise pass.
	6. **Named POI override** — fixed-coordinate stamps (Tintagel, Glastonbury-direction priories per `Cornwall-Sites.md`, Mên-an-Tol stone circle, Restormel) overwrite biome to `Village` (or appropriate POI biome) in a 3×3 patch around their authored coord. POIs are *always* in their canonical location regardless of seed.
	7. **Procedural village placement** — score function over remaining cells: `+river_adjacent +arable(flat ∧ moderate_moisture) +sheltered −bog −cliff_exposed −high_elevation +harbor(coast ∧ flat ∧ sheltered_cove)`. Greedy-place N villages by descending score with Poisson-disk min-spacing ≥6 cells. Replaces the "settlement-suitability noise" idea from the CDDA survey — settlements aren't noise blobs, they're scored locations.
	8. **Road network** — minimum spanning tree over all village + POI sites, **augmented with Gabriel-graph edges** (any pair where Euclidean distance < 1.3× current graph-shortest distance gets a direct edge) so the network has redundancy, not a single-cut failure. Each edge routed by A* with terrain cost = `distance × (1 + slope_penalty + bog_penalty − ridgeline_bonus)`, matching how Cornish trackways actually followed ridgelines and skirted bogs. Cells on routed edges get `has_road = true`.
	
	Determinism per (`world_seed`, ox, oy) means saves only store deltas — overmap regenerates identically on load (cache in RAM, don't persist the grid). Open question: what counts as an overmap-level delta (player-burned village? road damage?) — punt to follow-up card; v1 ships immutable overmap, chunk-level changes only. `chunkgen.rs` reads the overmap cell for `(cx, cy)` + 4 neighbors and dispatches to a biome-specific species/coverage table (subsumes the single-biome 45–70% band from the prior card). `has_river` / `has_road` flags drive intra-chunk routing — river cells stamp `StreamWater` along the chunk's gradient; road cells stamp a beaten-earth path. Schema bump folds into [[Survival - Save schema v2]]; coordinates everywhere already widened to `i64` per `main.rs` follow-cam contract. Scale note: 64 cells × ~32 tiles/chunk ≈ 2048 tiles ≈ 30-min E-W walk at 5 steps/sec — may need bumping to 96 or 128 once chunk-tile-count is locked, if Cornwall needs to *feel* like Cornwall.
- [ ] [[Survival - Combat - Action economy and turn flow]]
	CDDA speed + move-cost economy: each actor accrues `speed` moves per tick, actions deduct move-cost, continuous tempo (no "100 AP per turn" budget). Introduces `Attack` / `Shoot` / `Brace` / `Grapple` / `Throw` / `Disarm` / `Reload` verbs. Supersedes the old Action point system card.
- [ ] [[Survival - Combat - Weapon skills and proficiencies]]
	Two-axis CDDA model: Melee + Ranged + Dodge + Block + 3 armor skills (Padded/Mail/Plate) on top; 11 weapon proficiencies underneath (Knife, Sword, Falchion, Axe, Mace, Quarterstaff, Spear, Gisarme, Bow, Crossbow, Unarmed). Unarmed = wrestling: strikes + Grapple/Throw/Disarm verbs (historically attested Devon wrestling).
- [ ] [[Survival - Combat - Armor model]]
	CDDA-style layered armor on six body parts (head, torso, L/R arm, L/R leg). Per-piece coverage % + per-type DR (bash/cut/stab) + encumbrance. Period categories: Padded (gambeson/doublet) / Mail (hauberk/chausses/coif) / Plate (coat-of-plates, great helm, bascinet). Three armor skills mitigate the wearer's encumbrance penalties.
- [ ] [[Survival - Combat - Damage math and hit roll]]
	Normal-distribution contested roll (CDDA's actual model). Margin ≥ 0 = hit; ≥ 15 = crit (armor bypass + damage bonus + body-part control). Bash/cut/stab damage triplet, weapon-vs-armor rock-paper-scissors. Per-body-part HP (torso ~80, head ~40, limbs ~60). Binary crippling at 0 HP (drop weapon / move-cost x2 / death on head or torso). Aimed shots for both melee and ranged. Supersedes the old Combat math foundation card.
- [ ] [[Survival - Combat - Reach and ranged]]
	Polearm reach-2 (spear, gisarme) with CDDA's no-reach-adjacency damage penalty. Bow + crossbow ranged in v1: ammo as stackable items in pack, equipped quiver, recoverable arrows with break chance. Visible to-hit % shown only when targeting ranged (melee stays hidden-roll).
- [ ] [[Survival - Combat - Stamina]]
	Lighter-touch stamina pool. Only running and heavy actions (grapple, brace, crossbow reload, aimed swings) cost stamina; normal swings are free. Scales with Stam attribute. Encumbrance raises stamina cost of heavy actions. Out-of-stamina state penalties.
- [ ] [[Survival - Combat - Status armament tiers]]
	Statute-of-Winchester-style enemy arming: humanoid enemies roll loadout from a tier table (Rabble / Yeoman / Sergeant / Knight). Replaces piece-by-piece authoring. Drops match the rolled gear — natural progression by tier-punching. Period-accurate to Cornwall/Devon ~1300.
- [ ] [[Survival - Combat - Mythic and faerie arms]]
	Scaffold for Arthurian / faerie / demon-forged relic loot that intentionally breaks period rules. Saint relics, faerie smithing, demon-forged, and singular legendary tiers. Mechanics via mythic-modifier flags on ItemMetadata. v1 ships scaffold only; first specific mythic item lands with the first Arthurian quest.
- [ ] [[Survival - Combat - Skill XP sources]]
	Per-skill XP rules for all combat skills + 11 weapon proficiencies. CDDA-style use-based attribute training (no skill→attribute milestones — that's dropped). Hundreds of relevant actions per +1 attribute. Daily XP cap matches the existing 20-XP-at-dawn chassis. Supersedes the combat sections of the old Skill XP sources card.
- [ ] [[Survival - Combat - Bestiary slice 1]]
	First enemy: Cornish bandit (Yeoman tier). Single humanoid that exercises every locked system — melee, reach (if rolled spear), ranged (if rolled bow), armor layering, body parts, status arming, AI chase+bump+ranged. Death drops the rolled loadout — immediate Rabble→Yeoman gear progression for the player.


## Implemented

- [ ] [[Survival - Chunk and per-cell items]]
	Single-chunk world + per-cell item lists. The storage substrate everything else sits on. **Phase 3 shipped on cd440a2.**
- [ ] [[Survival - ItemInstance and weights]]
	`Vec<ItemInstance>` (stackable fungibles, unique uniques) + per-item weight grams + 15kg pack cap. **Phase 3 shipped on cd440a2.**
- [ ] Phase 4 (clock + needs HUD, death gate off): partial-ship of [[Survival - Game time clock and needs]] — see Drafts column for the death-gate work.
- [ ] [[Survival - Day night cycle]]
	24-hour clock with dusk/dawn screen tint (linear blend 19:30-20:30 / 05:30-06:30) and auto-save fired at each 06:00 dawn crossing. **Phase 5 shipped.**
- [ ] [[Survival - FOV]]
	Recursive shadowcasting; radius 20 day / 3 night; explored cells dim-rendered after leaving FOV; explored bits round-trip through save. Tree-blocker integration arrives with phase 11 tile gen; fire-as-light-source with phase 13 alongside warmth. **Phase 6 shipped.**
- [ ] [[Survival - Skill system URW]]
	0–100 percentile skills; `1d100 ≤ skill + mods` clamped [5,95]; +1 fail, +5 success; daily 20 XP cap that resets at 06:00 dawn; level-up when daily_xp ≥ (5 + value/5). xorshift32 RNG state persists across save to prevent save-scumming. **Phase 10 shipped.** Slice-2 adds Fishing/Cookery/Foraging against the same chassis.
- [ ] [[Survival - Tile generation slice 1]]
	TerrainKind expanded Grass / BareDirt / SandShore / TreeTrunk / StreamWater / PondWater / Wall. TerrainDef table per STYLE.md §3.5. Authored skeleton: stream from N edge into ellipse pond at (28, 22). Seeded population: 10-20 outer-ring trees + 3-5 herb patches + per-grass-cell debris (twigs/sticks/firewood/grass/stone/moss/mud). Deterministic per (world_seed, chunk_coord). Chunkgen handles slice-2 multi-chunk expansion. ChopTree / PickHerb / DrinkFromStream / FillWaterskin verbs all live + terrain-mutation save round-trip. **Phase 11 + 11b shipped.**
- [ ] [[Survival - Command menu]]
	Tap-Y vertical menu shipped phase 7. Hold-Y 4-direction radial overlay **shipped phase 15**: 250ms hold threshold promotes Y-press to a 25×5 radial showing Pickup/Eat/PickHerb/Drink in cardinal slots (greyed via panel_dim_fg when unavailable). Dpad direction while held fires the verb and closes; release-Y without direction closes silently. Quick tap-Y (release before threshold, no direction) still opens the vertical menu — Y press is owned by the hold state machine in main.rs so the two paths don't conflict.
- [ ] [[Survival - Multi-turn action queue]]
	Queue+tick+cancel+toggle+save-roundtrip shipped phase 9. PitchTent (300s), UnrollBedroll (30s), SetupCamp (queues both) live. Pitched tents/bedrolls feed warmth shelter flags as of phase 13a. Progress-bar banner overlay; Select toggles to time-skip (simulates each second for interrupts). Need-critical interrupt threshold = 10. Penalty amplified at queue time, not per-second. **Phase 9 + 13a shipped.**
- [ ] [[Survival - Fishing]]
	Slice-1 fishing **shipped phase 17**: requires pond adjacent, flat 20% via `world.rng.d100()`, 600 game-sec/attempt regardless of outcome, success drops one Fish item on the player's cell. Skill-less for slice 1 per the card; slice 2 will add the Fishing skill chassis on the existing skill.rs infra.
- [ ] [[Survival - Fire Making]]
	StartFire verb: requires flint+steel in pack + 1 tinder/3 kindling/2 fuel within 3x3 cells or inventory. 60-sec attempt cost. Starting Fire Making 15% + flint+steel +30% = 45% effective success. Success consumes 1+2+1 of the reserve and drops a `Firewood` item with `ItemMetadata::Lit { fuel_seconds: 3600 }` on the player's cell. Failure burns the tinder only. World ticks lit fires per game-second; extinguishes at 0 fuel. **Phase 10 shipped.** FeedFire verb (15-sec cost, +1800s/firewood, no skill check) **shipped phase 12.** Warmth env wiring (lit fire adjacent / Pitched tent / Pitched bedroll all populate `NeedsEnv`) **shipped phase 13a.** Fire-as-light-source: phase 13b stopgap (whole-FOV bump to radius 8 near fire) was replaced by **phase 13c** per-light-source FOV — each Lit-metadata carrier shadowcasts its own radius-5 disc independently; player keeps their own radius-3 night FOV; visible set is the union. Fire-lit cells (new `CellState.fire_lit` flag, transient) get a warm-yellow overlay at night. Zero-alloc per recompute via stack-buffered `collect_light_sources_into` (cap 16). Walls block fire light by the same shadowcast rule.
- [ ] Sleep verb **shipped phase 16**: queues a single-step multi-turn whose target is min(next-dawn, 8h) via the new `World::queue_multi_turn_raw` (no need-penalty amplification — sleep is wall-clock). Needs decay normally during the queue, so a hungry/cold player can interrupt early. `complete_step(Sleep)` restores Sleep to NEED_MAX. Explicit "Sleep until..." picker is a polish item.
- [ ] Crafting system + cooking card **shipped on `survival/crafting`**: new `Crafting` tab in the Select-button info hub (between Inventory and Skills) drives a CDDA-style recipe registry. Verbs: PlacePan (pan + adjacent Lit fire → PannedOnFire cookware that owns the fire's fuel), PickUpPan (reverses with fuel transferred back to a Lit firewood), CookFish (5s setup; pan ticks the 120s passive cook), SeasonPan (5s; sets a modifier bit on the in-flight cook), TakeFromPan (lifts the finished cook out as a single `ItemKind::Cooked` whose metadata carries base/state/seasonings — no `CookedFish`/`BurntFish`/`HerbFish` permutation explosion), EatHerb (pre-existing). Ingredient lookup spans pack + 1-cell radius via `count_reachable_kind`/`consume_one_reachable`. Cookware ticks live in `World::tick_cookware` alongside `tick_fires`; fuel exhaustion mid-cook spills food onto the cell as Burnt. All metadata variants round-trip through CBOR with no schema bump. Brew-tea + recipe-start modifier picker parked for a follow-up. Recipe modifiers (Herb, future Salt/Oil) live as a `Seasonings(u8)` bitfield on the output; one base recipe covers every output combination.
- [ ] Follow-cam **shipped on `survival/follow-cam`** (commit 613783f): player-centered camera + chunk ring + scroll-blit. No card; architectural enabler for slice-2 multi-chunk world.
- [ ] [[Survival - Calendar and seasons]]
	365-day Gregorian (no leap), Sarum liturgical, start 21 Mar 1300 (Easter, Spring). Solar season boundaries (Mar 21 / Jun 21 / Sep 23 / Dec 21). New `src/calendar.rs`; `World.calendar_day: u32` + `RunSave.calendar_day` advances at every midnight crossing; HUD shows `21 Mar Spring` on row 2 right. **Phase A of the seasons/flora cluster shipped.** Schema-v2 bump landed alongside: v1 saves now friendly-reject ("This save belongs to the Holy Land design. Start a new game…") via [[Survival - Save schema v2]] — the v2 card stays in Drafts until the per-cell season/species fields land in later phases.


## Playtest



## Completed





%% kanban:settings
```
{"kanban-plugin":"board","list-collapse":[false,false,false,false,false,false]}
```
%%