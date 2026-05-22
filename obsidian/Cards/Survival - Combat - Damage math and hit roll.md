CDDA-style normal-distribution contested roll, bash/cut/stab damage types, per-body-part HP, binary crippling. Supersedes [[Combat math foundation]].

## The hit roll

When an actor performs `Attack` or `Shoot` against a target:

1. **Attacker score** = Melee-skill (or Ranged-skill) + weapon-proficiency bonus + weapon's `to_hit` modifier + Agi-attribute modifier + situation modifiers (aimed-shot penalty, polearm-no-reach penalty, prone penalty, etc.).
2. **Defender score** = Dodge-skill + Agi-attribute modifier − total-encumbrance penalty + situation modifiers (prone penalty, surprised, etc.).
3. Each side rolls **from a normal distribution** centered on their score (CDDA's actual model — most spread sits near the average, occasional fat-tail swings).
4. **Margin** = attacker_roll − defender_roll.
   - `margin < 0` → **miss**.
   - `margin ≥ 0` → **hit**.
   - `margin ≥ 15` → **critical hit**.

For ranged, the same math applies but the targeting cursor commits to the shot — see [[Survival - Combat - Reach and ranged]] for the visible to-hit % UX.

For melee, the roll is **hidden** from the player. Feedback is via the log message ("You stab the bandit in the chest!" / "Your axe glances off his mail." / "You swing and miss."). Maintains tension; matches the gritty CDDA tone.

## Critical hits

When `margin ≥ 15`:

- **Bonus damage:** crit multiplier (placeholder ~1.5×, tune later).
- **Armor bypass:** crit reduces the rolled armor coverage by a flat amount (e.g. each crit layer rolls coverage at -25), making mail less reliable against a clean blow.
- **Aimed attack synergy:** if the attacker declared a body part via aimed-shot, a crit forces that part to be hit (otherwise crit lets the attacker pick the part to hit, instead of weighted-random by coverage).

## Aimed shots

Both melee and ranged can declare a target body part before committing the action. Effects:

- **Extra move-cost** on the swing/shot.
- **Hit penalty** on the roll (you're sacrificing a clean swing for placement).
- On hit, damage applies to the chosen part instead of weighted-random.

Aimed-shot UI: while targeting (ranged) the cursor includes a body-part submenu; for melee, the attack verb opens a "where" prompt before commit. Cancel returns the action's move-cost.

## Damage types: bash / cut / stab

Every weapon swing produces a damage triplet — e.g. a falchion does `(2 bash, 8 cut, 1 stab)` per hit. Daggers stab; maces bash; axes cut + bash; spears stab.

Armor reduces each type independently:

- Mail blocks cut very well, stab medium, bash poorly.
- Padded blocks bash well, cut medium, stab poorly.
- Plate blocks all three at high values but heavy.

This is the weapon-vs-armor rock-paper-scissors. A maul-wielder vs. a mail-clad foe is fine; a dagger-wielder vs. plate is a bad time.

## Damage rolling

For each damage type:

1. Roll the weapon's `damage_die[type]` (concrete numbers in balance card).
2. Add Str-attribute bonus to bash + cut; Str half-bonus to stab.
3. Add weapon-proficiency damage bonus.
4. Apply crit multiplier if applicable.
5. Subtract the rolled armor DR for that type.
6. Floor at 0; sum per-type damage and apply to the rolled body part.

## Per-body-part HP

Each body part has its own HP pool (see [[Survival - Combat - Armor model]] for the six parts). Scale:

- **Torso:** ~80 (highest, with the most coverage weight)
- **Head:** ~40 (fragile; headshots are real)
- **Each arm:** ~60
- **Each leg:** ~60

Stam attribute scales all of them (each Stam point above baseline = small bonus per part).

## Crippling (binary at 0 HP)

Limb HP can't go below 0. When a limb hits 0:

- **Arm crippled** — drop wielded weapon; cannot 2H or wield in that hand until healed. If the OTHER arm is also crippled, you can do nothing weapon-related.
- **Leg crippled** — move-cost doubled (effective speed halved on that leg).
- **Head at 0 HP** → death event triggered.
- **Torso at 0 HP** → death event triggered.

**Damage overflow:** damage that would push a limb below 0 spills to the torso instead. So you don't survive forever by spreading hits across already-crippled limbs.

**Healing:** limb HP regenerates slowly with rest / treatment. Rules deferred to the medical card (TBD); for v1, slow regen during sleep + faster regen if torso wound is treated.

**No "destroyed" tier** in v1 — limbs can be crippled but not permanently lost. That tier may land in a later card.

## Death event

When head OR torso hits 0 HP, fire a `DEATH(entity)` event. **What happens next is deferred** to `Survival - Death and respawn` (item drops, respawn position, limb-injury persistence). Combat just emits the event.

## Hit feedback (UX)

- **Melee:** hidden roll. Log messages describe the outcome with body-part flavor.
- **Ranged:** visible to-hit % during cursor target. Log messages also fire post-shot.

## Open

- Crit multiplier exact value — placeholder 1.5×; tune.
- Aimed-shot hit penalty exact value — placeholder -25; tune.
- Whether damage overflow on a crippled limb counts toward the crippled limb's stat or only adds to torso (probably only torso).
- Whether the death event triggers immediately at 0 HP, or after the current animation tick completes (cosmetic).

Source: combat refactor brainstorm 2026-05-21. CDDA reference: `src/melee.cpp` formulas.
