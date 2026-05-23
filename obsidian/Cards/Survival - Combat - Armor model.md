CDDA-style layered armor on six body parts, with three armor-class skills that reduce wearing penalties. Period-accurate categories for Cornwall/Devon ~1300.

## Body parts (CDDA-classic six)

- **Head** — coverage weight 11%
- **Torso** — coverage weight 35%
- **Left arm** — coverage weight 12%
- **Right arm** — coverage weight 12%
- **Left leg** — coverage weight 15%
- **Right leg** — coverage weight 15%

Hands and feet are folded into arm/leg slots (a glove counts as covering the arm's "hand sub-region"). See [[Survival - Combat - Damage math and hit roll]] for limb HP and crippling.

## Equip slots

One armor slot per body part: `head`, `torso`, `L_arm`, `R_arm`, `L_leg`, `R_leg`. Plus:

- `off_hand` — shield, knife, or empty (see [[Survival - Combat - Weapon skills and proficiencies]]).
- `main_hand` — wielded weapon.

Multiple armor pieces can occupy the same body part if they layer (e.g. padded gambeson + mail hauberk over torso). Layering order matters (inner → outer).

## Armor piece data

Each armor piece has:

- **Coverage %** per body part it covers (0–100). On hit, 1d100 vs this rolls whether the piece catches the strike.
- **DR (bash / cut / stab)** — damage-type-specific flat damage reduction. Mail blocks cut well, blunt poorly; padded blocks bash well, stab poorly; plate blocks all but is heavy.
- **Encumbrance** per body part it covers.
- **Material / category** — Padded, Mail, or Plate (matches the skill axis).

## Hit resolution against armor

On a successful attacker hit (see [[Survival - Combat - Damage math and hit roll]]):

1. Roll a body part by coverage weight.
2. For each armor piece covering that body part, in layering order (outer first):
   1. Roll 1d100 vs piece's coverage %.
   2. If caught, subtract the piece's per-type DR from the incoming damage triplet.
   3. Continue to the next layer with the remaining damage.
3. Apply remaining damage to that body part's HP, separated by type.

## Period-accurate armor categories

Three skills, three exemplar tiers (Cornwall/Devon ~1300):

### Padded / Textile (skill: **Light Armor**)
- **Padded doublet** — torso + arms. Cheap. Yeoman tier.
- **Gambeson** — torso, heavier. Often worn under mail.
- **Leather jerkin** — torso. Listed at ~5s in late-13c merchant inventories.
- DR profile: high bash, low cut, very low stab. Light encumbrance.

### Mail (skill: **Medium Armor**)
- **Hauberk** — torso + arms. The signature elite armor of the period (statute mandates for richer men). Expensive (~100s per the price compilation).
- **Mail chausses** — legs. Common from c.1200 on mounted warriors.
- **Mail coif** — head + neck. Often worn under a helm.
- DR profile: low bash (you still get bruised through mail), high cut, medium stab. Medium encumbrance.

### Plate (skill: **Heavy Armor**)
- **Coat-of-plates** — torso. Small rectangular plates riveted to a fabric or leather garment. The bleeding-edge transition armor of ~1300; the start of plate proper.
- **Articulated limb plates** — emerging post-1300; rare in v1 setting.
- **Great helm** — head. Cavalry-elite. Heavy enclosed.
- **Bascinet** — head. Emerging post-1300; rare in v1 setting.
- **Iron skullcap / kettle hat** — head. Cheap and common; technically iron-not-plate, but lives under the same skill for simplicity.
- DR profile: high bash, high cut, high stab. Heavy encumbrance.

Mythic / faerie plate (demon-forged, faerie steel, etc.) lives in [[Survival - Combat - Mythic and faerie arms]] and may break these rules.

## Encumbrance penalties (full CDDA)

Sum the encumbrance values across all armor pieces covering each body region:

- **Leg encumbrance** → raises move-cost (your move-action costs more moves).
- **Torso + arm encumbrance** → lowers your effective Dodge skill (Dodge rolls go down by encumbrance points).
- **Total encumbrance** → raises stamina cost of running, dodging, and heavy actions (see [[Survival - Combat - Stamina]]).

Encumbrance is the price of armor. Skill mitigates.

## Armor skills mitigate the wearer's penalties

While wearing armor of class X:

- Higher **Light Armor** skill → reduces encumbrance penalty of padded pieces (and their move-cost / dodge / stamina drag).
- Higher **Medium Armor** skill → reduces encumbrance penalty of mail pieces.
- Higher **Heavy Armor** skill → reduces encumbrance penalty of plate pieces.

A skilled mail-wearer dodges nearly as well as someone in padding; an unskilled mail-wearer is sluggish. Specialization is real.

Skill XP rules: see [[Survival - Combat - Skill XP sources]].

## Shield handling

Shields are NOT armor pieces in this card. They live in the `off_hand` slot, give an active Block-chance bonus, and train the Block proficiency (see [[Survival - Combat - Weapon skills and proficiencies]]).

## Open

- Concrete coverage % and DR numbers per piece — deferred to balance card.
- Whether layering allows a hard cap (e.g. max 3 pieces on torso) or trusts encumbrance to self-balance.
- Whether the iron skullcap should belong to Light Armor (it's cheap and not really plate) or stay under Heavy Armor for simplicity.
- Hand/feet armor slots: deferred. v1 folds gloves into arm slot, boots into leg slot.

Source: combat refactor brainstorm 2026-05-21.
