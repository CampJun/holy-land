---

kanban-plugin: board

---

## Ideas



## Drafts

- [ ] [[Survival - Game time clock and needs]]
	Clock + needs decay + HUD shipped phase 4. Death gate shipped phase 14: `DEATH_ENABLED=true`; any-need-at-zero triggers a `* DEAD *` overlay with cause-of-death epitaph (Thirst/Hunger/Cold/Exhaustion priority); A starts a new run (wipes `run.cbor`, regens world, keeps meta xp/affinity), Start quits. Active multi-turn cancels on death. Remaining: +0.5/min tent warmth precision (kept under this card; not phase-blocking).
- [ ] [[Survival - Command menu]]
	Tap-Y vertical menu shipped phase 7. Hold-Y 4-direction radial overlay **shipped phase 15**: 250ms hold threshold promotes Y-press to a 25×5 radial showing Pickup/Eat/PickHerb/Drink in cardinal slots (greyed via panel_dim_fg when unavailable). Dpad direction while held fires the verb and closes; release-Y without direction closes silently. Quick tap-Y (release before threshold, no direction) still opens the vertical menu — Y press is owned by the hold state machine in main.rs so the two paths don't conflict.
- [ ] [[Survival - Multi-turn action queue]]
	Queue+tick+cancel+toggle+save-roundtrip shipped phase 9. PitchTent (300s), UnrollBedroll (30s), SetupCamp (queues both) live. Pitched tents/bedrolls feed warmth shelter flags as of phase 13a. Progress-bar banner overlay; Select toggles to time-skip (simulates each second for interrupts). Need-critical interrupt threshold = 10. Penalty amplified at queue time, not per-second.
- [ ] [[Survival - Drag mechanic]]
	Whole-log drag, chop at destination. 2× movement cost while dragging.
- [ ] [[Survival - Cooking and herbs]]
	Pan-on-fire cooks raw→cooked. Three herb uses: eat raw, brew tea, season cooked food.
- [ ] [[Survival - Fishing]]
	Slice-1 fishing: skill-less, flat 20%, 600 game-sec/attempt. Skill comes in slice 2.
- [ ] [[Survival - Save schema v2]]
	Schema bump for the redesign. Friendly-reject v1 saves.


## Implemented (additions)

- Sleep verb **shipped phase 16**: queues a single-step multi-turn whose target is min(next-dawn, 8h) via the new `World::queue_multi_turn_raw` (no need-penalty amplification — sleep is wall-clock). Needs decay normally during the queue, so a hungry/cold player can interrupt early. `complete_step(Sleep)` restores Sleep to NEED_MAX. Explicit "Sleep until..." picker is a polish item.


## Planning



## Implemented

- [ ] [[Survival - Chunk and per-cell items]]
	Single-chunk world + per-cell item lists. The storage substrate everything else sits on. **Phase 3 shipped on cd440a2.**
- [ ] [[Survival - ItemInstance and weights]]
	`Vec<ItemInstance>` (stackable fungibles, unique uniques) + per-item weight grams + 15kg pack cap. **Phase 3 shipped on cd440a2.**
- Phase 4 (clock + needs HUD, death gate off): partial-ship of [[Survival - Game time clock and needs]] — see Drafts column for the death-gate work.
- [ ] [[Survival - Day night cycle]]
	24-hour clock with dusk/dawn screen tint (linear blend 19:30-20:30 / 05:30-06:30) and auto-save fired at each 06:00 dawn crossing. **Phase 5 shipped.**
- [ ] [[Survival - FOV]]
	Recursive shadowcasting; radius 20 day / 3 night; explored cells dim-rendered after leaving FOV; explored bits round-trip through save. Tree-blocker integration arrives with phase 11 tile gen; fire-as-light-source with phase 13 alongside warmth. **Phase 6 shipped.**
- [ ] [[Survival - Skill system URW]]
	0–100 percentile skills; `1d100 ≤ skill + mods` clamped [5,95]; +1 fail, +5 success; daily 20 XP cap that resets at 06:00 dawn; level-up when daily_xp ≥ (5 + value/5). xorshift32 RNG state persists across save to prevent save-scumming. **Phase 10 shipped.** Slice-2 adds Fishing/Cookery/Foraging against the same chassis.
- [ ] [[Survival - Tile generation slice 1]]
	TerrainKind expanded Grass / BareDirt / SandShore / TreeTrunk / StreamWater / PondWater / Wall. TerrainDef table per STYLE.md §3.5. Authored skeleton: stream from N edge into ellipse pond at (28, 22). Seeded population: 10-20 outer-ring trees + 3-5 herb patches + per-grass-cell debris (twigs/sticks/firewood/grass/stone/moss/mud). Deterministic per (world_seed, chunk_coord). Chunkgen handles slice-2 multi-chunk expansion. ChopTree / PickHerb / DrinkFromStream / FillWaterskin verbs all live + terrain-mutation save round-trip. **Phase 11 + 11b shipped.**
- [ ] [[Survival - Fire Making]]
	StartFire verb: requires flint+steel in pack + 1 tinder/3 kindling/2 fuel within 3x3 cells or inventory. 60-sec attempt cost. Starting Fire Making 15% + flint+steel +30% = 45% effective success. Success consumes 1+2+1 of the reserve and drops a `Firewood` item with `ItemMetadata::Lit { fuel_seconds: 3600 }` on the player's cell. Failure burns the tinder only. World ticks lit fires per game-second; extinguishes at 0 fuel. **Phase 10 shipped.** FeedFire verb (15-sec cost, +1800s/firewood, no skill check) **shipped phase 12.** Warmth env wiring (lit fire adjacent / Pitched tent / Pitched bedroll all populate `NeedsEnv`) **shipped phase 13a.** Fire-as-light-source: phase 13b stopgap (whole-FOV bump to radius 8 near fire) was replaced by **phase 13c** per-light-source FOV — each Lit-metadata carrier shadowcasts its own radius-5 disc independently; player keeps their own radius-3 night FOV; visible set is the union. Fire-lit cells (new `CellState.fire_lit` flag, transient) get a warm-yellow overlay at night. Zero-alloc per recompute via stack-buffered `collect_light_sources_into` (cap 16). Walls block fire light by the same shadowcast rule.


## Playtest



## Completed




%% kanban:settings
```
{"kanban-plugin":"board","list-collapse":[false,false,false,false,false,false]}
```
%%
