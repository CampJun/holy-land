Revisit the flat 100 mana pool once caster builds matter.

## Current state (per [[Combat math foundation]] / [[Power system]])
- Mana pool is **flat 100**. No scaling stat.
- Spirit reduces mana cost per cast and increases regen rate.
- Intellect handles cooldown reduction + skill XP + crafting/lore.

## Why deferred
At the brainstorm stage, scaling mana introduced too many design questions before any caster build had been played. The flat pool keeps the math simple for the first power-system implementation and lets us tune in code with real data.

## When to revisit
- After the first 3–4 powers are implemented and playtested.
- If casters consistently run out of mana mid-fight (suggests pool should scale up).
- If Intellect feels weaker than Spirit despite the role distinction.

## Candidate models
- **Spirit-scaled pool:** Base 100 + Spirit × 10 (Spirit becomes the "endurance caster" stat).
- **Intellect-scaled pool:** Base 100 + Intellect × 10 (matches WoW vanilla; gives Int both burst and uptime).
- **Both:** Base 100 + (Spirit + Intellect) × 5.
- **Stay flat, deepen regen tuning instead:** more aggressive Spirit-driven regen.

Source: design plan dated 2026-05-14 (mana scaling explicitly deferred).
