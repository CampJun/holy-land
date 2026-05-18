Implement the 100-AP-per-turn action economy and per-weapon AP swing costs.

## Core rules
- **100 AP per turn.** Unused AP does not carry over.
- **Base move cost:** 25 AP (4 squares/turn baseline).
- **Speed secondary stat reduces move AP cost** — scales with Agility, items, buffs, food, potions.

## Weapon AP costs (variable per weapon, averages)
| Class | Swing AP | Might ratio |
|---|---|---|
| Dagger | ~25 | ~0.5 |
| Sword (1H) | ~50 | ~1.0 |
| Mace / club (1H) | ~55 | ~1.1 |
| 2H avg (greatsword–maul) | ~80 (70–100) | ~1.8 (1.5–2.0) |

Net DPS roughly comparable across weapons; texture (many small vs. one big) differs and interacts with flat armor.

## Adjacency penalty
- **Normal adjacent step:** 25 AP, triggers an opportunity attack from each adjacent enemy.
- **Careful step:** 50 AP, no opportunity attack.
- _(Spec ambiguity flagged in design plan — confirm interpretation before implementing.)_

## Loadout AP modifiers
- **1H + empty off-hand:** halves power AP cost AND item AP cost (utility / spellsword loadout).
- **Dual wield:** off-hand swing pairs with main hand action (exact AP economics TBD); -20% hit chance on both swings.
- **2H:** per-weapon AP and Might.
- **Sword & board:** active "brace" action costs AP; passive block chance on incoming hits.

## Item AP
- Base 50 AP per item use (potion, food, scroll, equip swap).
- 1H + empty off-hand → 25 AP.

## Open
- Dual-wield exact AP/swing math.
- 1H utility loadout potentially OP (half AP on both items AND powers) — may need a single-bonus tradeoff.

Source: design plan dated 2026-05-14.
