# Arthurian Wounding of the Land — Supernatural System

**Status:** Draft 1 — design reference, not implementation spec.
**Source of historical content:** `/home/campjun/Downloads/deep-research-report.md`, esp. §Culture, law, identity, and myth and the Tintagel/Arthurian discussion.
**Related drafts:** [[Cornwall-World]], [[Cornwall-Sites]], [[Cornwall-Pilgrim]].

---

## 1. The lore frame

Cornwall and Devon c.1300 — and in particular **Tintagel** — are the epicenter of the Arthurian world. Geoffrey of Monmouth (1136) made it Arthur's conception-site, and by 1300 that is *settled medieval literature*, not soft folklore. Richard of Cornwall built his castle there in the 1230s **because** of the prestige. This is period-authentic.

The Fisher King mythos is also live in c.1300 — the Vulgate Cycle (1210–1230) and the *Queste del Saint Graal* (c.1220–1230) had already crystallized the wounded-king-and-wasted-land into the central Grail tradition. A 1300 pilgrim in Cornwall knows these stories the way a modern person knows *Snow White* or *The Lord of the Rings*.

**The conceit of Holy Land:** all of this is *true*. Arthur was real. The Wounded King is real and lies under Tintagel. The Land is genuinely wounded — sick, sparse, demon-haunted, and the wound spreads. The player's existing Holy Land demon system is reframed not as a separate metaphysics but as **manifestations of the Wound**. The Quest of the Grail is the only thing that ultimately heals it. Saints, pagans, stannary law, English shire administration — these are the worldly orders trying to live atop a wounded substrate.

The player is not the foretold Grail-bearer (necessarily). They are a pilgrim, a tinner, a fisher, a sister — someone in this world who can do small healings, large pilgrimages, and (if they push the long arc) participate in the Grail finding.

---

## 2. The blight model

### 2.1 Per-chunk wound intensity

Every chunk in the world has a scalar `wound_intensity: f32`, range `[0.0, 1.0]`. Default at world generation = `0.0` everywhere except for **corruption nodes** (§3), which start non-zero.

Wound intensity changes via:

- **Diffusion** — each in-game day, blight diffuses from high-intensity chunks to their 8-neighbors at a small rate (e.g. `+0.005 × diff`).
- **Emission** — corruption nodes emit a fixed value into their own chunk each day (resets after diffusion).
- **Healing** — sacred sites emit *negative* intensity into a ring around them (§4).
- **Player rites** — completing a rite at a sacred site applies a one-time large negative pulse.
- **Player desecration** — failing a rite or actively desecrating a sacred site applies a positive pulse.

### 2.2 Non-terminal floor and cap

Per user requirement, the world is **never destroyed by blight**. Mechanically:

- Wound intensity is **clamped to a soft ceiling** (`WORLD_WOUND_CAP`, default 0.95) — no chunk can reach 1.0 by diffusion alone.
- Corruption nodes can briefly push their own chunk to 0.95 but no further.
- The player can **always** push back. There is no game-over from blight saturation.
- The Wound has a **stable equilibrium** even without player intervention — the natural sacred network (holy wells emitting baseline healing) keeps mean intensity bounded. The player accelerates and localizes healing; they don't enable it.

### 2.3 Storage

`wound_intensity` is stored sparsely — only chunks with intensity > 0.05 are persisted. Empty (zero) chunks are not in the map. This keeps save bloat manageable across a peninsula-sized world.

In `RunSave` (schema bump from 1 → 2):

```
struct RunSave {
    // ... existing fields ...
    #[serde(default)]
    wound_intensities: Vec<(i32, i32, f32)>,  // (cx, cy, intensity)
    #[serde(default)]
    corruption_nodes: Vec<CorruptionNodeState>,
    #[serde(default)]
    sacred_node_states: Vec<SacredNodeState>,
}
```

### 2.4 Diffusion tick

Diffusion runs once per **in-game noon** (a wall-clock proxy for "once per day"). The algorithm:

```
fn diffuse_wound(world) {
    let mut delta = HashMap::new();
    for (coord, intensity) in &world.wound_intensities {
        if *intensity < 0.05 { continue; }
        for neighbor in coord.neighbors_8() {
            let n_intensity = world.wound_intensities.get(&neighbor).copied().unwrap_or(0.0);
            let diff = intensity - n_intensity;
            if diff > 0.0 {
                let transfer = (diff * DIFFUSION_RATE).min(0.05);  // never more than 5% per day
                delta.entry(neighbor).or_insert(0.0) += transfer;
                delta.entry(*coord).or_insert(0.0) -= transfer;
            }
        }
    }
    apply(delta);
    apply_corruption_emissions();
    apply_sacred_healing();
    clamp_all_to_cap();
}
```

Only chunks within ~10 chunks of the player (~600 m radius) need *visible* diffusion updates each tick; distant chunks can batch their diffusion lazily when the player approaches them (lazy-evaluation: when a chunk is loaded, replay N-day-equivalent diffusion on it).

---

## 3. Corruption nodes

A corruption node is a fixed chunk-coord that emits blight at a fixed rate per day. Each named site in [[Cornwall-Sites]] with `WoundResonance::Profane` or `::WoundEpicenter` is a corruption node. Additional procgen corruption nodes may roll on Bodmin Moor, Dartmoor, and the BlightedWaste biome.

### 3.1 Node types

| Node | Site reference | Emission rate (/day) | Quench condition |
|---|---|---|---|
| **The Fisher King's wound** | Tintagel (`WoundEpicenter`) | `+0.02` | Only the **Grail arc** quenches this. Permanent reduction once the Grail is brought to Tintagel. |
| **Haunted barrow** | Procgen on moors; one or more per Dartmoor / Bodmin Moor | `+0.01` | Slay the resident shade in single combat, or perform a barrow-quieting rite (offering of milk and salt) |
| **Desecrated chapel** | Procgen along the Tamar valley (places where churches were burnt in past raids) | `+0.005` | Have a priest re-consecrate the site; the player can guide a priest from Exeter or any priory |
| **Wreckers' shore** | Procgen on `CoastCliff` chunks | `+0.005` | Find the buried dead and give them Christian burial |
| **Drowned village** | Procgen at one site only, near Loe Pool | `+0.015` | Listen to the bell at full storm and ring the harbor bell in response |
| **Tregeagle's labor** | Dozmary Pool *and* Loe Pool | `+0.005` (low but constant) | Cannot be permanently quenched — but assisting Tregeagle's labor briefly halts emission for 7 days |

### 3.2 Visible signs at the chunk

- Pale tinted overlay on grass tiles (RGB shift toward yellow-gray)
- Sickly tinted overlay on trees
- Birds and game scarcer (fewer animal spawns from foraging tables)
- A persistent low hum (audio cue once audio is implemented)
- Dried/blocked wells (`water_quality` flag — a `StreamWater` at high local wound becomes undrinkable; consuming it causes `Sickness`)

### 3.3 Demonic spawns

Wound-driven demon spawns are gated on local intensity:

- `intensity < 0.2` — no demonic spawns
- `intensity ≥ 0.3` — rare nighttime spawns (1 per night, low CR)
- `intensity ≥ 0.5` — moderate (1–3 per night, mid CR; daytime appearances near dusk)
- `intensity ≥ 0.7` — biome flips to **BlightedWaste**; high spawn rates; foraging blanked or returns blighted/wrong items; needs decay accelerated

This *reuses* the existing demon spawn logic. The spawn-rate scalar already exists as a per-tile field; we just gate it on `wound_intensity` at the chunk level instead of using a flat global rate.

---

## 4. The sacred network

The opposing force. Sacred sites emit *negative* intensity into their surrounding chunks each day. These nodes are:

- All [[Cornwall-Sites]] with `WoundResonance::Sacred` — cathedrals, abbeys, priories, churchtowns, holy wells
- Player-built shrines (a high-tier crafting recipe; long-term)
- Active May processions, Pentecost vigils, saint feast days (event-driven; see §6)

### 4.1 Healing ring

Each sacred node emits `-EMISSION × (1 - distance/RING_RADIUS)` into chunks within `RING_RADIUS` chunks. Default `EMISSION = 0.01/day`, `RING_RADIUS = 5` chunks (~300 m). Larger nodes have larger rings:

| Node | Emission | Ring |
|---|---:|---:|
| Exeter Cathedral | -0.025 | 15 |
| Tavistock Abbey | -0.02 | 12 |
| Plympton Priory / St Germans Priory / Bodmin Priory | -0.015 | 10 |
| Churchtown (procgen) | -0.005 | 4 |
| Holy well (Madern, Clether, Germanus, etc.) | -0.01 | 6 |
| Stone circle (Hurlers, Merry Maidens, Mên-an-Tol) | -0.008 | 5 |

Higher-tier nodes (cathedrals, the bigger abbeys) project a sustained sacred field; this is why Exeter and its surroundings feel safer than the deep moors.

### 4.2 Sacred-site state

Each sacred node has runtime state: a `consecration: f32` field (default 1.0). The player can:

- **Tend** the site (clean the well, sweep the chapel) → consecration += 0.01, capped at 1.5 — emission scales linearly
- **Desecrate** the site → consecration plummets to 0.0; emission halts; nearby wound rises

Consecration decays slowly (`-0.001/day`) if not tended — abandoned sites lose power. Major sites with permanent priests/keepers don't decay.

---

## 5. Player rites

A **rite** is an interactable invoked at a sacred site (or at a corruption node — different rites). Rites cost time, items, and affinities; they produce concrete world-state changes.

### 5.1 Rite shape

```
struct Rite {
    site_kind: SiteKind,
    name: &'static str,
    required_items: &'static [ItemKind],
    consumed_items: &'static [ItemKind],
    required_affinity: AffinityRequirement,    // e.g. christian_devotion >= 3
    forbidden_affinity: Option<AffinityRequirement>,  // e.g. older_powers_devotion >= 7
    duration_minutes: u32,
    on_success: RiteEffect,                    // wound pulse + boon + affinity bump
    on_partial: RiteEffect,                    // diminished version
    on_fail: RiteEffect,                       // desecration penalty
}
```

### 5.2 Concrete rite examples

- **The Pilgrim's Drink at Madron Well** — site: Madron Well. Cost: 30 min + offering of a worked-tin item OR a Cornish ballad sung (player needs Lore skill ≥ 1). On success: cure one disease, +1 christian_devotion, +1 cornish_affinity, wound -0.05 in ring.
- **The Stone Circle Vigil at the Hurlers** — site: The Hurlers. Cost: 6 in-game hours (perform from midnight to dawn) + Salt item. On success: dramatic vision, +2 older_powers_devotion, wound -0.10 in ring, "the petrified men dance and are quiet for a season." On fail (no salt or wrong moon): wound spawns, wound +0.05 in ring.
- **Pass through Mên-an-Tol** — site: Mên-an-Tol. Cost: drop weapons outside; alone; daylight. On success: cure one chronic affliction once-per-affliction, +1 older_powers_devotion. (No wound effect — this rite is *personal* healing.)
- **Cathedral Pilgrimage at Exeter** — site: Exeter Cathedral. Cost: 1 day + 6d offering (pennies; Roman numeral VI denarii). On success: +2 christian_devotion, blessing item (consumable: one cancelled hostile encounter when used), wound -0.05 in 15-chunk ring.
- **Saint Petroc's Vow at Bodmin** — site: Bodmin Priory. Cost: 1 day vigil + cease all weapon use for 7 days. On success: +3 christian_devotion, "Petroc's Deer" follows you (a friendly companion stag for a season), wound -0.10 in 10-chunk ring.
- **Tregeagle's Bargain at Dozmary Pool** — site: Dozmary. Cost: 6 hours + accept the Leaky Shell. On success: a one-time wound -0.20 pulse at the pool; you now carry the Leaky Shell (long-term debuff, see [[Cornwall-Sites]] §4).
- **Re-consecration of a desecrated chapel** — site: desecrated chapel (corruption node). Cost: bring a priest from a priory; the priest is an escort NPC. On success: corruption node quenched permanently; ring of healing for 14 days.

### 5.3 Lapsed vs active state

A successful rite at a sacred site keeps it "active" for an in-game year. Active sites give passive bonuses (the surrounding chunks are gently healing). The player can pilgrimage the same site each year on its patron feast day for an accumulating boon ladder (year 1 — basic boon; year 2 — repeat + minor escalation; year 3 — major escalation; year 7 — locked-in major artifact).

---

## 6. The four-axis faith system

Per [[Cornwall-Pilgrim]] §3, identity is a five-axis vector but only **four** of these axes interact with the wound mechanic. The `stannary_standing` axis affects economy and faction but not the wound directly.

| Axis | Range | Bumps from | Affects |
|---|---|---|---|
| `christian_devotion` | 0–10 | Cathedrals, abbeys, priories, mass attendance, almsgiving, hagiographic deeds | Saint-cult rites; reactions of clergy; some demonic creatures are repelled |
| `older_powers_devotion` | 0–10 | Stone circles, barrows, holy wells (some), Dozmary/Loe rites, midnight vigils | Older-powers rites; stone-circle interactions; the older-folk NPCs trust you |
| `english_affinity` | 0–10 | Speaking English in courts, serving the shire, aiding crown business | English shire NPCs friendlier; eastern Devon factions favorable |
| `cornish_affinity` | 0–10 | Speaking Cornish (a learnable skill); serving Cornish saints; aiding Cornish customs | Western Cornwall NPCs friendlier; Cornish saint cults reachable |

### 6.1 Conflicts, not opposites

The user chose "everything is real" / "everything coexists." Axes are **not** zero-sum. A pilgrim can hold high `christian_devotion` *and* high `older_powers_devotion` — many Cornish saints (Madern, Petroc, Nectan) sit on a syncretic boundary, and the holy wells most clearly embody this. Pure-bred zealots in either direction may *refuse* the player some rites at the other end (e.g. a strictly orthodox Augustinian canon at Bodmin won't bless a player who openly venerates the Hurlers), but the player isn't locked out of either path.

Some rites *do* trigger mutual exclusivity. A few are flagged in the rite catalog:

- **The Bishop's Anathema** (at Exeter, optional) — declare the older powers as devils; cap `older_powers_devotion` at 2 thereafter
- **The Older Folk's Oath** (at the Hurlers at midwinter) — vow to forsake the new church; cap `christian_devotion` at 2 thereafter

These are *player-chosen escalations*, not default trajectories.

### 6.2 Affinity-gated content

- Some doors and dialog options open only at certain affinity thresholds.
- Some artifacts (Excalibur, the Madern Cup, the Rillaton Cup) refuse the wrong-handed (e.g. Excalibur lifts only for a pilgrim with `older_powers_devotion ≥ 5` AND `christian_devotion ≥ 5` — syncretic ideal).

---

## 7. The Grail arc — the long quest

The eventual narrative goal of a full playthrough. Not a "win the game" screen — a **permanent meta-state shift** at the world level.

### 7.1 Quest gates (rough sequence)

1. **The Vision at Exeter** — the player at high enough `christian_devotion` (≥ 3) gets a dream at the cathedral hinting at "a wound in the west" and the Bedchamber Stone at Tintagel.
2. **The Bedchamber Stone** — touch the stone at Tintagel. The vision identifies the Wounded King but does not reveal his chamber.
3. **The Three Saint Rites** — complete pilgrimages at three of: Madron, Clether, Germanus, Petroc-at-Bodmin. Each rite unlocks one of the Wounded King's locks.
4. **The Older Powers' Counsel** — speak with the keeper at the Hurlers or pass Mên-an-Tol. The keeper tells you that healing the Land needs both halves — Christian *and* older. (This is why the affinity rule above requires both ≥ 5.)
5. **The Wounded King's Audience** — at Tintagel inner chamber, the door is now unlocked. The king (an old, frail figure on a low bed, surrounded by dust and the scent of brine) tells you what was lost.
6. **The Grail Search** — the Grail's hidden chunk is rolled at world-gen, with breadcrumbs scattered across sacred sites (each saint's rite reveals one crystal-clear directional clue). The Grail itself is in a small authored chunk (a hermit's cell in the deep moor, the exact one rolled per seed).
7. **The Healing** — bring the Grail to Tintagel. The Wounded King drinks. He dies (the legend's truest form), the wound at Tintagel quenches permanently, and the **world's wound cap** drops from 0.95 to ~0.5. Demon spawns globally fall. The Land is *better*, not *cured*.

### 7.2 No game-over

After the Grail arc the world continues. The player has a healed-Land save and can keep playing (other arcs: trade, exploration of Penwith, full pilgrim circuit). This is by design — Holy Land is a survival-sim with a strong story arc, not a story-game with survival mechanics.

---

## 8. Demon reframe — manifestations of the wound

The existing demon system stays in place. We re-tag the demons as **manifestations** rather than agents of a coherent enemy faction.

| Existing demon (Holy Land) | Reframed as | Where it spawns |
|---|---|---|
| (lesser demon) | The Drowned Knight | Liminal sites (fords, river crossings, sea caves) |
| (mid demon) | The Wisht Hound | DartmoorGranite chunks at night, especially near tin streamworks |
| (mid demon) | The Pale Watcher | BlightedWaste chunks; stares the player into terror (a fear stat already exists) |
| (greater demon) | The Wild Hunt | ExmoorHeath at storm-nights, or any high-wound chunk; multi-spawn |
| (greater demon) | The Green Hand | Specifically Dozmary Pool when player approaches with a sword and wrong affinity |
| (unique) | The Wounded King's Pain | Tintagel until the Grail arc resolves; doesn't attack, but its mere presence makes the chunk a corruption node |
| (boss) | Tregeagle's Shade | Loe Pool or Dozmary; aggressive only if the player refused his bargain |

Each manifestation has flavor text and unique behavior. None of them is *gated by killing* alone — they are symptoms. Killing one resets its spawn timer in the chunk for 30 in-game days, but only **quenching the corruption node** ends them.

---

## 9. Sacred-event calendar (feast-day pulses)

The medieval feast calendar amplifies the sacred network on certain days. On a feast day, sacred sites of the relevant cult emit **3× their normal healing** for 24 hours. Player rites on feast days have higher success rates and bigger pulses.

Major feast days (from the Use of Sarum, the dominant medieval English liturgical calendar):

| Date (approx.) | Feast | Amplifies |
|---|---|---|
| 6 Jan | Epiphany | All Christian sacred sites |
| late Feb / early Mar | Ash Wednesday | All cathedrals/abbeys |
| variable, Mar–Apr | Easter | All Christian sacred sites; the Grail arc has special dialog this day |
| 23 Apr | St George | English-affiliated sacred sites |
| 1 May | Beltane / May Day | Older-powers sites (the Hurlers, Merry Maidens, Mên-an-Tol) and Helston (Hal-an-Tow) |
| 4 Jun (variable) | St Petroc | Bodmin Priory |
| 24 Jun | St John the Baptist / Midsummer | All sites; older-powers especially; Merry Maidens dance |
| 1 Aug | Lammas | Lowland farms; harvest; Loe Pool (Mouldon stone) |
| 17 May | St Madern | Madron Well |
| 23 Oct | St Clether | St Clether Well |
| 31 Oct / 1 Nov | All Hallows | All sacred sites; *wound diffusion rate doubled too* — this is a perilous night |
| 25 Dec | Christmas | All Christian sites |
| 21 Dec | Midwinter | Older powers peak; *wound diffusion peaks too* — the Land bleeds most |

The midwinter/Hallowmas amplification is deliberate — these are the dangerous nights when the wound is loudest. The cycle gives the year an emotional shape.

---

## 10. Tin as warded metal

The user pinned this as a thematic anchor. Mechanically:

- Worked tin items (tin cups, tin offerings, tin-tipped arrows) deal +50% damage to wound manifestations.
- Stannary towns sell worked-tin items at reasonable prices.
- Tin-mining is a player verb (long-tail).
- The 1201 charter and 1305 charter (the pending one — *just after* our game date) make tin work a politically protected activity. Tinners' enclaves on Dartmoor and Bodmin Moor are safer than the surrounding biome for tinners, hostile for non-tinners.

This gives Cornwall's economy a genuine mythic-mechanical role rather than being flavor-only. It hooks into [[Cornwall-Pilgrim]] §4 (Trade pillar).

---

## 11. Implementation summary (for later, not now)

### 11.1 New fields in `World` / `RunSave`

```
World {
    // ... existing ...
    wound_intensities: HashMap<ChunkCoord, f32>,    // sparse
    corruption_nodes: HashMap<ChunkCoord, CorruptionNodeState>,
    sacred_node_states: HashMap<SiteId, SacredNodeState>,
    affinities: Affinities,                         // see [[Cornwall-Pilgrim]] §3
    completed_rites: HashMap<SiteId, RiteHistory>,
    grail_state: GrailArcProgress,
}
```

### 11.2 New systems in `src/`

- `src/wound.rs` — wound diffusion tick, corruption node emission, sacred ring computation
- `src/rite.rs` — rite catalog, rite attempt resolution
- `src/affinity.rs` — affinity ladder, gate evaluation
- `src/grail.rs` — grail arc state machine, breadcrumb placement at world-gen
- `src/sites.rs` — already covered in [[Cornwall-Sites]] §1

### 11.3 Save format migration

Schema bump 1 → 2 with `migrate_v1_to_v2` that initializes empty wound state and default affinities. See `src/save.rs` header for the migration pattern.

### 11.4 Touchpoints to existing code

- `src/world.rs` — `try_move_player`: trigger blight-related on-step effects (BlightedWaste accelerated thirst) by querying wound intensity
- `src/main.rs` — main loop: schedule daily diffusion tick at noon
- `src/items.rs` — extend `ItemKind` with `WorkedTin`, `Salt` (already reserved per `crafting.rs`), `GrailVessel`, `Excalibur`, `RillatonCup`, `MadernCup`, `TregeaglesShell`, `BedchamberStoneVision` (a one-shot consumable representing the vision)
- (no change to `src/render.rs` for now; biome tinting is a chunk-level color modulation done in chunkgen)

---

## 12. Open questions

- **Tin smelting verb.** Should the player be able to smelt tin themselves, or only buy worked tin from stannary towns? Tradeoff: more verbs = more depth = more code. Recommend: only buy, for now; smelting is a season-3 feature.
- **Companion NPCs.** Petroc's Deer (from §5.2) implies a companion-following system. We don't have one yet. Recommend: defer companion as a generic feature; first version of the deer rite gives a passive buff instead.
- **Feast-day notification UX.** How does the player know it's a feast day? Recommend: a status-bar entry showing the current liturgical day; calendar accessible from a menu.
- **Save bloat from corruption-node emissions.** Solvable with sparse storage + lazy diffusion replay.
- **The "wrong items" from blighted foraging.** E.g. "Hand of Glory" — a hand-shaped twisted root that, if eaten, has hallucinogenic effects. These need an `ItemKind` of `BlightedForage` with a small subtable of cursed/strange items.
