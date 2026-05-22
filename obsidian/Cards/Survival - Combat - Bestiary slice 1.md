The first enemy that ships with combat. A single humanoid: the Cornish bandit (Yeoman tier). Designed to exercise as many combat systems as possible in one entity, so the v1 implementation has a real integration target.

## The enemy

**Cornish bandit** — a desperate broken man preying on travelers along the Tamar valley roads. Statute-of-Winchester Yeoman tier.

### Schema (humanoid tier)

Per the 3-tier enemy schema:

- **Body parts** — six (head, torso, L-arm, R-arm, L-leg, R-leg) with their own HP and crippling rules from [[Survival - Combat - Damage math and hit roll]].
- **Flat stat block** for the bandit (NOT full player rules — that's the Boss tier). Stats are hand-tuned numerical fields, not derived from attributes.
- **Loadout** rolled from the Yeoman tier table in [[Survival - Combat - Status armament tiers]].

### Stat block (placeholder values)

| Stat | Value | Notes |
|---|---|---|
| `speed` | 100 | Baseline human pace. |
| `melee_hit` | medium | For the contested hit roll. |
| `dodge` | low-medium | They're not trained. |
| `block_chance` | small (only if shield) | If their loadout rolls a shield. |
| `move_cost` | 100 | Standard. |
| `limb_hp` | torso 80 / head 40 / arms 60 / legs 60 | Per the standard scale. |
| `damage_profile` | varies | Determined by rolled weapon. |

Concrete numbers deferred to balance card.

### Loadout roll (Yeoman tier)

When a Cornish bandit spawns, roll:

- **main_hand:** 50% spear, 30% short sword, 20% falchion.
- **off_hand:** 60% knife, 30% empty, 10% small round shield.
- **head:** 70% iron skullcap, 30% nothing.
- **torso:** 80% padded doublet, 20% leather jerkin.
- **legs:** nothing (poor men rarely had leg armor).
- **ranged:** 20% has a bow + 12 arrows in pack, otherwise none.

(Probabilities are starting values; tunable.)

This guarantees variety: encounter five bandits and one might be a bowman, one a falchion-cleaver with a buckler, three with spears.

### AI

Chase + bump + use ranged if armed, per the v1 AI decision:

1. If player visible in FOV, A*-pathfind toward player.
2. If wielding a ranged weapon AND has ammo AND player at range AND LoS: shoot (using the targeting math from [[Survival - Combat - Reach and ranged]]).
3. Else if adjacent (or reach-2 for polearm): swing.
4. Else: keep advancing.

No flee, no group tactics, no aimed shots, no grapple. The bestiary card for richer behaviors is a follow-up.

### Spawning

For v1 integration, place a single Cornish bandit somewhere reachable in the player's chunk (e.g. spawn near a road tile, deferred until road generation lands; for first integration, place adjacent to the oasis exit as a deliberate first encounter).

Eventually bandits roam wilderness chunks, more likely at night, more likely near roads, more likely in groups of 2–3 (with the v1 "chase + bump" AI scaling to multiple bandits naturally).

### Death and drops

When the bandit's head or torso HP hits 0, fire the `DEATH(entity)` event. Per the deferred `Survival - Death and respawn` card, default behavior is:

- Bandit's loadout (rolled gear) drops as ground items at death position.
- Bandit's body becomes a corpse item (TBD — corpse looting/decay is its own concern; for v1 we can skip).
- The player learns combat by killing this one entity and looting their gear (immediate progression: a Rabble player picks up Yeoman gear).

### Why this enemy

- **Tests every system** that's locked: melee, reach (if rolled with spear), ranged (if rolled with bow), armor layering (padded + skullcap), per-body-part HP, status arming, AI, drops.
- **Period-correct.** Bandits / "broken men" were a real concern in 13–14c England, especially in rural Cornwall and Devon along administered borders.
- **Lore-aligned.** Pairs with the wilderness/pilgrim setting work in `obsidian/Cornwall-*` files.
- **Modular.** Once the bandit works, additional Yeoman enemies (town watch, militia) and tier-shifted enemies (a Sergeant captain bandit, a demon at the Boss tier) plug into the same chassis.

## Future bestiary slices

Not in v1, but the framework supports:

- **Wolf** (amorphous-tier or quadruped variant) — single-HP or simplified body-parts; pack AI.
- **Demon (lesser)** (amorphous-tier supernatural) — single HP pool, immune to some damage types, demonic loot table.
- **Sergeant-tier human enemy** — full mail + sword + shield, real threat.
- **Boss tier** — first Arthurian-quest boss. Full player rules, mythic-loot drop.

## Open

- Spawn rules (frequency, locations, time-of-day weighting).
- Whether bandits have NAMES / dialog (probably no in v1; flavor follow-up).
- Whether bandits flee when low HP (currently no — flee behavior deferred).
- Corpse looting workflow (probably reuse the existing per-cell items system).
- Whether multiple bandits coordinate (no — independent AI in v1).

Source: combat refactor brainstorm 2026-05-21.
