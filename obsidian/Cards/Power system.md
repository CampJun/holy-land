Active deity-granted abilities — 4 slots, modifier+dpad activation flow, 3 cost tiers, mana + cooldowns.

## Slots
- **4 power slots** on the player.
- Each slot holds a `PowerId` (one of 16 god powers — see [[16-god roster design]]).
- Player selects slot loadout at run start from unlocked powers.

## Activation flow (Miyoo-friendly, turn-based-paced)
1. Hold a modifier button (likely L or R on Miyoo).
2. Press one of 4 dpad directions → selects slot.
3. Release modifier → power auto-targets the **nearest enemy**.
4. Dpad cycles between visible candidate targets.
5. `Select` toggles **free grid-aim mode** (dpad moves a reticle on the grid).
6. Confirm → cast.

Turn-based pacing means this multi-step flow doesn't feel slow.

## Cost tiers
| Tier | AP | Mana | Cooldown (turns) |
|---|---|---|---|
| Light | 25 | 15 | 2 |
| Medium | 50 | 35 | 4 |
| Heavy | 75 | 60 | 6–8 |

Each god's power slots into one of these tiers.

## Scaling
- **Spirit** reduces mana cost and increases mana regen.
- **Intellect** reduces cooldowns.
- **1H + empty off-hand** loadout halves AP cost.
- **Magic skill** further reduces AP cost (skill-driven efficiency).

## Mana
- Flat 100 pool for now (see [[Mana pool scaling]] for the deferred scaling rework).

## Open
- Cooldown reduction magnitude per Intellect point.
- Whether powers themselves scale with caster stats (Intellect / Magic skill multiply effect magnitude?) — or strictly via blessings — TBD with first powers' authoring.

Source: design plan dated 2026-05-14.
