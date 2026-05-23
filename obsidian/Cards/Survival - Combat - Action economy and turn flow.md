CDDA-style speed + move-cost economy. The unit every other combat number is denominated in. Supersedes [[Action point system]].

## Core rule

Each actor has a `speed` value (humans baseline **100**). Each engine tick, every actor accrues `speed` moves. When an actor has `moves ≥ 0` it may act; the chosen action subtracts its move-cost from the accumulator. No "100 AP per turn budget" — this is continuous tempo (the classic roguelike tick, what CDDA, DCSS, Angband do).

## What drives an action's move-cost

- **Base cost** intrinsic to the action (one-tile step ≈ 100; weapon swings vary).
- **Weapon move-cost** (relative: dagger fast, sword medium, two-handers slow). Defined per weapon in [[Survival - Combat - Weapon skills and proficiencies]].
- **Proficiency reduces cost.** Higher per-weapon proficiency lowers swing cost; trained daggers swing faster than untrained.
- **Armor encumbrance raises move cost** on the legs slot for movement; on torso/arms for swinging. Armor skills mitigate.
- **Aimed attack penalty.** Declaring a target body part costs more moves (and lowers hit chance).
- **Reach penalty.** Polearm hitting an adjacent target (without reach setup) pays a cost penalty per [[Survival - Combat - Reach and ranged]].

## Speed modifiers

- Status effects can scale `speed` (haste +25, slow -25, etc.) — first-class in this system, no special-case math needed.
- Crippled leg cuts the leg's move-cost contribution in half (effectively doubles the cost) per [[Survival - Combat - Damage math and hit roll]].
- Out-of-stamina state lowers swing speed (raises swing move-cost), per [[Survival - Combat - Stamina]].

## Turn-flow loop (engine-side)

1. Engine tick: add `speed` to each living actor's move accumulator.
2. Sort actors by `moves` descending; for each with `moves ≥ 0`, run AI (or accept player input).
3. Each action: deduct move-cost, apply effects.
4. Repeat from 1.

Player and enemies share one queue — there's no "player turn / enemy turn" boundary. A fast actor (speed 150) will reliably act between two slow actors' turns.

## Verbs that touch the action economy

Combat introduces these new verbs (wired via the existing `evaluate / execute / complete_step` pipeline in `src/action.rs`):

- `Attack(target_entity, aimed_body_part?)` — melee swing at adjacent (or reach-2 for polearm) target.
- `Shoot(target_entity, aimed_body_part?)` — ranged shot, requires LoS and ammo.
- `Brace` — shield-bearer's active block-chance boost (next incoming hit).
- `Grapple(target_entity)` / `Throw(target_entity)` / `Disarm(target_entity)` — unarmed verbs, see weapon-skills card.
- `Reload` — restock the wielded ranged weapon from pack ammo.

## Open

- Concrete move-cost numbers per weapon: deferred to `Survival - Combat - Balance numbers`.
- Tile-step base cost: probably 100, but worth tuning against feel.
- Whether the engine ticks at the existing multi-turn `MULTI_TURN_GAME_SEC_PER_FRAME` rate or runs as fast as the player can input.

Source: combat refactor brainstorm 2026-05-21.
