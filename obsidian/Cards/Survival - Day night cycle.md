24-hour in-game clock with dawn/dusk transitions and shelter-driven warmth.

## Schedule
- 24-hour in-game clock derived from `clock_seconds`.
- 06:00 – 20:00 = day. 20:00 – 06:00 = night.
- Slice-1 spawn at **14:00**.

## Visual
- Screen tint blends linearly over the hour around 20:00 (dusk) and 06:00 (dawn).
- Tint applied as a `bg` darkening pass during render — works with the per-cell-diff renderer; just causes a full redraw at each clock-tick that crosses a tint step.

## Mechanical hooks (slice 1)
- **Warmth decay** kicks in at 20:00; pauses at 06:00.
- **Sleep decay** doubles at night unless the player is in a bedroll.
- **FOV radius** drops from ~20 day to ~3 night; lit-fire light source extends to ~8 inside 5 cells of the fire.
- **Auto-save** fires at every 06:00 dawn transition.

## Out of scope (slice 2+)
Weather, seasons, day-length variance, sunrise color, moon phases.

Reference: `[[Survival - Game time clock and needs]]`, `[[Survival - FOV]]`, `[[Survival - Fire Making]]`.
