Arthurian / faerie / demon-forged loot that intentionally breaks period rules. Mundane combat stays grounded in Cornwall/Devon ~1300; this card defines the exceptions.

## Design intent

The setting is the Cornwall of Arthurian legend. The MUNDANE world is period-strict — see [[Survival - Combat - Status armament tiers]]. But the SUPERNATURAL world is wild: Excalibur, faerie-wrought blades, demon-forged plate, relics from forgotten saints. These rare items intentionally bend the combat math to feel mythic.

Mythic loot is **rare, quest-gated, and singular** — typically one per object (no "Excalibur +2" enchant-tier inflation).

## Categories

### Saintly relics

Drawn from the wider Arthurian / Cornish religious tradition.

- **Blade of a saint** — a sword found in a tomb. Cuts demonic flesh as if it were mortal (full damage vs. amorphous-tier demons). Burns the unworthy wielder (small HP drain per swing if your Devotion attribute is too low).
- **Pilgrim's stave** — a quarterstaff worn smooth on the road to Compostela. Grants minor stamina regen while wielded; never breaks; can banish a single weak spirit per day.

### Faerie smithing

Crafted by the Tylwyth / the Cornish piskies / the Otherworld.

- **Hawthorn-bound dagger** — a knife with a hawthorn handle. Strikes against iron-wearing foes have a chance to bypass mail entirely (one armor layer ignored). Cold to the touch; reacts badly to consecrated ground.
- **Tinner's blade** — a falchion forged by the duchy's Cornish tinners under a hill-king's blessing. Damage scales with Stam instead of Str. Sells for a fortune but no priest will bless it.

### Demon-forged

Made in the Pit. Wielding them comes at a cost.

- **Sulfur axe** — an axe that burns its target. Deals fire damage (new damage type — TBD whether to add a 4th type or just flavor the existing triplet) over time per hit. Slowly corrupts the wielder's Spirit attribute (training rate is negative while wielded).
- **Boneplate cuirass** — a coat-of-plates made of demon-bone, no encumbrance, very high DR. Wearing it draws hostile attention from certain factions (saintly NPCs flee; demonic NPCs are slightly less aggressive).

### Singular legendary

The capital-A artifacts.

- **Caliburn / Excalibur (TBD)** — a sword that cannot break, that strikes with crit on every swing against a single named enemy (per quest condition), and that may or may not be available in v1. Its narrative weight is the whole point.
- **The Spear of Longinus (placeholder)** — a polearm of biblical provenance. Massive damage; consumed on a single use against the right target.

## Mechanical hooks

Mythic items live in the same item / equipment slot system as mundane gear. They differ by:

- **Custom modifiers** on the standard combat math (extra damage type, armor bypass, regen, attribute scaling, etc.).
- **Conditions / costs** that mundane items don't have (drain Spirit, harm unworthy wielder, react to consecrated ground, faction reputation effects).
- **Singular flag** — not stackable, often not duplicatable. The world may contain exactly one.

## Acquisition

- **Quest reward** — completing a pilgrimage / saving a hermit / freeing a faerie captive.
- **Boss drop** — capital-B boss (the only enemy tier with full player rules — see [[Survival - Combat - Bestiary slice 1]] and the deferred 3-tier enemy schema).
- **Altar / shrine** — visiting a specific holy site at a specific phase of the day or moon.
- **Crafted only by specific NPCs** — e.g. a faerie smith who'll re-forge a broken blade for the right price.

## Save schema

Mythic items extend the existing ItemMetadata enum with a `Mythic { name, modifiers: Vec<MythicModifier>, condition_flags: u32, ... }` variant. Each modifier is a small effect tag (e.g. `ArmorBypass(1)`, `ScalesWithStam`, `DrainsSpirit`, `IgnitesTarget`). Modifiers are additive and round-trip through CBOR.

## Not in v1

This card is a **scaffold** — the framework for mythic loot exists, but no specific mythic item must ship in v1. v1 combat is mundane. The first mythic item can land alongside the first Arthurian quest.

## Open

- Whether mythic items respect armor-skill mitigations the same way mundane items do (probably yes for the wearer's skill, no for the armor's own properties).
- Whether mythic ranged weapons exist in v1 thinking (e.g. a faerie bow) or only melee for the first batch.
- Whether deities directly bestow mythic items (overlap with the deferred magic refactor's [[Power system]] and [[Blessing system]]).
- Whether a mythic item can be DESTROYED (a hammer-of-the-righteous shatters Excalibur?) — flavor question with mechanical consequences.

Source: combat refactor brainstorm 2026-05-21.
