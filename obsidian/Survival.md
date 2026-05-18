---

kanban-plugin: board

---

## Ideas



## Drafts

- [ ] [[Survival - Game time clock and needs]] — death gate (slated for phase 14)
	Clock + needs decay + HUD shipped phase 4. Remaining: death-on-zero (DEATH_ENABLED flip + game-over UX) + +0.5/min tent warmth resolution.
- [ ] [[Survival - Command menu]] — radial overlay (phase 15)
	Tap-Y vertical menu shipped phase 7: 14-action registry (SetupCamp added phase 9), availability resolver with greyed-out reasons, A-confirms / B-Y-close, scroll via dpad, name+cost or name+reason per row, description footer. Pickup/EatRation/EatHerb/DrinkWaterskin/PitchTent/UnrollBedroll/SetupCamp wired. Remaining: hold-Y 4-direction radial overlay.
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
	StartFire verb: requires flint+steel in pack + 1 tinder/3 kindling/2 fuel within 3x3 cells or inventory. 60-sec attempt cost. Starting Fire Making 15% + flint+steel +30% = 45% effective success. Success consumes 1+2+1 of the reserve and drops a `Firewood` item with `ItemMetadata::Lit { fuel_seconds: 3600 }` on the player's cell. Failure burns the tinder only. World ticks lit fires per game-second; extinguishes at 0 fuel. **Phase 10 shipped.** FeedFire verb (15-sec cost, +1800s/firewood, no skill check) **shipped phase 12.** Warmth env wiring (lit fire adjacent / Pitched tent / Pitched bedroll all populate `NeedsEnv`) + fire-as-light-source (night FOV bumps to radius 8 when any lit fire is within Chebyshev 5 of the player; recomputes when fires are lit or burn out) **shipped phase 13.**


## Playtest



## Completed




%% kanban:settings
```
{"kanban-plugin":"board","list-collapse":[false,false,false,false,false,false]}
```
%%
