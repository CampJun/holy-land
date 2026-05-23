Game-time clock and four-meter needs system. Death enabled.

## Clock
- `World.clock_seconds: u64` — game-time since game start.
- Each action consumes its declared cost; clock advances by that cost.
- AFK pauses everything; real-time wall-clock is irrelevant.

## Four needs (all 0–100, all fatal at 0)
| Need | Start | Decay (game-time) | <25 | <10 | Recovery |
|---|---|---|---|---|---|
| Thirst | 75 | 1/min always | +20% action cost | +50% + tint | drink stream/waterskin/tea |
| Hunger | 75 | 1/3 min | +20% | +50% | eat ration/herb/cooked |
| Sleep | 75 | 1/5 min day; 1/3 min night (unless bedroll) | +20% | +50% | sleep in bedroll: full over 8 hr |
| Warmth | 100 | 0/min day; **3/min night** unless fire/tent/bedroll | +20% + tint | +50% | fire +1/min, tent +0.5/min, bedroll +1/min, stack |

## Penalty application
Action cost = `base_cost × (1.0 + sum_of_active_penalties)`. Capped at +100% total. Penalties don't compound multiplicatively to keep the math readable.

## Death
- Any need hitting 0 → death. Death message + restart from last save (auto-save fires at dawn + on quit + on manual Select).
- Slice-1 tutorial implication: must light a fire before 20:00 dusk or die ~33 game-minutes into the night.

## Module
`src/needs.rs` — `Needs { thirst, hunger, sleep, warmth }`, decay tick called from action executor, penalty resolver, death check.

Reference: `[[Survival - Day night cycle]]`, `[[Survival - Fire Making]]`.
