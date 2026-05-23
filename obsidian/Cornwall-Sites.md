# Cornwall & Devon c.1300 — Named-Site Gazetteer & Authoring Schema

**Status:** Draft 1 — design reference, not implementation spec.
**Source of historical content:** `/home/campjun/Downloads/deep-research-report.md`, §Gazetteer and GIS map package.
**Related drafts:** [[Cornwall-World]], [[Arthurian-Wound]], [[Cornwall-Pilgrim]].

---

## 1. The site schema

Every named site in the world is a record in a `NamedSite` table. The runtime keeps these in `src/sites.rs` (proposed new module) as a static `&[NamedSite]` slice.

```
struct NamedSite {
    id:                   SiteId,          // stable string id, e.g. "exeter"
    c1300_name:           &'static str,    // Exonia / Excestre
    modern_name:          &'static str,    // Exeter
    lat:                  f64,
    lon:                  f64,
    cell:                 (i64, i64),      // precomputed via wgs84_to_cell
    chunk:                (i32, i32),      // precomputed
    kind:                 SiteKind,
    authored_chunks:      (i32, i32),      // size of authored region in chunks, e.g. (9, 9)
    population_c1300:     Option<u32>,     // working estimate, or None for non-settlements
    factions:             &'static [FactionAffiliation],
    wound_resonance:      WoundResonance,  // Sacred / Profane / Neutral / Liminal / WoundEpicenter
    period_authentic_legends: &'static [&'static str],
    soft_folklore_layers:     &'static [&'static str],   // "everything real" => mechanically active
    artifacts:            &'static [ArtifactId],
    description:          &'static str,    // short narrative, ~2-3 sentences
}

enum SiteKind {
    CathedralCity, MajorBorough, MinorBorough,
    Castle, Priory, Abbey, ChurchTown,
    HolyWell, StoneCircle, SacredPool, SeaCave, Barrow,
    Moor, RiverFord, PortHarbor, RuinPrecinct,
}

enum WoundResonance {
    Sacred,         // emits healing; reduces nearby wound_intensity
    Profane,        // emits blight; raises nearby wound_intensity
    Neutral,
    Liminal,        // threshold places (fords, sea caves) — both heal and harm
    WoundEpicenter, // Tintagel; the Fisher King's wound
}

enum FactionAffiliation {
    EnglishCrown, CornishPolity, ExeterBishopric,
    DevonStannary, CornishStannary,
    SaintCult(&'static str),    // e.g. "Petroc", "Madern", "Clether"
    OlderPowers,
}
```

The static table is the source of truth. Runtime mutations (player has visited, NPC has been killed, the rite has been completed) live in `RunSave`.

---

## 2. The 24+ named sites

Each entry follows the same shape: header (c.1300 name and modern name), one-line description, coordinates and chunk count, faction and wound resonance, period-authentic legends (real in the c.1300 literature), soft folklore (later traditions — *in our world, mechanically real per user choice*), authored chunk sketch.

> Source for every entry: the gazetteer in §Built sites and §Landscape/hydrology/ritual/legend of the research report.

### 2.1 Exonia / Excestre — Exeter

- **WGS84:** 50.7224 °N, -3.5290 °E
- **Cell:** (0, 0)
- **Chunk:** (0, 0)
- **Kind:** CathedralCity
- **Population c.1300:** ~7,000–9,000 (the largest urban node on the peninsula)
- **Authored chunks:** 9 × 9 (~550 m × ~415 m footprint — cathedral close, marketplace, three gates, walls, ribbon of suburbs)
- **Factions:** EnglishCrown, ExeterBishopric, DevonStannary (administrative seat for stannary regulation)
- **Wound resonance:** Sacred (cathedral) + Neutral (city as a whole)
- **Period-authentic legends:** Cathedral relics, Bishop Bronescombe's tomb (d.1280), the unbroken series of customs accounts from 1266
- **Soft folklore:** None major — Exeter is the *secular center*
- **Artifacts:** *(none ritual)*; Exeter Cathedral Letter-of-Introduction (gating doc for the starting Pilgrim)
- **Authored sketch:**
  - **Center chunk (0,0):** Cathedral Close — cathedral porch, the western doors, the chapter house, the Bishop's Palace gate
  - **Chunks (-1..1, -1..1):** Inside the city — high street, market, guildhalls, residential lanes
  - **Outer ring:** Walls, gates (East, West, North, South), bridge over the Exe at the south gate, suburbs
- **Notes:** This is the player's spawn. The starting scenario is anchored here (see [[Cornwall-Pilgrim]] §6).

### 2.2 Barnestaple — Barnstaple

- **WGS84:** 51.0804, -4.0580
- **Cell:** (-24,578, -26,073)
- **Chunk:** (-615, -870)
- **Kind:** MajorBorough, PortHarbor
- **Population c.1300:** ~1,500–2,200 (third-wealthiest borough by 1332)
- **Authored chunks:** 5 × 5
- **Factions:** EnglishCrown, ExeterBishopric (peripherally)
- **Wound resonance:** Neutral
- **Period-authentic legends:** Earliest charter c.930 (tradition); long-standing borough rights
- **Soft folklore:** Boggart-on-the-bridge tale (later — but real per choice)
- **Authored sketch:**
  - Bridge over the Taw at the southwest chunk
  - Long-strand street running north from the bridge
  - Wool warehouse, customs jetty, market square with the borough cross
  - North gate at the edge of the authored region
- **Economy:** Wool, shipping, north-Devon agricultural exchange. A key node for the wool trade.

### 2.3 Totnes

- **WGS84:** 50.4315, -3.6855
- **Cell:** (-7,272, 21,185)
- **Chunk:** (-182, 706)
- **Kind:** MinorBorough, Castle
- **Population c.1300:** ~1,500–2,200
- **Authored chunks:** 5 × 5
- **Factions:** EnglishCrown (seigneurial borough)
- **Wound resonance:** Neutral
- **Period-authentic legends:** Brutus of Troy (foundation myth — "The Brutus Stone" was already a tradition by 1300; *real in-world*: stepping on it briefly hears the voice of Brutus)
- **Soft folklore:** none additional
- **Authored sketch:**
  - Castle keep (Norman motte-and-bailey) on the north chunk
  - Main street descending south to the Dart
  - Bridge over the Dart at the south chunk
  - Market cross, fish shambles
  - The Brutus Stone embedded in the main street pavement

### 2.4 Sutton / Plymouth

- **WGS84:** 50.3650, -4.1330
- **Cell:** (-28,063, 26,029)
- **Chunk:** (-702, 867)
- **Kind:** MinorBorough, PortHarbor
- **Population c.1300:** ~2,000–3,000
- **Authored chunks:** 5 × 5
- **Factions:** EnglishCrown, Plympton Priory (the inland abbey holds part of Sutton)
- **Wound resonance:** Neutral, Liminal at the harbor mouth
- **Period-authentic legends:** Cawsand smugglers (later); medieval-era port
- **Soft folklore:** Drake's stone (later, anachronistic for 1300 — skip)
- **Authored sketch:**
  - Sutton Harbour at the south edge — quays, fish market, salt-fish racks
  - Walled town on the north slope above the harbor
  - West gate looking toward the Plym crossing
  - Plympton Priory road exits east

### 2.5 Tavistoke — Tavistock Abbey

- **WGS84:** 50.5494, -4.1448
- **Cell:** (-28,610, 12,600)
- **Chunk:** (-716, 420)
- **Kind:** Abbey, MinorBorough
- **Population c.1300:** ~1,000–1,500 (abbey + dependents)
- **Authored chunks:** 5 × 5
- **Factions:** ExeterBishopric (technically the abbey is exempt; flavor it as a powerful peer of the bishopric), DevonStannary (one of the four Devon stannary courts)
- **Wound resonance:** Sacred (abbey precinct)
- **Period-authentic legends:** Founded 961 by Ordgar; relics of St Rumon
- **Soft folklore:** The Abbey's hidden tunnel to the Tamar (later — but in-world a real escape passage)
- **Authored sketch:**
  - Abbey precinct: cloister, church, refectory, chapter house at the center chunk
  - Town outside the abbey gates with the weekly market
  - Stannary court chamber (a small wing of the gatehouse) — this is where tinners come for justice in west Devon
  - West road out to Lydford and Dartmoor

### 2.6 Lideford — Lydford Castle

- **WGS84:** 50.6436, -4.1048
- **Cell:** (-26,754, 5,739)
- **Chunk:** (-669, 191)
- **Kind:** Castle, MinorBorough
- **Population c.1300:** small, ~300–500
- **Authored chunks:** 3 × 3
- **Factions:** EnglishCrown (royal forest law), DevonStannary
- **Wound resonance:** Liminal (the gorge and the tower) — a dread place
- **Period-authentic legends:** The 1260s tower as a notorious prison and court — already a place of judicial dread by 1300
- **Soft folklore:** "Lydford Law" (hang first, try after) is the later proverb; *real in-world*: stannary justice is harsh here
- **Authored sketch:**
  - The tower (a single squat keep) at the center chunk — prison cells below, courtroom above
  - Village around the tower
  - **Lydford Gorge** at the east edge — a deep ravine running south; a `Liminal` spot with sound effects and possible Wound spawns

### 2.7 Dunheved / Launceston

- **WGS84:** 50.6367, -4.3609
- **Cell:** (-38,653, 6,241)
- **Chunk:** (-967, 208)
- **Kind:** Castle, MajorBorough
- **Population c.1300:** ~1,500–2,500
- **Authored chunks:** 5 × 5
- **Factions:** EnglishCrown (assize/castle town, eastern entry into Cornwall)
- **Wound resonance:** Neutral
- **Period-authentic legends:** Charter 1225; Domesday market
- **Soft folklore:** Mary Magdalene church (later granite; in-world has powerful relic of Magdalene)
- **Authored sketch:**
  - Castle keep at the north chunk — circular shell keep
  - Walled town with assize court inside
  - South gate ("the gate of the castle")
  - Market square with the borough's cross
- **Border function:** This is the main *eastern entry checkpoint* into Cornwall. Crossing the Tamar to the east at Polson Bridge → Launceston enforces the **identity-papers** check for the first time. (See [[Cornwall-World]] §8.)

### 2.8 Bodmen — Bodmin

- **WGS84:** 50.4716, -4.7245
- **Cell:** (-55,546, 18,266)
- **Chunk:** (-1,389, 608)
- **Kind:** MajorBorough, Priory, coinage town
- **Population c.1300:** ~1,500–2,500 (probably Cornwall's largest medieval town)
- **Authored chunks:** 7 × 7 (bigger because of priory + market + coinage hall)
- **Factions:** CornishPolity, ExeterBishopric (via the priory), CornishStannary
- **Wound resonance:** Sacred (priory of St Petroc) + Neutral (town)
- **Period-authentic legends:** St Petroc's relics in the priory — major Cornish cult; coinage town since 1198
- **Soft folklore:** Petroc's deer (the saint befriended a stag — in-world the stag still wanders the moors at dawn near Bodmin)
- **Authored sketch:**
  - Priory of St Petroc at the center — large Augustinian house, the relic shrine
  - Coinage hall at the market square (where tin is assayed and stamped)
  - Castle Canyke (Iron Age fort, prehistoric but visible) at the southeast chunk
  - Market street, fish shambles, north and south town gates
  - Berry Tower visible from the east

### 2.9 Lostwithiel

- **WGS84:** 50.4078, -4.6664
- **Cell:** (-52,844, 22,912)
- **Chunk:** (-1,322, 763)
- **Kind:** MinorBorough, PortHarbor (river port), Castle-adjacent
- **Population c.1300:** ~1,000–1,500
- **Authored chunks:** 5 × 5
- **Factions:** CornishPolity, CornishStannary (the stannary administrative capital), EnglishCrown (earldom administration)
- **Wound resonance:** Neutral
- **Period-authentic legends:** Late-13th-century planned town; Duchy palace under construction
- **Soft folklore:** Tregeagle visits the Fowey here (he is doomed to eternal labor)
- **Authored sketch:**
  - Bridge over the Fowey (the lowest crossing)
  - Duchy palace complex at the riverbank — administrative buildings, stannary court, exchequer
  - Market square, the church of St Bartholomew
  - The river quays where tin is shipped downstream

### 2.10 Restormel — Restormel Castle

- **WGS84:** 50.4466, -4.6685
- **Cell:** (-52,940, 20,089)
- **Chunk:** (-1,324, 669)
- **Kind:** Castle
- **Population c.1300:** garrison + ~100 dependents
- **Authored chunks:** 3 × 3
- **Factions:** EnglishCrown / earldom
- **Wound resonance:** Neutral
- **Period-authentic legends:** Edmund of Cornwall's residence; shell-keep on a strong motte
- **Soft folklore:** none major
- **Authored sketch:**
  - The shell keep at the center chunk — circular, on a high motte, deer park surrounding
  - Approach road from Bodmin
  - Park gate to the south toward Lostwithiel
  - Inside: great hall, chapel, kitchens, lord's chamber

### 2.11 Truru — Truro

- **WGS84:** 50.2632, -5.0510
- **Cell:** (-70,713, 33,442)
- **Chunk:** (-1,768, 1,114)
- **Kind:** MinorBorough, PortHarbor (head of navigation), market town
- **Population c.1300:** ~1,000–1,500
- **Authored chunks:** 5 × 5
- **Factions:** CornishPolity, CornishStannary (coinage town a generation later)
- **Wound resonance:** Neutral
- **Period-authentic legends:** Landlord-planted town; growing tin market
- **Soft folklore:** None major specific to c.1300
- **Authored sketch:**
  - Three rivers converge — Kenwyn, Allen, Truro — at the south chunk
  - Quay where tin is shipped downstream to Carrick Roads
  - Market square, church of St Mary
  - Bridge over the Kenwyn

### 2.12 Sanctus Germanus / Lannaled — St Germans Priory

- **WGS84:** 50.3970, -4.3095
- **Cell:** (-36,264, 23,698)
- **Chunk:** (-907, 789)
- **Kind:** Priory
- **Population c.1300:** ~300–500
- **Authored chunks:** 3 × 3
- **Factions:** ExeterBishopric, CornishPolity, SaintCult("Germanus")
- **Wound resonance:** Sacred
- **Period-authentic legends:** Former bishopric (until 1043); Augustinian priory; major SE-Cornwall ecclesiastical focus
- **Soft folklore:** Germanus heals the lame (in-world: real, at a specific font)
- **Authored sketch:**
  - Priory church + cloister at center
  - Holy well of St Germanus (just outside the cloister gate)
  - Estuary edge (Lynher) at the south chunk — fishing dock

### 2.13 Tintagel / Dintagel — Tintagel Castle

- **WGS84:** 50.6680, -4.7599
- **Cell:** (-57,190, 3,962)
- **Chunk:** (-1,430, 132)
- **Kind:** Castle, RuinPrecinct
- **Population c.1300:** small garrison + chaplain
- **Authored chunks:** 5 × 3 (extends seaward; sea on the west chunks)
- **Factions:** EnglishCrown (Richard of Cornwall's castle, built 1230s)
- **Wound resonance:** **WoundEpicenter** — this is where the Fisher King's wound is
- **Period-authentic legends:** Geoffrey of Monmouth (1136) placed Arthur's conception here. By 1300 this is *settled literature* — pilgrims come to see the place. Richard of Cornwall built his castle here in the 1230s **specifically** because of the prestige.
- **Soft folklore (mechanically real per user choice):** Merlin's Cave is real; the Wounded King (the Fisher King) lies in a hidden chamber under the castle.
- **Artifacts:** The bedchamber stone (where Arthur was conceived; touching it gives a vision); the Wounded King's chamber (locked behind a long quest); access to Merlin's Cave below
- **Authored sketch:**
  - **Mainland ward** at chunk (-1,430, 132): outer gate, lower courtyard, the surviving 13th-c. wall
  - **Island ward** at chunk (-1,431, 132): inner courtyard, the great hall (Richard's), the chapel of St Juliot, the bedchamber stone in a partly-ruined inner room
  - **Cliff descent** at chunk (-1,430, 133): staircase carved into the cliff, Sea-Cave below (separate site, §2.16)
  - **Sea chunks** west and north — `Sea` biome
- **Special note:** Tintagel is the player's most important destination. The Grail arc resolves here ([[Arthurian-Wound]] §7).

### 2.14 Bodmin Moor (Foweymore)

- **WGS84:** ~50.5960, -4.6380 (centroid)
- **Cell:** (-51,527, 9,206)
- **Chunk:** ~(-1,289, 306) centroid; biome polygon ~20 × 15 km
- **Kind:** Moor — not a single site but a biome with multiple embedded sub-sites
- **Authored chunks:** None at centroid; sub-sites are authored individually below (Hurlers, Rillaton, Dozmary, King Arthur's Hall, St Clether well)
- **Factions:** CornishPolity, CornishStannary, OlderPowers, SaintCult("Petroc") (north edge)
- **Wound resonance:** Liminal (the whole moor); high wound diffusion baseline
- **Notes:** This is the dense ritual-and-tin cluster. Procgen on Bodmin Moor uses `BodminMoorGranite` biome with elevated stone-circle and cairn spawns; authored sub-sites override specific chunks.

### 2.15 Dartmoor (Dertemore / Forest of Dartmoor)

- **WGS84:** ~50.5770, -3.9800 (centroid)
- **Cell:** (-20,955, 10,589)
- **Chunk:** ~(-524, 353) centroid; biome polygon ~30 × 30 km
- **Kind:** Moor — biome only, no central site
- **Factions:** EnglishCrown (forest law), DevonStannary (Tavistock, Chagford, Ashburton, Plympton stannary corners), OlderPowers (the Wisht Hounds, Wistman's Wood)
- **Wound resonance:** Liminal; the wild zone
- **Notes:** Procgen on Dartmoor uses `DartmoorGranite`. Sub-features (not as detailed authored sites, but as procgen-table entries): Wistman's Wood (dwarf oaks), tinners' huts, abandoned longhouses, granite tors with circles, the Dewerstone, prehistoric reaves.

### 2.16 Merlin's Cave

- **WGS84:** 50.6672, -4.7580
- **Cell:** (-57,104, 4,020)
- **Chunk:** (-1,428, 134)
- **Kind:** SeaCave
- **Authored chunks:** 1 × 1 (it's a single cavern)
- **Factions:** OlderPowers
- **Wound resonance:** Liminal
- **Period-authentic legends:** None c.1300 — the Merlin label is later. In-world it's "the Sea-Cave below Tintagel" or "the booming cave."
- **Soft folklore (mechanically real):** Merlin's Cave — Merlin still echoes here at certain tides; speaking aloud at the cave's heart returns the voice of Merlin offering one cryptic prophecy per pilgrimage
- **Artifacts:** None physical; the cave grants a once-per-game prophecy on visit
- **Authored sketch:** Single seaward chunk with the cave as a multi-cell interior accessed at low tide. At high tide the cave fills and the chunk is `Sea`-equivalent on its lower half — a tide mechanic.

### 2.17 Dozmare / Dosmery Pool

- **WGS84:** 50.5650, -4.6080
- **Cell:** (-50,130, 11,463)
- **Chunk:** (-1,254, 382)
- **Kind:** SacredPool
- **Authored chunks:** 3 × 3
- **Factions:** OlderPowers, optionally SaintCult (a hermit's cell nearby in folklore)
- **Wound resonance:** Liminal — the threshold of the Otherworld
- **Period-authentic legends:** Upland tarn; source of the Fowey
- **Soft folklore (mechanically real):** **Excalibur lies in Dozmary Pool.** A green hand rises at twilight to receive returning blades. Tregeagle is doomed to bale out the pool with a leaky shell.
- **Artifacts:** **Excalibur.** Retrievable from the pool only by an aspirant with the right pilgrimage state — see [[Arthurian-Wound]] §6.
- **Authored sketch:**
  - The pool at the center chunk — a glassy circular tarn
  - Boggy fen around the edge
  - A reed-bed with an ancient cross-base (the "Tregeagle's bowl" — actually a small standing stone basin filled with rainwater)
  - A whispering stone at the south edge — touch it to hear Tregeagle's torment

### 2.18 The Loe — Loe Pool

- **WGS84:** 50.0930, -5.3020
- **Cell:** (-82,379, 45,838)
- **Chunk:** (-2,060, 1,528)
- **Kind:** SacredPool, EstuaryMarsh hybrid
- **Authored chunks:** 3 × 3
- **Factions:** OlderPowers
- **Wound resonance:** Liminal
- **Period-authentic legends:** Natural barred lagoon; bar of pebbles separating it from the sea
- **Soft folklore (mechanically real):** Alternate Excalibur-return site (some hold it is here, not Dozmary). Tregeagle's labor continues here at intervals. Wreckers' ghosts on the shingle bar.
- **Artifacts:** The Mouldon stone (a coastal standing stone said to bleed at sunset on Lammas)
- **Authored sketch:**
  - Pool at the center
  - The shingle bar at the south edge separating pool from sea — passable on foot
  - Marsh and reed flanking the pool
  - A drowned village's church bell heard at storm

### 2.19 The Hurlers (stone circles)

- **WGS84:** 50.5162, -4.4587
- **Cell:** (-43,196, 15,016)
- **Chunk:** (-1,080, 500)
- **Kind:** StoneCircle (triple circle group)
- **Authored chunks:** 3 × 3
- **Factions:** OlderPowers
- **Wound resonance:** Sacred (older powers) — emits *anti-wound* if the rite is performed correctly; Liminal at midnight
- **Period-authentic legends:** Bronze Age stone circles — *present and visible* c.1300 but not Arthurian-labeled. Local Cornish would call them an dans-meyn ("the dancing stones")
- **Soft folklore (mechanically real):** Men hurling on a Sunday were turned to stone — *they are still inside the stones*. At certain lunar nights they briefly become men again (Wound spawns).
- **Artifacts:** None permanent; offering of *salt* at the central pillar quiets the stones for a season (wound resonance pulse)
- **Authored sketch:**
  - Three stone circles in a line, north-to-south
  - The Pipers (two outlying stones) to the west — also petrified men
  - A flat altar-stone at the central circle
  - Visible from a distance — high moor, no woods

### 2.20 Rillaton Barrow

- **WGS84:** 50.5211, -4.4557
- **Cell:** (-43,057, 14,660)
- **Chunk:** (-1,077, 488)
- **Kind:** Barrow
- **Authored chunks:** 1 × 1
- **Factions:** OlderPowers
- **Wound resonance:** Liminal
- **Period-authentic legends:** Largest round cairn on Bodmin Moor — visible since prehistory, ancient by 1300
- **Soft folklore (mechanically real):** Rillaton Cup — a buried gold cup in the chamber. The barrow's resident shade tests entrants; if found worthy, the cup is given. If not, the entrant emerges senseless.
- **Artifacts:** The Rillaton Cup — once-per-game artifact; carrying it grants a daily "shade's favor" boon
- **Authored sketch:** Single chunk with the mound visible from outside; a stone-lined chamber inside reached by descending the side.

### 2.21 Mên-an-Tol (the holed stone)

- **WGS84:** 50.1440, -5.6240
- **Cell:** (-97,340, 42,126)
- **Chunk:** (-2,434, 1,404)
- **Kind:** StoneCircle (atypical — the holed stone group)
- **Authored chunks:** 3 × 3
- **Factions:** OlderPowers, SaintCult("Madern") (peripherally — Madron is nearby)
- **Wound resonance:** Sacred
- **Period-authentic legends:** Late-prehistoric monument; long used as a healing/threshold site by local people
- **Soft folklore (mechanically real):** Passing through the holed stone heals certain afflictions (chronic disease, infertility, rickets), once per affliction per pilgrim. The stone watches.
- **Artifacts:** None; the rite itself is the boon
- **Authored sketch:**
  - The holed stone at center, flanked by two uprights
  - Cropped turf, gorse hummocks
  - Penwith ridge visible to the west

### 2.22 The Merry Maidens

- **WGS84:** 50.0830, -5.6350
- **Cell:** (-97,851, 46,567)
- **Chunk:** (-2,447, 1,552)
- **Kind:** StoneCircle
- **Authored chunks:** 3 × 3
- **Factions:** OlderPowers
- **Wound resonance:** Sacred (when respected); Liminal (when disturbed)
- **Period-authentic legends:** Late-prehistoric circle; visible since prehistory
- **Soft folklore (mechanically real):** Nineteen maidens danced on the Sabbath and were turned to stone. They wake on Midsummer Night and dance again — Wound spawns or, if the player has high `older_powers_devotion`, a transient benediction
- **Artifacts:** None permanent
- **Authored sketch:** Single circle of 19 stones, ground heavily cropped by grazing; the Pipers (two outlying stones — the musicians) visible to the northeast.

### 2.23 Madron Well — Well of St Madern

- **WGS84:** 50.1397, -5.5764
- **Cell:** (-95,129, 42,440)
- **Chunk:** (-2,379, 1,415)
- **Kind:** HolyWell
- **Authored chunks:** 1 × 1 (well + chapel)
- **Factions:** SaintCult("Madern"), CornishPolity, OlderPowers (syncretic — Madern is *Christianized* but the well is pre-Christian)
- **Wound resonance:** Sacred
- **Period-authentic legends:** Surviving Cornish saint cult; one of the densest Cornish well-sites
- **Soft folklore (mechanically real):** Drinking from the well during the right rite cures any single *disease* (not injury). Leaving a rag tied to a nearby thorn relieves a single curse.
- **Artifacts:** The Madern Cup (a tin offering bowl always at the well)
- **Authored sketch:** A spring rising into a basin, with a small chapel of St Madern beside it (one room, an altar, a relic). Trees with rag offerings. A path to nearby Mên-an-Tol (3.4 km in real terms ≈ 2,230 cells ≈ 56 chunks).

### 2.24 St Clether Well

- **WGS84:** 50.6410, -4.5930
- **Cell:** (-49,437, 5,928)
- **Chunk:** (-1,236, 197)
- **Kind:** HolyWell
- **Authored chunks:** 1 × 1
- **Factions:** SaintCult("Clether"), CornishPolity
- **Wound resonance:** Sacred
- **Period-authentic legends:** Secluded medieval well-chapel in the Inny valley
- **Soft folklore (mechanically real):** Clether's well grants rest — sleeping at the chapel restores full Sleep need and one Warmth point overnight regardless of weather
- **Artifacts:** None; the chapel itself is the boon
- **Authored sketch:** A small stone chapel with a spring rising through its altar floor. The water exits via a channel under the altar to a holding pool outside. A hazel grove around it.

### 2.25 King Arthur's Hall

- **WGS84:** 50.5900, -4.6880
- **Cell:** (-53,851, 9,643)
- **Chunk:** (-1,347, 321)
- **Kind:** RuinPrecinct (Neolithic enclosure)
- **Authored chunks:** 1 × 1
- **Factions:** OlderPowers
- **Wound resonance:** Liminal
- **Period-authentic legends:** *No Arthurian name in 1300* — to medieval Cornish it is simply an old earthwork on the moor. In-game it should be locally known as something like *plas an gawr*, "the giant's hall."
- **Soft folklore (mechanically real):** Despite the modern name being later, in our world the site is *actually* Arthurian — Arthur held a council here, and the hall remembers. Sleeping inside at midwinter grants a vision of the Once and Future King.
- **Artifacts:** None physical; the vision is the boon and a Grail-arc hint
- **Authored sketch:** A rectangular enclosure of standing stones on the open moor, ankle-deep in water in winter, dry in summer. A central stone slab at the north end.

### 2.26 Trevillet kieve / later St Nectan's Kieve

- **WGS84:** 50.6800, -4.6900
- **Cell:** (-53,943, 3,088)
- **Chunk:** (-1,349, 102)
- **Kind:** SacredPool, HolyWell (waterfall basin)
- **Authored chunks:** 1 × 1
- **Factions:** SaintCult("Nectan") (later attribution; in 1300 it would be a *local water-spirit* shrine if anything)
- **Wound resonance:** Liminal
- **Period-authentic legends:** The waterfall and basin are *physically present* but the saintly attribution is later. In 1300 it's known as "the trevillet kieve" or simply "the basin."
- **Soft folklore (mechanically real):** Bathing in the kieve cleanses ritual pollution (e.g. recovers from a profane curse) once per year.
- **Authored sketch:** A waterfall plunging through a hole in a rock platform into a deep basin; an approach path along the gorge.

### 2.27 Helston

- **WGS84:** 50.1020, -5.2780
- **Cell:** (-81,264, 45,183)
- **Chunk:** (-2,032, 1,506)
- **Kind:** MinorBorough
- **Population c.1300:** ~800–1,200
- **Authored chunks:** 5 × 5
- **Factions:** CornishPolity, CornishStannary (coinage town later)
- **Wound resonance:** Neutral
- **Period-authentic legends:** Chartered borough; west-Cornwall market center
- **Soft folklore (mechanically real):** The Hal-an-Tow processional path (the May procession leaves Helston into the wild and returns) — performing the procession on May Day boosts older_powers_devotion *and* christian_devotion (rare syncretic event)
- **Authored sketch:** Market street running E-W down a slope; church of St Michael at the upper end; mill at the lower end; the broad street wide enough for cattle drives.

### 2.28 Liskeard

- **WGS84:** 50.4520, -4.4640
- **Cell:** (-43,443, 19,694)
- **Chunk:** (-1,087, 656)
- **Kind:** MinorBorough
- **Population c.1300:** ~800–1,300
- **Authored chunks:** 3 × 3
- **Factions:** CornishPolity (east Cornish edge), CornishStannary (coinage town from 1307, so just after our date)
- **Wound resonance:** Neutral
- **Period-authentic legends:** Borough with courts; east-Cornish market
- **Soft folklore:** None major
- **Authored sketch:** Castle remnants on a north hill; church of St Martin; market square; the well of St Martin (a minor holy well in town).

### 2.29 Dartmouth

- **WGS84:** 50.3500, -3.5800
- **Cell:** (-2,370, 27,122)
- **Chunk:** (-60, 904)
- **Kind:** PortHarbor, MinorBorough
- **Population c.1300:** ~1,000–1,800
- **Authored chunks:** 5 × 5
- **Factions:** EnglishCrown
- **Wound resonance:** Neutral
- **Period-authentic legends:** Major Atlantic carrying port — Gascon wine, pilgrims to Compostela
- **Soft folklore:** Sea-glow at Start Point (the lighting of the dead sailors) — a Wound manifestation off the coast on storm nights
- **Authored sketch:** Tight cluster of quays on the Dart, customs house, a sloping town climbing the hillside, the chain across the harbor mouth (later — partial in 1300). Boat-travel hub for southern coast.

### 2.30 Land's End and Lizard Point (boundary anchors)

- **Land's End:** WGS84 50.0670, -5.7146 → cell (-101,541, 47,731), chunk (-2,539, 1,591). `CoastCliff` biome. Atlantic west boundary. Wreckers' coast.
- **Lizard Point:** WGS84 49.9590, -5.2070 → cell (-77,965, 55,600), chunk (-1,950, 1,853). Southernmost point. Serpentine outcrops; reputed serpent encounter rare.

Treat these as **biome control points** for the procgen, not as authored sites. They give the player a "you have reached the end of the land" experience without a town to enter.

---

## 3. Biome polygons (recommended vertex sets)

These are *suggested* polygon vertex lists for `assets/biomes.json`. Coordinates are in **cell space** (relative to Exeter origin). Final tuning is an authoring task; the values below are a starting layout.

### 3.1 Peninsula outline (inside = land; outside = Sea)

A single polygon following the c.1300 coastline. Critical control points (cell coords):

- Exmouth: (4,650, 6,500)
- Sidmouth: (15,500, 14,000)
- Lyme Regis (east boundary): (32,000, 12,000)
- Hartland Point (north Devon): (-26,500, -22,500)
- Bristol Channel near Lynton (Exmoor coast): (-15,000, -34,500)
- Ilfracombe: (-23,000, -27,000)
- Barnstaple bay: (-30,000, -22,000)
- North Cornish cliffs (Boscastle area): (-55,000, 1,500)
- Tintagel headland: (-57,200, 4,000)
- Padstow / Camel estuary: (-65,000, 7,000)
- St Ives bay: (-92,000, 38,500)
- Land's End: (-101,541, 47,731)
- Penzance: (-90,000, 45,500)
- Lizard Point: (-77,965, 55,600)
- Helston / Helford: (-78,000, 47,000)
- Falmouth (Carrick Roads mouth): (-68,000, 43,000)
- Fowey port: (-50,000, 28,500)
- Plymouth Sound: (-28,000, 28,000)
- Salcombe estuary: (-15,000, 26,500)
- Dartmouth (Dart mouth): (-2,500, 27,500)
- Teignmouth: (1,500, 13,500)

Close back to Exmouth. (~22 control points — enough to read as a recognizable peninsula.)

### 3.2 Dartmoor polygon (centroid -20,955, 10,589)

Bounding control points: (-29,000, 3,000), (-13,500, 4,500), (-9,500, 12,000), (-13,500, 18,500), (-25,000, 18,500), (-30,000, 14,000). Roughly an oval ~30 km × 30 km.

### 3.3 Bodmin Moor polygon (centroid -51,527, 9,206)

Bounding control points: (-59,000, 4,500), (-44,000, 4,000), (-41,500, 11,000), (-46,500, 15,500), (-56,000, 13,500), (-60,500, 9,000). Roughly an oval ~20 km × 15 km.

### 3.4 Exmoor polygon (centroid -10,454, -31,360)

Bounding control points: (-22,000, -27,000), (-2,000, -27,000), (1,500, -33,000), (-2,500, -36,500), (-19,500, -36,500), (-22,000, -32,500).

### 3.5 Tamar valley corridor (river polygon)

Trace down from north (Bristol Channel near Marsland Mouth, ~(-25,000, -25,000)) southward through the Tamar source country, along the Tamar valley, to Plymouth Sound (~(-28,000, 27,500)). Width: 2 km (~1,300 cells) on each side. The corridor enforces `RiverValley` biome regardless of what else the per-chunk biome would have been.

### 3.6 Penwith ritual zone (Land's End peninsula)

A bounded polygon containing Madron, Mên-an-Tol, Merry Maidens, and Land's End. Roughly: (-101,541, 47,731), (-89,000, 39,000), (-85,000, 47,000), (-95,000, 50,000), (-101,541, 47,731). Inside this polygon, procgen rolls stone-circle / cairn / holy-well sub-sites at elevated rates.

---

## 4. Mythic-real artifact list

Each artifact is a `unique` item ([[Cornwall-Pilgrim]] §2.2 lists how unique items already work in `src/items.rs`). Add an `Artifact` item variant with a `tag: ArtifactId` and a `bound_to: Option<SiteId>` (artifacts can be returned to or recovered from their site).

| Artifact | At site | Effect | Acquisition gate |
|---|---|---|---|
| **Excalibur** | Dozmary Pool (or Loe Pool, alt) | +50% damage; cleaves through 1 demon per swing; cannot be sold; if removed from Cornwall, slowly dulls | Player must have completed at least 3 sacred rites and *not* desecrated any site. The green hand offers the sword. |
| **Rillaton Cup** | Rillaton Barrow | "Shade's favor" — 1/day, halve a single source of injury | Enter the barrow; pass the shade's test (a riddle drawn from a fixed list weighted by player's affinities) |
| **The Madern Cup** | Madron Well | Drinking from the cup cures any one disease per drink; refills automatically at the well | Make an offering of any worked-tin item (stannary economy hook); the cup is then yours to carry, but if you leave the well's chunk it returns to the basin |
| **Tintagel Bedchamber Stone** (vision shard) | Tintagel inner chamber | Touching gives a vision of Arthur's conception; granted once per game; reveals a hidden Grail-arc clue | None — just visit |
| **Hurlers Salt Bowl** | The Hurlers, central altar | Pouring salt in the bowl quiets the petrified men for a season; failed salt offering wakes them | Carry a `Salt` item (the seasoning system already has Salt reserved per `src/crafting.rs`) |
| **Mên-an-Tol passage** | Mên-an-Tol | The rite (passing through the stone three times sunwise) cures one chronic affliction per affliction per pilgrim | Player must approach in daylight, alone, with no iron carried (drop weapons outside) |
| **The Grail** | Hidden — placed procedurally per save, with breadcrumbs scattered across sacred sites | Bringing the Grail to the Wounded King heals the Land — permanent reduction in `wound_intensity_cap` across the world | Quest chain involving the Bedchamber Stone vision, three saint rites, and the Wounded King's audience |
| **Tregeagle's Leaky Shell** | Dozmary Pool | A cursed item — if carried, the player can never permanently store water; their waterskin always leaks 50% overnight. But each midnight an audible cry advances the player's `older_powers_devotion` by a tiny amount. | Refuse to help Tregeagle when asked (a quest hook) |

---

## 5. Authored chunk format

Two formats are supported, mixed and matched per chunk.

### 5.1 Layout file format (`.layout.txt`)

A 40-wide × 30-tall grid of CP437 characters representing terrain. A trailing **legend block** maps characters to `TerrainKind` and optional `ItemSpec` / `InteractableSpec`. Example:

```
# Tintagel mainland gate chunk
# 40x30
# ----
########################################
#......##............##...............##
#..A...##...        .##....@..........##
#......##...                  ..........
.       .....                  ..........
. cell  layout                 ..........
. (40w  x 30h)                 ..........
########################################

# Legend
. = Grass
# = Wall
A = Altar(stone)
@ = NamedSiteAnchor(tintagel_castle)
```

The parser is in `src/authored/layout.rs` (proposed). It reads the file, validates dimensions, parses the legend, and produces a `Chunk`. Layout files are stored in `assets/authored/<site_id>/<chunk_dx>_<chunk_dy>.layout.txt` keyed by chunk offset from the site's center chunk.

### 5.2 Factory-function format

For chunks with too much interactive complexity to express in a layout file (e.g. the Tintagel inner chamber with the Bedchamber Stone, the cathedral chancel at Exeter), we author them in Rust:

```
pub fn author_tintagel_inner_chamber(chunk: &mut Chunk, ctx: &AuthorCtx) {
    // base layout loaded from disk
    chunk.load_layout("tintagel/inner_chamber.layout.txt", ctx);
    // place special interactables that the layout legend can't express
    chunk.place_interactable(13, 14, Interactable::BedchamberStone);
    chunk.place_interactable(20, 14, Interactable::Door {
        locked_by: Lock::WoundedKingArc,
        leads_to: SubLocation::WoundedKingChamber,
    });
}
```

### 5.3 Dispatch from chunkgen

In `chunkgen::generate_chunk(coord, world_seed)`:

```
fn generate_chunk(coord: ChunkCoord, world_seed: u64) -> Chunk {
    if let Some(site) = sites::owns_chunk(coord) {
        return authored::generate_for_site(site, coord);   // dispatch to layout file or factory fn
    }
    let biome = biome_at(coord);
    procgen::generate_biome_chunk(biome, coord, world_seed)
}
```

`sites::owns_chunk(coord)` returns `Some(&'static NamedSite)` if `coord` falls inside the site's authored region (using `chunk` ± `authored_chunks / 2`).

### 5.4 Authoring workflow

1. Edit a `.layout.txt` file in `assets/authored/<site>/` (no recompile needed).
2. For special interactables, edit the factory function in `src/authored/<site>.rs` (recompile).
3. Run `cargo run --release`, fast-travel to the site, walk around.
4. Iterate.

This split optimizes the *common* authoring tasks (changing tile layouts, repositioning buildings) for fast iteration while keeping the *rare* tasks (placing the Wounded King) in code where they can be type-checked.

---

## 6. Sub-sites within biome regions (procgen anchors)

Beyond the 24+ named sites, the wilderness should feel populated. Procgen rolls minor sites at low probability per chunk based on biome:

| Biome | Sub-sites that may roll (uncommon → rare) |
|---|---|
| `LowlandFarm` | Wayside cross, abandoned manor, water-mill, charcoal kiln, dovecote, manorial pound |
| `RiverValley` | Ford, fishing weir, willow-osier bed, hermit's cell, tinker's camp |
| `EstuaryMarsh` | Salt pan, eel trap, smugglers' shack, drowned chapel (Wound-resonant) |
| `CoastCliff` | Watch beacon, wreckers' lair, cliff hermitage, stone cross |
| `CoastBeach` | Driftwood pile, fisherman's hut, shrine to drowned saints |
| `OakWoodland` | Charcoal kiln, outlaw camp, deer trap, hunted-out wolf den, sacred oak |
| `BeechCombe` | Hazel coppice, hedger's hut, witch's cottage, pig-pannage clearing |
| `DartmoorGranite` | Tinner's hut, abandoned longhouse, stone row, kistvaen (cist-grave), pony enclosure |
| `BodminMoorGranite` | Smaller stone circles (3–5 stones), tinners' streamworks, isolated cross, druid grove |
| `ExmoorHeath` | Forester's hut, deerstand, royal kennel, ancient track |

These sub-sites are *unnamed* in the gazetteer but help the wilderness feel inhabited and full of small discoveries.

---

## 7. Process notes

- All coordinates here are derived from the report's gazetteer; if a coordinate in the report is corrected, the cell/chunk values must be recomputed via the `wgs84_to_cell` formula in [[Cornwall-World]] §2.2.
- The "mythic-real" content respects the user's choice that *everything mythic is true in-world*. Sites where the modern label is anachronistic (King Arthur's Hall, Merlin's Cave, St Nectan's Kieve) keep their c.1300-plausible local names for in-fiction NPCs, but their mythic content is mechanically active.
- The 24+ sites here are not exhaustive. The report mentions additional control points (Plympton Priory, Wistman's Wood, St Michael's Mount) that we'll add as second-pass authored sites when first-pass is implemented.
