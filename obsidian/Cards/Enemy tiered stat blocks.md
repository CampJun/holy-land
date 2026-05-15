Two enemy schemas: simple grunts vs. full-stat-block bosses. Plus the "in striking range" alert UX.

## Grunt schema (most enemies)
- HP
- Damage range (min, max)
- AP per turn (e.g. 100, same as player)
- Move cost (e.g. 25 AP)
- Special-ability list (optional, e.g. "leap", "poison bite")
- Loot table reference

No attribute layer underneath — numbers tuned directly. Cheap to author, cheap to balance, AI is asymmetric with player rules but that's fine for grunts.

## Boss schema (bosses + elites)
- All 5 attributes (Stam, Str, Agi, Int, Spi)
- Secondaries derived from attributes (HP, Speed, etc.)
- AP pool, mana pool, cooldowns
- Equipment slots + weapon
- 1–4 powers (using the same power system as the player)

Bosses play by player rules. Reusable code for boss AI = reusable code for player decisions; encourages a strong rules engine.

## In striking range alert
- No movement-range visualization on the grid.
- When any enemy can reach the player in their next turn, the **alert fires**.
- Visual treatment TBD: candidates are glyph blink, reverse-video glyph, corner-of-screen icon, log message, or some combination. Decide during [[Combat HUD]] implementation.

## Open
- Whether grunts can be promoted to "elites" mid-encounter (rare event tier) — keeps boss-tier authoring extensible.
- Specific alert visual treatment.

Source: design plan dated 2026-05-14.
