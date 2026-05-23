# Cornwall & Devon c.1300 — Player Identity, Factions, the Four Pillars, Calendar

**Status:** Draft 1 — design reference, not implementation spec.
**Source of historical content:** `/home/campjun/Downloads/deep-research-report.md`, esp. §Culture, law, identity, and myth; §Population, settlement, and lordship; §Economy, markets, and taxation.
**Related drafts:** [[Cornwall-World]], [[Cornwall-Sites]], [[Arthurian-Wound]].

---

## 1. Player character — slice 1 vs eventual

### 1.1 Slice 1: Pilgrim from Exeter

The first implementation has **one** player template: a **pilgrim** spawning at Exeter Cathedral's western porch. This is the minimum-viable start that exercises every other system in the design without requiring a character-creation UI.

Fixed starting state:

- **Name:** player-entered text (the existing menu already supports this)
- **Location:** Exeter Cathedral porch, chunk (0, 0), cell (~20, ~14) inside the cathedral close
- **Date:** **Easter Monday, 17 April 1301** (see §5)
- **Pack (≤ 15,000 g):**
  - Pilgrim's staff (light bludgeon; +1 to walking speed cell-cost reduction in fast-travel; tradition-loaded)
  - Wool cloak (warmth)
  - Linen tunic and braies (already worn)
  - Letter of Introduction from Exeter Cathedral (`PaperKind::CathedralLetter` — gates some clergy interactions)
  - 12 pennies (`d.` — silver, ~15 g total)
  - Existing starting kit from `src/world.rs`/`src/items.rs`: ration (2x), waterskin (full, 4 uses), tinder, kindling, fuel, axe, herb satchel
- **Affinities:**
  - christian_devotion: 2
  - older_powers_devotion: 0
  - english_affinity: 4 (born in Devon)
  - cornish_affinity: 0
  - stannary_standing: 0
- **Skills:** Lore 1, Cornish 0 (the language is learnable; default 0), Tinwork 0, Foraging 1, Cooking 1
- **Known sites on overmap:** Exeter only

### 1.2 Eventual: CDDA-style character creation

A later session adds a full chargen flow. The shape is a **background table + skill array + starting kit + starting location** — exactly CDDA's pattern.

Background table (proposed):

| Background | Starting site | Starting affinities | Starting skills | Starting kit additions |
|---|---|---|---|---|
| **Pilgrim from Exeter** (slice 1 default) | Exeter | christian +2, english +4 | Lore 1, Foraging 1, Cooking 1 | Cathedral letter, pilgrim's staff |
| **Cornish wanderer** | Bodmin | cornish +4, older +2 | Cornish 2, Foraging 2, Lore 1 | Folding knife, woven blanket |
| **Tinner of Dartmoor** | Tavistock | english +3, stannary +5 | Tinwork 3, Mining 2, Brawling 1 | Tin pick, smelter token, hammer |
| **Tinner of Bodmin Moor** | Bodmin or Lostwithiel | cornish +3, stannary +5 | Tinwork 3, Mining 2, Cornish 1 | Tin pick, leather apron |
| **Fisher of Dartmouth** | Dartmouth | english +3 | Fishing 3, Sailing 1 | Hand-line, net, oilcloth |
| **Augustinian novice** | Bodmin or Plympton or St Germans | christian +5, cornish +1 (if Cornish house) | Lore 3, Latin 2, Healing 1 | Habit, book of hours, small bell |
| **Hedge-witch** | Anywhere rural (random) | older +5, cornish +3 | Foraging 3, Herbalism 3, Lore 2 | Herb bundles, mortar, charms |
| **Dispossessed knight** | Random borough | english +5 | Brawling 3, Sword 2, Riding 2 | Sword (unauthorized — could draw shire attention), gambeson, signet ring |
| **Cornish bard** | Truro or Helston | cornish +5, older +3 | Cornish 3, Lore 3, Performance 3 | Harp, traveling clothes |
| **Tregeagle's penitent** | Dozmary Pool | older +5, christian +3 | Lore 2, Endurance 3 | Pilgrim's rags, the Leaky Shell (cursed start) |

Eventual chargen UI: list of backgrounds, drill-into-detail, optional skill point reallocation, name entry, then "begin." Player can also customize appearance (hair color, etc.) via the existing CP437 glyph palette.

For drafts purposes, **the priority is the slice-1 fixed Pilgrim**; the wider system is sketched here for forward continuity.

---

## 2. Identity axes (deep dive)

The five-axis system. The first four are described in [[Arthurian-Wound]] §6; the fifth is `stannary_standing`.

### 2.1 stannary_standing

- Range: 0–10.
- Bumps from: sworn tin work (a player verb), assisting a stannary court, defending tinners from a shire bailiff, attending a stannary parliament (rare event).
- Affects:
  - Tin prices in stannary towns (high standing → buy low, sell high)
  - Whether tinners' enclaves on Dartmoor/Bodmin Moor are *safe* for the player or hostile
  - Access to stannary court justice (you can sue and be sued only in stannary court if you're affiliated)
  - The 1305 charter (event triggering in-game at the historical date — see §5.5) gives big bumps to all tinners' standings if Cornish

### 2.2 Affinity display

A status screen (menu key `C` for "character") shows:

```
Affinities
   Christian devotion:  ■■□□□□□□□□  2/10
   Older powers:        □□□□□□□□□□  0/10
   English shire:       ■■■■□□□□□□  4/10
   Cornish polity:      □□□□□□□□□□  0/10
   Stannary standing:   □□□□□□□□□□  0/10

Reputation by faction
   English Crown        Neutral
   Exeter Bishopric     Favored
   Devon Stannary       Unknown
   Cornish Polity       Unknown
   Cornish Stannary     Unknown
   Saint cults (known): Petroc — Acquainted
   Older Powers         Indifferent
```

Reputation is *derived* from affinities + completed rites + tracked events (you killed a stannary bailiff → Devon Stannary hostile).

---

## 3. Faction map

### 3.1 English Crown / shire administration

- **Seats:** Exeter (county town, Devon assize), Launceston (Cornwall assize), Lydford (Dartmoor stannary court, prison)
- **NPC archetypes:** shire reeve, bailiff, sergeant-at-arms, royal justice (in town only at assize sessions), tax collector
- **Relations:**
  - Friendly to high `english_affinity` players
  - Demands toll at Tamar crossings (12 d. or papers) — see [[Cornwall-World]] §8
  - Hostile if player has killed a shire official, been declared outlaw, or has stolen from a royal granary
  - Will arrest the player for major crimes; prison = Lydford
- **Player verbs:** pay toll, swear king's peace at the borough cross, serve as juror at an assize (long event), buy a royal pardon (expensive)

### 3.2 Cornish polity

- **Seats:** Bodmin (effective center), Truro (growing), Helston (west market)
- **NPC archetypes:** Cornish gentry, Cornish-speaking traders, churchwardens at Cornish saint-cult churches, hedge-witches loosely aligned
- **Relations:**
  - Friendly to high `cornish_affinity` players
  - Wary of strict English shire-affiliated players west of the Tamar
  - Will hide a player from shire bailiffs if `cornish_affinity` ≥ 5
- **Player verbs:** learn Cornish (a skill that levels through use), participate in a Cornish-language tribunal, join the Petroc procession at Bodmin
- **Historical note:** This is *pre-Duchy* — the Duchy of Cornwall is created in 1337. In 1300, "Cornish polity" is more of a cultural-linguistic identity than a separate jurisdiction. We frame it that way in dialog.

### 3.3 Exeter Bishopric

- **Seats:** Exeter Cathedral; abbeys at Tavistock, Plympton, Buckfast, Torre; priories at Bodmin (St Petroc), St Germans, etc.
- **NPC archetypes:** bishop, archdeacon, canons of the cathedral, abbots, priors, parish priests, friars
- **Relations:**
  - Friendly to high `christian_devotion`
  - Refuses certain blessings to players who have declared the older powers as "wholesome"
  - Will arrange for re-consecration of a desecrated chapel ([[Arthurian-Wound]] §3.1)
- **Player verbs:** confess, ask for blessing, donate to almonry, take a vow (binding mechanical pledge with reward + penalty), accept a pilgrimage commission

### 3.4 Devon stannary

- **Seats:** Tavistock, Chagford (procgen), Ashburton (procgen), Plympton, Lydford (court)
- **NPC archetypes:** stannary bailiff, tin merchant, smelter, miner, charter-master
- **Relations:**
  - Open to anyone willing to swear the tinners' oath (one-time event at any of the four Devon stannary towns)
  - Defends sworn tinners against shire jurisdiction (the 1201 charter)
- **Player verbs:** swear the tinners' oath (gates tin work), buy worked tin, sell raw tin, sue in stannary court, attend stannary parliament

### 3.5 Cornish stannary

- **Seats:** Lostwithiel (administrative capital), Bodmin (coinage town), Liskeard (coinage from 1307 — *just after our date*), Truro, Helston
- **NPC archetypes:** same as Devon stannary plus the earldom administration at Lostwithiel
- **Relations:** as Devon's
- **Player verbs:** as Devon's, plus the **1305 Cornish charter event** — a year after game start, a charter is publicly read at Lostwithiel that strengthens Cornish stannary liberties. Attending grants a one-time `cornish_affinity +1`, `stannary_standing +1` boost
- **Historical note:** The 1305 charter is a real event ("New Cornish stannary charter" in the report's timeline). We trigger it at the correct in-game date.

### 3.6 Saint cults

Each saint cult is a small per-saint network. Major cults:

- **St Petroc** at Bodmin — large Cornish cult, relics
- **St Madern** at Madron — well + chapel
- **St Clether** at Clether — well + chapel
- **St Germanus** at St Germans Priory
- **St Rumon** at Tavistock Abbey (relic)
- **St Juliot** at Tintagel chapel
- **St Nectan** at the Trevillet kieve (later attribution; in 1300 unattached — we keep it loose)
- **St Petroc's deer** as a companion (rite reward)

Cult relations: each cult has a small reputation track. Visiting the cult site on the saint's feast day grants larger boons. Particular saints are syncretic with older powers (Madern, Nectan).

### 3.7 Older powers

Not a hierarchical faction — a *diffuse network*. Sites are independent. Recognized by:

- High `older_powers_devotion` triggers visibility of "Older folk" NPCs at stone circles, holy wells, deep moors
- Specific NPCs: the Hurler-keeper, the Mên-an-Tol watcher, the Tregeagle of Dozmary, the Wisht-Hound's master on Dartmoor
- Mythic-real artifacts in [[Cornwall-Sites]] §4 are mostly older-powers gated

### 3.8 Wound manifestations

Universally hostile to humans, but the player can briefly bind some via specific rites (this is dangerous and turns the wound on the player). The "binding the Wisht Hounds" rite is a late-game option for the older-powers maximalist.

### 3.9 Smaller / situational factions

- **Outlaws** — local bandit camps; hostile by default; some can be bargained with at high `older_powers_devotion`
- **Wreckers** — coastal robbers; appear on `CoastCliff` chunks at storm
- **Smugglers** — on estuarine `EstuaryMarsh` chunks; can be bought into to move goods illicitly
- **Royal verderers of Exmoor** — strict forest law enforcement
- **The Wild Hunt** — Wound manifestation that operates as a "faction-like" recurring threat

---

## 4. The four-pillar game loop

The user pinned all four pillars (survive + pilgrim + trade + political). Each pillar has its own verbs, each weaving through the others.

### 4.1 Survival pillar

**Verbs (current systems, extended for biomes):**

- Forage — biome-specific forage tables; fewer/different items in BlightedWaste; abundant herbs in OakWoodland
- Hunt — wild game spawns per biome (deer in Exmoor; rabbit on `CoastCliff`; fish on `RiverValley` and `EstuaryMarsh`); existing fishing verb already shipped
- Camp — pitch tent, light fire, bedroll (existing)
- Travel — manual or fast-travel (see [[Cornwall-World]] §6)
- Sleep — restore Sleep need

**Extensions needed:**

- **Per-biome forage table** in `src/world.rs` or `src/biome.rs`
- **Seasonal foraging** — herbs scarce in winter, berries only in summer, mushrooms in autumn
- **Per-biome ambient temperature** — DartmoorGranite -5°C from regional avg in winter
- **Per-biome night length tweak** — moors feel longer
- **Weather** — rain, sleet, snow, fog; wind on the cliffs

### 4.2 Pilgrimage pillar

**Verbs:**

- Visit a sacred site — passive (visiting raises devotion by small amounts)
- Perform a rite — see [[Arthurian-Wound]] §5
- Take a vow — binding short-term constraint with reward (e.g. "vow of silence for 7 days → +3 christian_devotion at completion; -2 if broken")
- Accept a commission — a clergy NPC sends you to deliver a relic, retrieve a manuscript, escort a pilgrim
- Attend a feast — feast days amplify rites; calendar visible in menu

**Pilgrimage routes:**

A formal pilgrimage is a chain of three sites. The completion unlocks a route boon:

- **The South-Cornish Saints' Way:** Bodmin (Petroc) → Lostwithiel (Bartholomew) → St Germans (Germanus). Boon: +2 christian_devotion, +1 cornish_affinity, the Petroc Deer companion
- **The Old Stones' Round:** The Hurlers → Mên-an-Tol → Merry Maidens. Boon: +3 older_powers_devotion, the Old Stones' Eye (see invisible at stone-ringed chunks)
- **The Tamar Crossing:** Launceston (eastern border) → Lydford (forest court) → Tavistock (abbey). Boon: +1 english, +1 stannary, "the Tamar's truce" (the Tamar tolls drop for the player for a year)
- **The Grail Way:** Exeter → Tintagel inner chamber (the Bedchamber Stone) → three saint rites → the hidden Grail chunk → Tintagel again. Boon: world-changing ([[Arthurian-Wound]] §7)

### 4.3 Trade / economy pillar

**Verbs:**

- Buy / sell at a market or fair
- Carry goods between markets for profit (the classic trade loop)
- Coinage — Cornish tin must be assayed and stamped at a coinage town (Bodmin, Lostwithiel, Truro, Helston, Liskeard from 1307) before legal sale
- Pay tolls (Tamar crossings, bridge tolls)
- Hire boat passage (port to port; expensive but fast)
- Buy a royal pardon, papers of safe-conduct, a stannary token

**Major markets:**

| Town | Market day(s) | Specialty | Notes |
|---|---|---|---|
| Exeter | Mon, Wed, Fri (3 days a week by 1281) | Cloth, grain, leather, fish | Biggest market; cathedral oversight |
| Barnstaple | Sat | Wool, livestock | North-Devon hub |
| Totnes | Sat | Cloth, grain, fish | Dart corridor |
| Plymouth/Sutton | Wed, Sat | Fish, salt-fish, Gascon wine | Port |
| Dartmouth | Sat | Wine, pilgrim provisions, sea charters | Port |
| Tavistock | Fri | Tin (Devon stannary), abbey produce | Stannary court town |
| Launceston | Sat | Mixed (border market) | Tamar east |
| Bodmin | Sat | Cornish saints' relics, tin (Cornish stannary), wool | Largest in Cornwall |
| Lostwithiel | Tue, Fri | Tin, riverine goods | Stannary capital |
| Truro | Wed, Sat | Tin, riverine goods | Rising tin market |
| Helston | Sat | Salt, fish, west-Cornwall produce | West market |
| Liskeard | Tue | Local Cornwall produce | Small market |

**Annual fairs:** (a fair lasts ~3 days)

- Exeter — Lammas Fair (early August), Michaelmas Fair (late September)
- Bodmin — St Petroc's Fair (June 4)
- Tavistock — Goose Fair (late September)
- Barnstaple — Sept Fair
- Truro — Whitsun Fair (Pentecost)
- Helston — Trinity Fair

Fairs scale prices and item variety up 3×; carrying goods to a fair is a common trade route.

**Tin economy:**

- Raw tin ore is mined from `DartmoorGranite` / `BodminMoorGranite` cells with a `Tinwork` skill check
- Smelted tin is made at a smelter (NPC service at stannary towns)
- *Stamped* tin (assayed at a coinage town with a coinage hall) sells for 2× unstamped
- The 1201 charter exempts tinners from many shire dues but requires them to use stannary court (a binding commitment)
- Worked tin items are warded against wound manifestations (+50% damage; see [[Arthurian-Wound]] §10)

**Currency:**

- 1 d. (denarius / penny) — silver, ~1.5 g
- 12 d. = 1 s. (shilling)
- 20 s. = 1 £ (pound)
- Half-penny and farthing (¼d.) exist
- Player carries pennies as `ItemKind::Pennies(u32)` — abstracted weight (~1.5 g each, lumped)

Sample prices (illustrative):

| Item | Price |
|---|---:|
| Quart of small beer | 0.5 d. |
| Loaf of bread | 0.25 d. |
| Pound of cheese | 1.5 d. |
| Pound of salt | 2 d. |
| Day's lodging at an inn | 1 d. |
| Tin cup (worked) | 4 d. |
| Sword (sergeant grade) | 36 d. (3 s.) |
| Boat passage Dartmouth → Plymouth | 6 d. |
| Pilgrim badge (Petroc) | 2 d. |
| Royal pardon | 240 d. (£1) |
| Tamar toll | 1 d. or papers |

### 4.4 Combat / political pillar

**Combat verbs:** the existing scope is small. We don't expand it in this draft. The pillar instead emphasizes *political combat* — toll-disputes, court actions, faction loyalty.

**Political verbs:**

- **Submit at a border** — Tamar crossings demand toll + papers; show your Letter of Introduction or pay 1 d.
- **Refuse a sheriff** — kicks off a "wanted" event chain
- **Outlawry** — being declared outlaw means the shire NPCs are hostile in their county but the older folk / Cornish hide-out network may aid you
- **Sanctuary** — claiming sanctuary at a major church protects you from arrest for 40 days (canon law)
- **Court duel** — for some disputes, accept trial by combat
- **Swear an oath** — see Affinities §6 of [[Arthurian-Wound]]; binding pledges
- **Pay a tinners' fine** — bypass shire law if you're a sworn tinner

**Combat encounters** (existing demon system, reframed per [[Arthurian-Wound]] §8) — these are wound manifestations + bandits + wolves + Wild Hunt + occasional shire bailiff pursuit.

---

## 5. Calendar

### 5.1 Start date

**Easter Monday, 17 April 1301.** Rationale:

- It is *just* into the spring travel season — foraging is starting, weather is improving but unpredictable on Dartmoor
- It is **before** the 1305 Cornish charter — so the player experiences the charter as an in-game event
- It is mid-Edward I (a politically active king; the 1301 Carlisle Parliament; the Scottish wars are draining the kingdom)
- Easter Monday is a feast — the player wakes after the Easter celebrations, which fits the pilgrim spawn

### 5.2 Year structure

- 365-day year (no leap-year for simplicity; can add later)
- Real Sarum-rite liturgical calendar with movable feasts computed correctly (Easter, Pentecost, Trinity, Corpus Christi)
- Saints' feast days as in [[Arthurian-Wound]] §9

### 5.3 Seasons

| Season | Months | Effects |
|---|---|---|
| Spring | Mar 21 – Jun 20 | Foraging recovers; lambs; bird-egg foraging on cliffs; warmer nights begin May |
| Summer | Jun 21 – Sep 22 | Best foraging; longest days (15 hours light at midsummer in Cornwall); harvest begins August |
| Autumn | Sep 23 – Dec 20 | Mushrooms; nuts; deer rut; market peaks (Michaelmas fairs); fading foraging |
| Winter | Dec 21 – Mar 20 | Bare; cold; Dartmoor / Bodmin Moor lethal without shelter; many feast days but bleakest pilgrimage season; wound diffusion peaks |

### 5.4 Day-night cycle

The existing `clock_seconds` system handles this. Sunrise/sunset times vary by latitude (Cornwall, ~50.3°N) and date — straightforward to compute. FOV radius is already 20 daytime / 3 nighttime ([[Cornwall-World]] §9 open questions).

### 5.5 Scripted in-game events on real dates

- **17 April 1301** — game start
- **1 May 1301** — Beltane / May Day; Hal-an-Tow procession at Helston
- **24 June 1301** — Midsummer; Merry Maidens dance
- **1 August 1301** — Lammas; harvest begins
- **29 September 1301** — Michaelmas; major fair at Exeter
- **31 October 1301** — All Hallows; perilous night
- **21 December 1301** — Midwinter; wound peaks; older powers strongest
- **25 December 1301** — Christmas
- **March 1305** — Edward I issues the Cornish stannary charter at Lostwithiel; major in-game event with celebration / unrest depending on faction

### 5.6 Year-arc gameplay

A single playthrough is *expected* to span multiple in-game years. Saves persist across years; the calendar wraps. Annual events (fairs, feast days, the wound's midwinter peak) become familiar rhythms.

The Grail arc isn't bound to a specific year. The player can complete it in year 1 or year 7.

---

## 6. Starting Exeter scenario walkthrough

The player's first hour. End-to-end design walkthrough for the slice-1 start. Each step exercises one piece of the design.

### Step 1: Spawn at the cathedral porch

- Renders: the cathedral's west-front porch chunk (authored); a few NPCs (a priest sweeping the porch, two pilgrims with bundles, a beggar)
- The priest greets the player: *"Welcome, pilgrim. Bishop Bytton awaits no one in particular — but the Letter you carry will open most doors west of the Tamar. Make your way as you will."*
- The player learns their starting location and that they're free to act

### Step 2: Explore Exeter city

- Walk south down the cathedral close, out through the south gate into the market chunks
- Buy supplies: more bread, a small flask of beer, maybe a tin pilgrim badge (1 d.; gates a small saint-cult interaction at any Petroc-related site)
- Talk to the market clerk to *learn-of* a few sites — Bodmin, Tavistock, Plymouth — adding them to the overmap with `?` markers

### Step 3: Leave Exeter

- Cross the Exe bridge (south gate of the city) and head west on the Roman-era road remnant
- The road is now a `RoadDirt` strip running through `LowlandFarm` and `BeechCombe` chunks
- Fast-travel toggle becomes useful: open overmap (the player has discovered ~5 chunks now), notice Exeter pinned and the road heading west
- Click Tavistock — fast-travel queue activates; the player auto-walks along the road, witnessing the landscape stream past

### Step 4: First survival night

- It's now late afternoon; the queue auto-cancels when Sleep need crosses the warning threshold
- Player is now somewhere along the road, ~10 km west of Exeter, in a `BeechCombe` chunk
- Pitch tent, light a fire (existing systems), eat a ration, drink water, sleep
- Wake at dawn

### Step 5: Approach the Tamar

- Resume fast-travel toward Tavistock
- After ~4 hours of in-game travel the player crosses the Tavistock town edge
- Tavistock Abbey rises into view — a Sacred-resonance node with a powerful Christian field

### Step 6: First real choice

- The abbey gate guard asks the player's business
- If the player shows the Letter of Introduction, they're welcomed and given guest lodging (free)
- If they're rude or hide the letter, they're treated as a stranger (must pay 1 d. for lodging in town)
- Inside the abbey, the abbot offers a *pilgrimage commission*: deliver a sealed letter to St Petroc's at Bodmin
- Accepting the commission is the player's first quest

### Step 7: Cross the Tamar (the choice)

- The road from Tavistock west crosses the Tamar at Greystone Bridge or Polson Bridge (near Launceston)
- At the bridge, a sergeant demands toll
- The player can pay (1 d.) or show their abbey letter (free passage) or talk their way through (Speech check, gated on Lore skill)
- Crossing the Tamar westward shifts NPC dialog — at Launceston (just west of the bridge), Cornish-speaking NPCs appear; ambient flavor changes

### Step 8: Arrive at Bodmin

- After another fast-travel leg the player reaches Bodmin
- Deliver the letter to St Petroc's priory; quest complete
- The prior offers: a blessing (mass attendance — +1 christian_devotion), a meal, and a *new* commission (south to St Germans, or north to Tintagel)

### Step 9: The world opens

- By the end of the first 2–3 hours of play, the player has:
  - Walked or fast-traveled ~95 km
  - Visited 3 named sites
  - Learned of 6+ more
  - Earned ~12 d. in commissions and lost ~6 d. on supplies
  - Had one mild wound encounter on Bodmin Moor (a Wisht Hound on the road south)
  - Bumped affinities meaningfully (christian +2 from the abbey + priory; cornish +1 from crossing the Tamar and engaging with Cornish-speaking NPCs)
- The "Caves of Qud × CDDA" loop is now alive: a vast partially-known map, many sites unlocked, a quest hook in hand, a survival baseline maintained

### Step 10: Long-term hooks

- Pilgrimage commissions chain across the peninsula
- The Bedchamber Stone vision (eventually, after sufficient christian_devotion) starts the Grail arc
- The 1305 charter is 4 in-game years away — long-term event the player can prepare for / position themselves for
- Trade between Plymouth (Gascon wine) and Bodmin (Cornish saints' market) becomes profitable
- The wound's midwinter peak (December) becomes a survival challenge in its own right

---

## 7. Implementation summary (sketch, not spec)

### 7.1 New modules

- `src/affinity.rs` — affinity vector, getters/setters, gate evaluator
- `src/faction.rs` — faction list, reputation derivation
- `src/calendar.rs` — Sarum calendar, feast days, season/weather
- `src/quest.rs` — pilgrimage commissions, quest state machine
- `src/dialog.rs` — minimal NPC dialog tree system (for the slice-1 priest, abbot, prior, sergeant, market clerk)

### 7.2 Touchpoints

- `src/save.rs` — RunSave gains `affinities`, `factions`, `calendar_day`, `active_quests`. Schema bump.
- `src/main.rs` — starting state is no longer just an oasis; spawn at Exeter cathedral porch, set affinity defaults, set calendar date.
- `src/world.rs` — new `Interactable` enum for NPCs / shrine doors / market stalls; placed by authored chunks.

### 7.3 Slice-1 minimum implementation

This is *not* the full draft. The minimum to ship a playable slice 1 is:

1. wgs84_to_cell + named-site coord table (no rites yet)
2. Biome layer (just enough biomes: LowlandFarm, OakWoodland, BeechCombe, DartmoorGranite, BodminMoorGranite, RiverValley, EstuaryMarsh, CoastCliff, CoastBeach, Sea, TownEdge)
3. Authored chunks for Exeter (9×9), Tavistock (5×5), Launceston (5×5), Bodmin (7×7), Tintagel (5×3). Other sites can come later.
4. Overmap UI + fast-travel queue
5. Tamar toll mechanic
6. Affinity vector + status screen
7. Minimal NPC dialog at Exeter / Tavistock / Bodmin (priest, abbot, prior, sergeant, market clerk)
8. One commission chain (Tavistock → Bodmin → St Germans)
9. Calendar (date display in status bar; one feast day mechanically active — Pentecost)

Wound mechanics + the Grail arc + the full rite system land in a later slice.

---

## 8. Process notes

- This document is the *bridge* between [[Cornwall-World]] / [[Cornwall-Sites]] / [[Arthurian-Wound]] and the player's experience. Every concept here ties back to one of the other three drafts.
- The "Easter Monday, 17 April 1301" start date is a recommendation; the user can override.
- The Pilgrim default is intentionally bland-ish — a player who wants high-flavor starts uses the later chargen system.
- The slice-1 minimum implementation list in §7.3 is a useful starting point for breaking the work down once we move from draft to implementation planning.
