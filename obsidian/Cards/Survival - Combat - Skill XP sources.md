Per-skill XP rules and CDDA-style attribute training rates. Combat-only — crafting/survival skills retain their own [[Skill XP sources]] entries. Supersedes the combat sections of that card.

## Combat skill XP

| Skill | XP awarded for | Notes |
|---|---|---|
| **Melee** | Every melee swing, hit or miss | More on hit; small floor on miss. Trains broadly across all melee weapons. |
| **Ranged** | Every ranged shot, hit or miss | Same pattern. |
| **Dodge** | Successful dodge of an incoming attack | Also small XP on near-misses (margin between hit and miss < threshold). |
| **Block** | Successful shield block of an incoming attack | Trains while a shield is in the off-hand and a hit is mitigated. |
| **Light Armor** | Taking a hit while wearing padded armor | XP per damage point absorbed by padded layer(s). |
| **Medium Armor** | Taking a hit while wearing mail | Same — per damage absorbed. |
| **Heavy Armor** | Taking a hit while wearing plate | Same. |

## Per-weapon proficiency XP

| Proficiency | XP source |
|---|---|
| Knife / Dagger | Swing landed with a knife/dagger |
| Sword | Swing landed with an arming sword |
| Falchion | Swing landed with a falchion |
| Axe | Swing landed with an axe (the woodcutting axe item counts; ChopTree does NOT train Axe combat) |
| Mace / Cudgel | Swing landed with a mace or cudgel |
| Quarterstaff | Swing landed with a staff |
| Spear / Lance | Swing landed with a spear or lance (reach-2 hits also count) |
| Gisarme / Bill | Swing landed with a gisarme or bill |
| Bow | Shot landed with a bow |
| Crossbow | Shot landed with a crossbow |
| Unarmed | Successful strike OR successful grapple verb (Grapple/Throw/Disarm) |

**Hit-only training:** proficiencies train on hits, not misses. (Distinct from Melee/Ranged top-level skills, which train on both.) This rewards specialization in something you can actually land.

## XP curve

- **Cap 99** with an exponential per-level cost (RS-shape). Each level is meaningfully harder than the last.
- A 99-cap skill at level 1 → level 2 might be ~50 XP; level 98 → 99 might be millions.
- Concrete curve values are deferred to `Survival - Combat - Balance numbers`.

**Daily XP cap** matches the existing survival skill chassis ([[Survival - Skill system URW]]): 20 XP per day per skill, reset at 06:00 dawn. This bounds grinding intensity and gives the wilderness loop a natural rhythm.

## Attribute training (CDDA-style use-based)

Attributes do NOT increase via skill milestones (that mechanic was dropped in this refactor). Instead, each attribute trains slowly through distinct activities. **Hundreds of relevant actions per +1.** Permanent gains.

| Attribute | Combat-relevant training |
|---|---|
| **Strength** | Dealing heavy melee damage (per damage point above a threshold). Carrying heavy loads. |
| **Agility** | Successful dodges. Movement actions while at low encumbrance. Successful ranged shots at long range. |
| **Stamina** | Taking damage (per HP lost). Running. Sustained exertion (combat duration). |
| **Intellect** | Combat utility deferred to magic refactor card. (For combat, currently no Int training.) |
| **Spirit** | Deferred to magic refactor card. |

Each attribute has a hidden `train_progress` counter; when the counter exceeds a threshold (which scales upward as the attribute rises), the attribute increments by 1 and the counter resets.

Attribute training is **independent of skill XP**. A character with Melee 50 but Strength 8 is plausible (lots of practice, never really pushed their max).

## Save schema

Adds per-skill `xp: u32` and `daily_xp: u8` (matching existing skill chassis). Adds per-attribute `train_progress: u32`. All `#[serde(default)]`-compatible additions. The combat schema bump covers these.

## Open

- Exact XP-per-action magnitudes per skill — balance pass.
- Whether Block trains while wielding a shield from PASSIVE blocks (auto-rolls per hit) or only from an actively-Braced block. Probably both, with the active brace giving more.
- Whether "taking damage" trains armor skills proportionally to which layer caught it, or proportionally to the wearer's encumbrance. Probably the former (the layer that caught the hit trains).
- Spirit/Int combat training arrives with the magic refactor; placeholder zero training in v1.
- Whether the Daily XP cap should be different for combat skills vs. survival skills (combat may want a higher cap to avoid feeling grindy; or the same to keep things consistent).

Source: combat refactor brainstorm 2026-05-21. Supersedes combat sections of [[Skill XP sources]].
