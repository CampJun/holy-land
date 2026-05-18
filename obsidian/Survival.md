---

kanban-plugin: board

---

## Ideas



## Drafts

- [ ] [[Survival - Game time clock and needs]] — death gate (slated for phase 14)
	Clock + needs decay + HUD shipped phase 4. Remaining: death-on-zero (DEATH_ENABLED flip + game-over UX) + +0.5/min tent warmth resolution.
- [ ] [[Survival - Command menu]] — radial overlay (phase 15)
	Tap-Y vertical menu shipped phase 7: 14-action registry (SetupCamp added phase 9), availability resolver with greyed-out reasons, A-confirms / B-Y-close, scroll via dpad, name+cost or name+reason per row, description footer. Pickup/EatRation/EatHerb/DrinkWaterskin/PitchTent/UnrollBedroll/SetupCamp wired. Remaining: hold-Y 4-direction radial overlay.
- [ ] [[Survival - Multi-turn action queue]] — Phase 13 will plug in structure placement on completion
	Queue+tick+cancel+toggle+save-roundtrip shipped phase 9. PitchTent (300s), UnrollBedroll (30s), SetupCamp (queues both) live. Progress-bar banner overlay; Select toggles to time-skip (simulates each second for interrupts). Need-critical interrupt threshold = 10. Penalty amplified at queue time, not per-second. Remaining: structure entities (Tent, Bedroll) need actual world placement once warmth/sleep wiring lands in phase 13.
- [ ] [[Survival - Multi-turn action queue]]
	Progress-bar / time-skip modes; simulate every elapsed turn so interrupts can fire.
- [ ] [[Survival - Skill system URW]]
	0–100 percentile skills; `1d100 ≤ skill + mods`; +1 fail, +5 success; daily 20 XP cap.
- [ ] [[Survival - Fire Making]]
	First skill. Flint+steel gives 45% start. Gates first-night survival.
- [ ] [[Survival - Drag mechanic]]
	Whole-log drag, chop at destination. 2× movement cost while dragging.
- [ ] [[Survival - Cooking and herbs]]
	Pan-on-fire cooks raw→cooked. Three herb uses: eat raw, brew tea, season cooked food.
- [ ] [[Survival - Fishing]]
	Slice-1 fishing: skill-less, flat 20%, 600 game-sec/attempt. Skill comes in slice 2.
- [ ] [[Survival - Tile generation slice 1]]
	40×30 single forest tile: authored stream+pond skeleton + seeded trees/herbs/debris.
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
	Recursive shadowcasting; radius 20 day / 3 night; explored cells dim-rendered after leaving FOV; explored bits round-trip through save. Tree-blocker integration arrives with phase 11 tile gen; fire-as-light-source with phase 10. **Phase 6 shipped.**


## Playtest



## Completed




%% kanban:settings
```
{"kanban-plugin":"board","list-collapse":[false,false,false,false,false,false]}
```
%%
