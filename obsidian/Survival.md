---

kanban-plugin: board

---

## Ideas



## Drafts

- [ ] [[Survival - Chunk and per-cell items]]
	Single-chunk world + per-cell item lists. The storage substrate everything else sits on.
- [ ] [[Survival - ItemInstance and weights]]
	Replace stack-based inventory with `Vec<ItemInstance>`; per-item weight grams + 15kg pack cap.
- [ ] [[Survival - Game time clock and needs]]
	Per-action game clock + four needs (Thirst, Hunger, Sleep, Warmth). Death enabled at 0.
- [ ] [[Survival - Day night cycle]]
	24-hour clock, dawn/dusk tint, save-on-dawn. Drives Warmth decay and FOV radius.
- [ ] [[Survival - FOV]]
	Recursive shadowcasting; trees block sight; lit fire extends night vision.
- [ ] [[Survival - Command menu]]
	Y-button context menu: tap = vertical list, hold = 4-action radial, Select-hold = view toggle.
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



## Playtest

- [ ] [[Blessing balance pass]]
	Playtest blessing power; tune so no blessing is obviously dominant.
- [ ] [[Demon difficulty curve]]
	Playtest the regional ramp; tune HP, damage, AI chase speed.
- [ ] [[Run pacing and feel]]
	Playtest the oasis → blessing → wilderness → death rhythm.


## Completed




%% kanban:settings
```
{"kanban-plugin":"board","list-collapse":[false,false,false,false,false,false]}
```
%%
