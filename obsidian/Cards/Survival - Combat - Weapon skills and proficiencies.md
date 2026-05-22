Two-axis skill system mirroring CDDA: one umbrella combat skill + per-weapon proficiencies that train independently. Plus the dedicated defensive skills (Dodge, Block, 3 Armor classes). Supersedes the combat sections of [[Skill system]].

## Top-level combat skills

| Skill | Trains on | Gates |
|---|---|---|
| **Melee** | Any melee swing (hit or miss) | Hit-chance baseline + base damage scaling for ALL melee weapons |
| **Ranged** | Any ranged shot (hit or miss) | Hit-chance baseline + base damage scaling for ALL ranged weapons |
| **Dodge** | Successful dodges + near-misses | Dodge roll value in the contested attacker-vs-defender contest |
| **Block** | Successful shield blocks while wielding a shield | Block-chance per incoming hit; treated as a weapon proficiency under Melee |
| **Light Armor (Padded)** | Wearing padded armor while taking hits | Reduces padded armor's encumbrance penalties |
| **Medium Armor (Mail)** | Wearing mail while taking hits | Reduces mail's encumbrance penalties |
| **Heavy Armor (Plate)** | Wearing plate while taking hits | Reduces plate's (coat-of-plates, emerging articulated) encumbrance penalties |

All combat skills cap at **99** with an exponential XP curve (RS-shape). XP curves per skill: see [[Survival - Combat - Skill XP sources]].

## Weapon proficiencies (the second axis)

One proficiency per weapon type. Each trains independently while wielding that weapon, REDUCES the swing's move-cost and stamina cost, and (post-v1) unlocks techniques. Proficiencies sit under Melee or Ranged.

Period-accurate roster (Cornwall/Devon ~1300, per the research doc):

### Melee proficiencies

- **Knife / Dagger** — fast, stab-heavy, 1H. Universal (statute mandates a knife for every man).
- **Sword (arming)** — balanced 1H, cut + stab mix. Status-graded — cheap to fine. Some commoners owned them.
- **Falchion** — single-edged cleaver, 1H. Cut-heavy, attested in early-1300s coroner rolls.
- **Axe** — gray-zone tool/weapon. 1H or 2H variants. Cut-heavy with bash. Overlaps with the woodcutting axe item (`ChopTree` verb).
- **Mace / Cudgel** — commoner blunt, 1H. Pure bash. (War-hammer-proper is 15c+, dropped.)
- **Quarterstaff** — 2H wooden pole, commoner weapon. Pure bash, decent reach. NOT a magic staff.
- **Spear / Lance** — 2H polearm with reach-2. Stab-heavy. Universally available per the statute. See [[Survival - Combat - Reach and ranged]] for reach mechanics.
- **Gisarme / Bill** — 2H polearm with reach-2, sickle-bladed. Cut + stab mix. The period polearm proper (replaces halberd/bardiche).
- **Unarmed (Wrestling)** — see "Unarmed and wrestling" section below.

### Ranged proficiencies

- **Bow** — drawn-string ranged. Period-correct (avoid the "longbow myth" — research warns against). Treated as a single Bow proficiency in v1; sub-bow distinctions deferred.
- **Crossbow** — mechanical-trigger ranged. Slower reload, higher damage, less training-intensive (matches the Royal Armouries' "militia weapon" framing). Especially associated with castle defense.

### Deferred to follow-up cards

- **Throwing** — thrown knives, hand axes, stones. Not in v1; see future `Survival - Combat - Throwing and dual-wielding`.

## Unarmed and wrestling

Reflects the historically-attested Devon wrestling culture (Exeter Cathedral roof bosses, 1286–1302).

**Unarmed proficiency** trains when fighting bare-handed. Grants both strikes AND grapple verbs:

- **Strike** — fist/kick/headbutt swing. Low bash damage.
- **Grapple(target)** — initiates a hold. While grappled, target can't move or attack. Costs heavy moves. Both parties roll Str-contest each turn to maintain/break.
- **Throw(target)** — knocks target prone (severe combat penalty; standing back up costs moves). Requires successful grapple or an opening (target staggered, crippled-leg, etc.). Costs heavy moves.
- **Disarm(target)** — on a successful Unarmed strike, force the target to drop their wielded weapon. Higher hit threshold than a normal strike. Costs heavy moves.

Grapple states are tracked as a `Grappling { partner, since_tick }` component on both actors.

## Weapon stat profiles (relative, not absolute)

Concrete numbers are deferred to `Survival - Combat - Balance numbers`. The card-level commitment is to relative position:

- **Dagger** = fast, stab-heavy, low damage.
- **Sword** = medium speed, balanced cut + stab.
- **Falchion** = medium speed, cut-heavy, slight bash.
- **Axe** = medium-slow, cut + bash.
- **Mace** = medium, pure bash.
- **Quarterstaff** = medium, bash, modest reach (still adjacent though).
- **Spear** = medium, stab, reach-2.
- **Gisarme** = slow, cut + stab, reach-2.
- **Bow** = slow draw, ranged, stab.
- **Crossbow** = very slow reload, ranged, stab + bash, high damage per shot.

## Improvised weapons

Specific tool items get an implicit (poor) weapon profile when wielded: woodcutting axe (already a real axe), kitchen knife, cooking pan (bash), stone (bash), woodcutting tools. Other items (waterskin, tent, herb) refuse to swing. Period-honest. No proficiency XP from improvised swings.

## Open

- Whether Unarmed strikes train both Melee AND Unarmed proficiency, or only Unarmed.
- Whether grappling requires LoS / adjacency / specific body-part availability (e.g. can you grapple if both your arms are crippled? — probably not).
- The Block proficiency's exact roll: % chance to fully cancel? % damage reduction? Roll vs. attacker's hit margin?

Source: combat refactor brainstorm 2026-05-21. Period research: `~/Downloads/deep-research-report(1).md`.
