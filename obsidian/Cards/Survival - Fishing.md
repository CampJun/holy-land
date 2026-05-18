Slice-1 fishing: skill-less, single action, low success, long duration.

## Slice-1 spec
- Action: "Try fishing" available when player is adjacent to a pond-water cell.
- Cost: 600 game-seconds (10 in-game minutes per attempt).
- Success rate: **flat 20%** (no skill in slice 1).
- On success: spawn 1 `RawFish` item in the player's cell. +0.5 kg.
- On failure: nothing consumed; just elapsed time and need decay.
- Skill-less = no Fishing XP accrues; this is purely an action.

## UX during the long attempt
- Default view: progress bar.
- Player can swap to time-skip via Select-hold (see `[[Survival - Multi-turn action queue]]`).
- Time-skip simulates each turn so needs decay + interrupt conditions fire correctly.

## Why no skill in slice 1
- Keeps "First skill = Fire Making" as the focused tutorial beat.
- Adds Fishing skill in slice 2, where Fishing XP turns the flat 20% into a skill-modified curve.

## Out of scope (slice 2+)
- Fishing rod item (slice 1 = hand-grab in shallows).
- Fish species and rarity.
- Bait.
- Lakes vs. streams vs. ocean (Cornwall coast in later slices).
- Fishing skill itself.

Reference: `[[Survival - Cooking and herbs]]` (cooked fish), `[[Survival - Multi-turn action queue]]`.
