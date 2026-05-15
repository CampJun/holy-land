Implement the WoW-inspired combat math layer: 5 attributes, secondary stats, damage formula, hit / crit / armor.

## Attributes
- **Stamina** → HP pool (Base 100 + Stam × 10).
- **Strength** → Might (the damage scaling stat — renamed from "AP" to avoid colliding with Action Points).
- **Agility** → Speed (move cost reduction), Hit rating, Dodge, melee Crit chance.
- **Intellect** → Skill XP rate, Power cooldown reduction, Crafting / lore / identify gates, caster Crit chance.
- **Spirit** → Mana cost reduction, Mana regen speed.

## Secondaries
- HP = `100 + Stamina × 10`.
- Mana = flat 100 (scaling deferred — see [[Mana pool scaling]]).
- Might = derived from Strength + weapon-specific contributions.

## Damage formula
```
damage = weapon_base + (Might × weapon_might_ratio) - target_armor
```

## Hit & crit
- Hit: contested roll, attacker hit-rating vs. defender dodge. Both rooted in Agility.
- Crit chance: scales from Agility AND Intellect (melee vs. caster sources).
- Crit damage: 2× default.

## Armor
- Flat subtraction per hit. Flagged: daggers vulnerable to armor; armor-penetration mechanic likely needed for fast-weapon viability vs heavy armor.

## Open
- % per Agility for hit/dodge/crit — TBD during balance pass.
- % per Intellect for caster crit + cooldown reduction — TBD.
- Armor penetration system or scaling solution for daggers vs. heavy armor.

Source: design plan dated 2026-05-14 (combat & skills brainstorm).
