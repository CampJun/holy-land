First skill in the game. URW-style percentile check. Gates first-night survival.

## Skill identity
- Name: **Fire Making**.
- Range: 0–100.
- Starting value (slice 1): 15.

## Roll
`1d100 ≤ skill + tool_bonus + condition_modifier`

| Modifier | Value (slice 1) |
|---|---|
| Flint + steel | +30 |
| Bow drill (later) | +0 |
| Wet tinder | -25 (slice 2+ when weather lands) |
| Indoors / sheltered | +5 |

Starting effective chance with flint+steel: 45%.

## Required materials (within adjacent 8 cells OR inventory)
- 1+ tinder: twig OR grass blade
- 3+ kindling: sticks
- 2+ fuel: firewood
- 1 fire-starter tool: flint+steel

If any requirement missing, the "Start fire" action is greyed in the command menu with the specific missing item.

## Attempt
- Cost: 60 game-seconds.
- On failure: consume 1 twig as expended tinder. +1 XP.
- On success: spawn a `Fire` entity in the player's adjacent cell. Consume 1 tinder + 2 kindling + 1 fuel. +5 XP.

## XP cap
+20 XP per skill per in-game day. Resets at 06:00 dawn.

## Fire entity
- Lights its cell + emits a 5-cell light radius (extends night FOV to 8 within that radius).
- Burns down over time scaled by remaining fuel — initial fuel = ~1 game-hour per firewood piece consumed at light.
- Player can add fuel mid-burn ("Add firewood to fire" action).
- Extinguishes if fuel depleted OR rained on (slice 2+).
- Warmth source (see `[[Survival - Game time clock and needs]]`).

Reference: `[[Survival - Skill system URW]]`, `[[Survival - Game time clock and needs]]`.
