A separate stamina pool, **lighter-touch** than CDDA's. Only running and heavy actions consume it; normal swings are free.

## Pool

Each actor has `stamina` (and `stamina_max`). Baseline `stamina_max` scales with Stam attribute.

Out-of-stamina state (stamina ≤ 0) applies penalties: slower swings (+move-cost on attacks), lower hit (penalty to Melee/Ranged rolls), cannot run, cannot use heavy actions.

## What costs stamina

- **Running** (move action at half move-cost but +stamina drain). Normal walking is free.
- **Dodge action** (when the player has a stamina-dodge ability post-techniques) — deferred with techniques.
- **Heavy actions** — defined as: grapple verbs (Grapple / Throw / Disarm), reach-2 polearm attack from full extension, brace (shield active block), crossbow reload, aimed-shot swings.
- **Power attacks** (post-techniques) — deferred.

**Normal swings are FREE.** This is the key delta from CDDA. We don't want "every fight is a stamina spreadsheet." Stamina is for emphasis, not for routine.

## What does NOT cost stamina (v1)

- Standard melee swings (any weapon).
- Standard ranged shots (bow draw + release; crossbow firing once loaded).
- Walking.
- Picking up items, switching weapons, looking around.

## Regeneration

Stamina regenerates over time, modulated by:

- **Resting** (standing still or walking) — fastest regen.
- **In combat** (any swing in the last few ticks) — slower regen.
- **Encumbrance penalty** — total encumbrance from armor reduces regen rate. See [[Survival - Combat - Armor model]].

Sleeping fully restores stamina (per [[Survival - Game time clock and needs]]).

## HUD

Stamina bar lives in the in-combat HUD next to the total HP bar (per the round-7 decision). Color: green → yellow → red as it depletes. When out (≤ 0), the bar pulses red to make the penalty state legible.

## Save schema

`stamina: i16` and `stamina_max: i16` are new fields on the Player save struct (and on humanoid/boss enemy saves). Additive: serialize with `#[serde(default)]` so old saves migrate cleanly. The combat schema bump (covering all combat additions) carries this.

## Open

- Concrete numbers: baseline `stamina_max`, regen rate per second, drain per heavy action — deferred to balance card.
- Whether being grappled (the target of a grapple) drains stamina each turn (struggling).
- Whether "running" is a toggle state or a per-action variant (probably a toggle: hold a modifier while moving = run).
- Stamina interaction with hunger/thirst from the survival side — does low hunger lower stamina max? (probably yes, light coupling, but defer to a polish card.)

Source: combat refactor brainstorm 2026-05-21.
