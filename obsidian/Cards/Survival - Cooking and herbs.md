Pan-on-fire cooking + three herb use cases. Validates the pan in slice 1.

## Pan placement
- Action: "Place pan on fire" (requires pan in inventory + adjacent lit fire).
- Cost: 5 game-seconds.
- Pan becomes a `Cookware` entity sitting on the fire's cell.
- "Pick up pan" reverses (also 5 sec; fails if pan currently cooking something).

## Cook raw → cooked
- Action: "Cook X" where X is a raw food item (e.g. raw fish from `[[Survival - Fishing]]`).
- Cost: 120 game-seconds, passive (multi-turn; toggleable progress bar / time-skip).
- Output: cooked variant; +2× Hunger restore vs. raw.
- Player may walk away from the fire — cooking continues. If left too long, item burns to inedible.

## Brew tea
- Action: "Brew tea" (requires pan-on-fire + waterskin with water + 1 herb).
- Cost: 180 game-seconds passive.
- Output: replaces the waterskin's water content with "tea" (4 uses). Each tea drink: +20 Thirst AND +5 Warmth.

## Eat herb raw
- Action: "Eat herb" (no fire needed; from inventory).
- Cost: 5 game-seconds.
- Output: +5 Hunger.

## Seasoning
- Action: "Add herb to cooking" (requires pan-on-fire with food in it + 1 herb).
- Cost: 5 game-seconds.
- Output: cooked food gets a `seasoned: true` flag. Eating seasoned: +1.5× cooked-food Hunger restore (so 3× raw).
- Stacks once; second herb does nothing.

## Out of scope (slice 2+)
- Recipes (e.g. fish stew = fish + water + herbs).
- Spoilage / preservation.
- Cookery skill (slice 1 cooking is skill-less).
- Burnt food penalty in detail.

Reference: `[[Survival - Fishing]]`, `[[Survival - Fire Making]]`, `[[Survival - ItemInstance and weights]]`.
