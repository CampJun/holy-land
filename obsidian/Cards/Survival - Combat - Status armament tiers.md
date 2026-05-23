Humanoid enemy loadout is rolled from a Statute-of-Winchester tier table, not authored piece-by-piece. Encodes the wealth-graded armament reality of Cornwall/Devon ~1300.

## Rationale

The 1285 Statute of Winchester divided every man between fifteen and sixty into armament classes by goods/wealth: rich men carry hauberk + iron helm + lance + knife + horse; middling men carry lance + bow + knife + iron helm + doublet; poor men carry falces + gisarmes + knives + "small arms"; men with less than twenty marks in goods carry swords + knives + small arms.

That historical gradient is exactly the schema we want for arming humanoid enemies. Saves piece-by-piece authoring, gives instant period-consistent variety, and grounds the world in real social structure.

## The four tiers

| Tier | Wealth | Likely encounter | Loadout pool (rolled) |
|---|---|---|---|
| **Rabble** | Below twenty marks | Bandits, vagrants, broken men, peasants in revolt | Knife (always); plus one of: cudgel, falces, gisarme, quarterstaff, stone, improvised weapon. No real armor — maybe a leather jerkin or padded scrap. No shield typically. |
| **Yeoman** | Middling | Town watch, freeholder militia, common bandit | Knife + spear OR bow; padded doublet; iron skullcap. Sometimes a falchion or short sword in place of bow. Shield uncommon. |
| **Sergeant** | Household soldier | Lord's man-at-arms, castle garrison, sergeant-at-arms | Sword (arming) + knife; partial mail (hauberk on torso, often without chausses); kettle hat or open helm; round shield. Sometimes a crossbow. |
| **Knight** | Aristocratic | Mounted noble, retinue captain, errant knight | Lance OR sword + knife; full mail (hauberk + chausses + coif); great helm; large shield; horse (often dismounted in this v1). Possible coat-of-plates over mail (bleeding-edge). |

## How an enemy rolls its loadout

When a humanoid enemy spawns:

1. Its tier is set by the spawn rule (`Cornish_bandit → Yeoman`, `castle_guard → Sergeant`, etc. — defined in the bestiary).
2. The tier's loadout pool is rolled per slot (main_hand, off_hand, head, torso, legs, ammo).
3. Optional flavor tweaks (e.g. 30% chance of a falchion instead of a sword for Yeoman). Probabilities live in the tier table.
4. The resulting loadout is fixed for that encounter; the enemy's stat block records what they're wielding.

## Drops

When an enemy dies, their loadout drops as ground items. This naturally makes Sergeant+ enemies juicy loot piñatas and creates a clear "punch above your tier" progression. Mythic loot is gated separately ([[Survival - Combat - Mythic and faerie arms]]).

## Hostile vs. neutral framing

The tier is a **loadout schema**, not a behavior tag. A friendly Sergeant escort and a hostile bandit Sergeant share the same equipment table; what differs is their disposition (`Hostile` component, faction, dialog).

## Player-vs-tier scaling

The player starts effectively as a Rabble-tier individual (no real armor, basic weapon). Progression through the game is partly about climbing the gradient — earning or looting Yeoman gear, then Sergeant, then (rarely) Knight. Mythic loot exists alongside but doesn't replace the gradient.

## Period accuracy

- **No firearms** in any tier — gunpowder weapons aren't in normal use in England until 1327 (Walter de Milemete). v1 stays pre-firearm.
- **Halberd / bardiche / war hammer** are anachronistic for 1300 and absent from all tiers. Gisarme / bill / spear / falchion are the period polearms and cleavers.
- **Plate armor** in 1300 is the coat-of-plates (small rectangular riveted plates over fabric). Full white plate is much later. Even Knight tier rarely sees full articulated plate.

## Open

- Whether each tier should also carry a "wealth" drop (coin / goods) that feeds the survival/economy loop.
- Whether NPC friendlies (priests, merchants) get unarmed tier(s) — probably yes, but not the focus of this combat-tier card.
- Whether a Knight tier always has a horse, or "dismounted Knight" is a v1 simplification (likely the latter — horses are out of scope).
- A possible fifth tier: **Faerie / Demonic** for non-mortal arming patterns (e.g. demon-forged spear + bone armor) — could fold into mythic loot card or be a v1 tier extension.

Source: combat refactor brainstorm 2026-05-21. Period research: `~/Downloads/deep-research-report(1).md`.
