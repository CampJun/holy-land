# Cornwall & Devon c.1300 — World Architecture, Overmap, and Travel

**Status:** Draft 1 — design reference, not implementation spec.
**Source of historical content:** `/home/campjun/Downloads/deep-research-report.md` (Cornwall and Devon Around 1300).
**Related drafts:** [[Cornwall-Sites]], [[Arthurian-Wound]], [[Cornwall-Pilgrim]].

---

## 1. Foundational scale

The Holy Land overworld is a **single seamless world** with an **overmap UI** layered over it. The world is anchored on the real geography of c.1300 Cornwall and Devon. Named historical sites sit at coordinates derived from the GIS gazetteer in the research report; the wilderness between them is procgen.

### 1.1 Cell and chunk dimensions

| Unit | Size | Notes |
|---|---|---|
| 1 cell | **5 ft = 1.524 m** | CDDA-style ground scale. |
| 1 chunk | 40 × 30 cells = **200 ft × 150 ft ≈ 61 m × 46 m** | Existing engine constant (`CHUNK_W = 40`, `CHUNK_H = 30` in `src/world.rs:25–26`). Not changing. |
| 1 overmap cell | 1 chunk | The overmap downsamples chunk → 1 char. Each overmap cell ≈ 60 m wide. |
| Peninsula bounding box | ~131,000 × 52,500 cells = **~3,275 × 1,750 chunks** | ~5.7 M *potential* chunks; lazy generation means only the 3×3 ring is ever in memory. |

### 1.2 Travel time per cell

The existing game uses a 5-second action cost for a single step. We keep that for **manual exploration walking** — it represents survival-pace movement (scanning, watching, carrying gear, ready for danger). For **fast-travel queued movement**, we use realistic walking pace.

| Mode | Time per cell | Effective speed | Comment |
|---|---|---|---|
| Manual step (cautious survival pace) | 5 s | ~0.3 m/s = ~1.1 km/h | Existing engine cost. Don't change. |
| Fast-travel on road | 1.2 s | ~1.27 m/s = ~4.6 km/h | Roman/medieval road remnants + maintained borough roads. |
| Fast-travel on open ground (lowland, beach) | 2 s | ~0.76 m/s = ~2.7 km/h | Sheep tracks, field margins, droveways. |
| Fast-travel on moor / forest / cliff / marsh | 4 s | ~0.38 m/s = ~1.4 km/h | Slow because of broken ground, the report's "× 1.6 moorland" terrain modifier. |
| Boat coastal hop (port → port) | abstracted as event | varies | Separate fast-travel mode; see §6. |

### 1.3 Intersite distance check

Using the report's distance matrix (§Distances and editor import notes), with cell size 1.524 m:

| Route | Real km | Cells (∆) | Chunks (∆) | Fast-travel hours (mixed) |
|---|---:|---:|---:|---:|
| Exeter → Totnes | 34.2 | 22,440 | 561 | ~9 |
| Exeter → Tintagel | 86.9 | 57,020 | 1,425 | ~22 |
| Launceston → Bodmin | 31.6 | 20,740 | 518 | ~8 |
| Bodmin → Tintagel | 22.0 | 14,435 | 360 | ~5 |
| Exeter → Plymouth | 58.3 | 38,255 | 956 | ~14 |
| Plymouth → Truro | 66.2 | 43,438 | 1,085 | ~16 |
| Tintagel → Land's End | ~95 | ~62,335 | 1,558 | ~24 |

**Implication:** a full peninsula traversal (Exeter → Land's End ≈ 175 km ≈ 115,000 cells ≈ 2,875 chunks) is **multiple in-game days of fast-travel** and weeks of manual exploration. The overmap is load-bearing UX, not flavor.

### 1.4 Memory budget

Only 9 chunks (the 3×3 ring) are loaded at a time → ~10,800 live cells. At ~80 bytes per cell (including the `Vec<ItemInstance>` heap header), live memory ≈ ~870 KB. Even on Miyoo Mini Plus this is trivial. Save file accumulates terrain mutations across visited chunks; at 1,000 visited chunks × ~50 mutations × 16 bytes ≈ **~800 KB CBOR**, well within the existing tier-2 save budget.

---

## 2. Coordinate transform — WGS84 to game cells

### 2.1 Origin

Game cell `(0, 0)` is anchored at **Exeter** (50.7224 °N, -3.5290 °E) per the gazetteer. Exeter is the dominant urban node, the cathedral city, and the player's start location — pinning the origin here keeps starting-area coordinates near zero.

### 2.2 Projection (local-tangent-plane approximation)

For a peninsula <250 km wide we can ignore curvature and use a flat projection:

```
EARTH_LAT_KM       = 110.946           // km per degree latitude (mean WGS84)
EARTH_LON_KM_AT(φ) = 111.320 · cos(φ)  // km per degree longitude at latitude φ
CELL_M             = 1.524             // 5 ft, our world unit

// Project (lat, lon) onto game-cell coordinates relative to Exeter
fn wgs84_to_cell(lat: f64, lon: f64) -> (i64, i64) {
    let lat0  = 50.7224;
    let lon0  = -3.5290;
    let lat_km_per_cell = (1000.0 * EARTH_LAT_KM)         / CELL_M; // ~72,800
    let lon_km_per_cell = (1000.0 * EARTH_LON_KM_AT(lat0))/ CELL_M; // ~46,500 at 50.7°N
    let dx = ((lon - lon0) * lon_km_per_cell).round() as i64;
    let dy = ((lat0 - lat) * lat_km_per_cell).round() as i64;
    //          positive dx = east, positive dy = south (screen convention)
    (dx, dy)
}

fn cell_to_chunk(x: i64, y: i64) -> (i32, i32) {
    (x.div_euclid(40) as i32, y.div_euclid(30) as i32)
}
```

This uses the latitude at Exeter for the longitude scale; sites in west Penwith (50.1°N) drift by ~0.5% from "true" cell distances. Within the design tolerance. The formula is centralized in `src/geo.rs` (proposed new module).

### 2.3 Computed coordinates for all named sites

| Site (modern) | lat | lon | cell (x, y) | chunk (cx, cy) |
|---|---:|---:|---:|---:|
| Exeter | 50.7224 | -3.5290 | (0, 0) | (0, 0) |
| Dartmouth | 50.3500 | -3.5800 | (-2,370, 27,122) | (-60, 904) |
| Totnes | 50.4315 | -3.6855 | (-7,272, 21,185) | (-182, 706) |
| Tavistock Abbey | 50.5494 | -4.1448 | (-28,610, 12,600) | (-716, 420) |
| Lydford Castle | 50.6436 | -4.1048 | (-26,754, 5,739) | (-669, 191) |
| Plymouth / Sutton | 50.3650 | -4.1330 | (-28,063, 26,029) | (-702, 867) |
| St Germans Priory | 50.3970 | -4.3095 | (-36,264, 23,698) | (-907, 789) |
| Launceston Castle | 50.6367 | -4.3609 | (-38,653, 6,241) | (-967, 208) |
| Liskeard | 50.4520 | -4.4640 | (-43,443, 19,694) | (-1,087, 656) |
| The Hurlers | 50.5162 | -4.4587 | (-43,196, 15,016) | (-1,080, 500) |
| Rillaton Barrow | 50.5211 | -4.4557 | (-43,057, 14,660) | (-1,077, 488) |
| St Clether Well | 50.6410 | -4.5930 | (-49,437, 5,928) | (-1,236, 197) |
| Dozmary Pool | 50.5650 | -4.6080 | (-50,130, 11,463) | (-1,254, 382) |
| Bodmin Moor (centroid) | 50.5960 | -4.6380 | (-51,527, 9,206) | (-1,289, 306) |
| King Arthur's Hall | 50.5900 | -4.6880 | (-53,851, 9,643) | (-1,347, 321) |
| St Nectan's Kieve | 50.6800 | -4.6900 | (-53,943, 3,088) | (-1,349, 102) |
| Lostwithiel | 50.4078 | -4.6664 | (-52,844, 22,912) | (-1,322, 763) |
| Restormel Castle | 50.4466 | -4.6685 | (-52,940, 20,089) | (-1,324, 669) |
| Bodmin | 50.4716 | -4.7245 | (-55,546, 18,266) | (-1,389, 608) |
| Tintagel Castle | 50.6680 | -4.7599 | (-57,190, 3,962) | (-1,430, 132) |
| Merlin's Cave | 50.6672 | -4.7580 | (-57,104, 4,020) | (-1,428, 134) |
| Truro | 50.2632 | -5.0510 | (-70,713, 33,442) | (-1,768, 1,114) |
| Helston | 50.1020 | -5.2780 | (-81,264, 45,183) | (-2,032, 1,506) |
| Loe Pool | 50.0930 | -5.3020 | (-82,379, 45,838) | (-2,060, 1,528) |
| Madron Well | 50.1397 | -5.5764 | (-95,129, 42,440) | (-2,379, 1,415) |
| Mên-an-Tol | 50.1440 | -5.6240 | (-97,340, 42,126) | (-2,434, 1,404) |
| Merry Maidens | 50.0830 | -5.6350 | (-97,851, 46,567) | (-2,447, 1,552) |
| Land's End | 50.0670 | -5.7146 | (-101,541, 47,731) | (-2,539, 1,591) |
| Lizard Point | 49.9590 | -5.2070 | (-77,965, 55,600) | (-1,950, 1,853) |
| Barnstaple | 51.0804 | -4.0580 | (-24,578, -26,073) | (-615, -870) |
| Exmoor (centroid) | 51.1530 | -3.7540 | (-10,454, -31,360) | (-262, -1,046) |
| Dartmoor (centroid) | 50.5770 | -3.9800 | (-20,955, 10,589) | (-524, 353) |
| Tamar (border anchor) | 50.5190 | -4.2220 | (-32,200, 14,816) | (-805, 494) |

> The world extends from roughly chunk (-2,540, -1,050) in the northwest tip (Land's End / Exmoor) to chunk (0, 1,850) in the southeast (Lizard Point). Game bounds: **a strip ~2,540 chunks wide × ~2,900 chunks tall** centered on the peninsula, padded by sea.

---

## 3. The biome layer

Biomes are a second layer of metadata over the chunk grid. Each chunk has a **primary biome ID**; chunkgen uses it to choose tile palettes, foraging tables, spawn tables, and encounter rolls. Biomes are selected by:

1. **Authored biome polygons** for the major real regions (Dartmoor, Bodmin Moor, Exmoor, Tamar valley, south-coast estuary belt, north Cornish cliff coast, Penwith ritual zone, Sea).
2. **Default lowland biome** (`LowlandFarm`) elsewhere on land.
3. **Per-chunk hash variety** within a biome — e.g. a `LowlandFarm` chunk may roll a barn, a copse, a pond, or an open field via a deterministic hash of `(world_seed, chunk_coord)`.

### 3.1 Biome roster

| ID | Region in real world | Tile palette | Forage | Spawn flavor | Sight |
|---|---|---|---|---|---|
| `LowlandFarm` | Lowland Devon, east Cornwall, lowland fringes | Grass, dirt tracks, ploughland, hedgerow, farmstead, livestock | Berry, herb, mushroom, edible greens | Travelers, shire reeves, herders, occasional bandits | Open |
| `RiverValley` | Tamar, Tavy, Tamar, Tamar valley, Fowey, Camel, Lyhner | Long grass, willow, alder, mud bank, ford, fish-stream | Reed, herb, watercress; fishing | Otters, herons, river-fey; bandits at fords | Mixed |
| `EstuaryMarsh` | Tamar mouth, Camel estuary, Erme/Plym estuaries, lower Dart, Helford | Reed, samphire, salt-marsh, mud, tidal channel | Salt-flora, eel, shellfish | Smugglers, ducks, ague (sickness) | Low |
| `CoastCliff` | North Cornwall, north Devon, Penwith cliffs | Sea-pink, gorse, cliff, rock, sea-spray | Sea-pink, rabbits | Wreckers, gulls, smugglers, mermaid (very rare) | Open |
| `CoastBeach` | South-coast coves, Hayle, Whitsand | Sand, dune, driftwood, surf, marram | Driftwood, shells, cuttle | Beachcombers, drowned-men (wound) | Open |
| `OakWoodland` | Wistman's Wood–like, Dart, mid Devon | Oak, hazel, holly, fern, mossy boulder | Acorn, hazel, mushroom, deer, boar | Outlaws (the report's pre-Lydford "wild men"), wolves, wood-spirits | Closed |
| `BeechCombe` | East Devon, lowland Devon hedgerow vales | Beech, hawthorn, hazel, hedgerow | Berry, herb, hazelnut | Foresters, charcoal-burners | Mixed |
| `DartmoorGranite` | Dartmoor proper, forest-law zone, stannary upland | Heather, gorse, granite outcrop, clitter, peat | Sphagnum, heather honey | Tinners, stannary bailiffs, moor-spirits, Wisht Hounds | Open but hazardous |
| `BodminMoorGranite` | Bodmin Moor, granite uplands, ritual cluster | Heather, granite, stream-grass, stone circle, cairn | Similar to Dartmoor | Stone-keepers, the Hurler-men (wound), Beast of Bodmin | Open but hazardous |
| `ExmoorHeath` | Exmoor royal forest | Heather, gorse, boggy hollow, deer-lawn, ash, rowan | Bilberry, deer | Foresters, royal verderers, the Wild Hunt | Open |
| `Sea` | Open ocean, hard impassable in walk mode | Water-deep | None on foot | None | n/a |
| `TownEdge` | Hand-authored ring around each authored town site | Lane, ditch, hedge, market garden, midden | Garden herbs | Townspeople, beggars, watchmen | Mixed |
| `RuinHinterland` | Hand-authored ring around prehistoric sites (Hurlers, M-an-Tol, etc.) | Cropped turf, stones, cleared rings | Sacred herbs | Rite-keepers, prehistoric echoes | Open |
| `BlightedWaste` | Dynamic — any chunk where `wound_intensity ≥ 0.7` | Bleached overlay of base biome; pale grass, sickly trees | Halved or none; some "wrong" forage | Demonic manifestations from the wound | Variable |

### 3.2 Biome polygons (rough boundaries)

> Polygons are stored as cell-coord vertex lists in `assets/biomes.json` (proposed). The chunkgen layer queries `biome_at(chunk_coord) -> BiomeId` by point-in-polygon test (fallback `LowlandFarm` on land; `Sea` outside the peninsula bounding polygon).

Approximate polygon centers and extents:

- **Sea** — anything beyond the peninsula outline (the outline itself is a single polygon traced from coast control points: Land's End → Lizard → Plymouth Sound → Dart estuary → Exeter coast → Exmouth coast → Lyme Regis area east boundary → Bristol Channel north coast → Hartland Point → Land's End again). All "outside" chunks default to `Sea`.
- **DartmoorGranite** — irregular blob ~30 km × 30 km centered around (-20,955, 10,589). Approximate vertex set: ~12 control points around real Dartmoor National Park boundary.
- **BodminMoorGranite** — irregular blob ~20 km × 15 km centered around (-51,527, 9,206). ~8 control points.
- **ExmoorHeath** — irregular blob ~30 km × 15 km centered around (-10,454, -31,360). ~10 control points.
- **Tamar RiverValley** — a 4–8 km wide corridor running south from Bristol Channel (around (-25,000, -25,000)) to Plymouth Sound (-28,000, 27,000). Roughly a polyline with cross-section width.
- **Penwith RuinHinterland** — west of Hayle, roughly (-95,000 to -101,500, 40,000 to 48,000). Surrounds Madron, Mên-an-Tol, Merry Maidens.
- **CoastCliff** (north) and **CoastBeach** (south) — derived from the peninsula outline by a 0.5–1 km inland offset.

Anything inside the peninsula outline that is not in another polygon defaults to `LowlandFarm`.

---

## 4. Authored vs procgen chunks

Two chunk classes:

### 4.1 Authored (named-site) chunks

Each of the ~24 named sites in the gazetteer ([[Cornwall-Sites]]) has a **5×5 chunk authored region** (200 ft × 150 ft × 25 = ~300 m × ~230 m around the anchor cell — about the right scale for a medieval town footprint or a sacred-site precinct). The center chunk is the anchor; the 24 surrounding chunks are also authored to provide a graceful procgen-to-authored seam.

- Exeter is bigger — **9×9 authored chunks** (~550 m × ~415 m), because it's the cathedral city.
- Tintagel is wider on the seaward axis — **5 chunks × 3 chunks** plus a `Sea` chunk seaward.
- Stone circles (Hurlers, Merry Maidens, Mên-an-Tol) are small — **3×3 chunks** is enough.

Authored chunks are stored either as:

- **Hand-coded factory functions** in `src/authored/` — e.g. `pub fn author_exeter_cathedral(out: &mut Chunk) { … }`. Pro: full Rust expressiveness, can place items + interactions. Con: requires recompile to edit content. Best for the small set of cathedral/castle interiors.
- **Layout files** in `assets/authored/<site>/<chunk>.txt` — CP437 grid with a legend. Pro: human-editable, supports rapid iteration, can be authored by non-coders. Con: limited expressiveness, has to be loaded at runtime. Best for towns/sites where the bulk is "tile layout" with a few special interactions.

Recommended split: **most authored chunks use layout files**; **special interactive chunks** (cathedral chancel, Tintagel castle inner ward, Excalibur stone at Dozmary) use factory functions for the special tiles + load a base layout file for the surrounding cells.

### 4.2 Procgen chunks

For chunks outside the authored regions, `chunkgen::generate_chunk(biome, chunk_coord, world_seed)` produces deterministic content. The current `generate_chunk()` already does this for a single forest biome (`src/chunkgen.rs:34–120`); the work for Slice 2 is parameterizing it on `biome` and adding a biome-tile-table dispatch.

Wilderness chunks may also roll into **named-but-not-anchored** sites at low probability — abandoned manor-vills, lost hermitages, wayside crosses, watch beacons. These are biome-flavored, generated from a small template library, and add interest between authored sites.

### 4.3 Authored-chunk seam handling

When an authored chunk borders a procgen chunk, the procgen side honors a 2-cell soft edge: it reads the authored chunk's edge cells via `tile_at()` and biases its own edge tiles to match (e.g. a path that exits an authored chunk continues across the seam as a path). This is implementable inside `generate_chunk` by querying the neighbor before generating one's own edge band.

---

## 5. The overmap UI

### 5.1 Display

The overmap is a full-screen mode toggled with `M` (desktop) or **Select + R-shoulder** (Miyoo gamepad) to avoid conflict with the existing inventory `I`. While open:

- The viewport renders **biome glyphs at 1 char per chunk** centered on the player's chunk.
- Known named sites are pinned as larger highlighted glyphs (e.g. `‡` for cathedrals, `╫` for castles, `○` for stone circles, `~` for holy wells).
- **Fog of war:** chunks not yet seen by the player are rendered as `?` in dark gray. A chunk becomes "seen" when the player has been within 3 chunks of it OR when its named site is *learned-of* via NPC dialog/quest.
- The player's chunk is a flashing `@`.
- A cursor (`+`) moves with the d-pad; the chunk under cursor shows its biome name, known site (if any), and estimated travel time at the bottom of the screen.
- Press `Enter` / `A` to initiate fast-travel to the cursor chunk.
- Press `M` / `B` to close the overmap.

### 5.2 Discovery rules

A chunk is "discovered" (and visible on the overmap) under any of:

1. Player has visited within 3 chunks of it (visited = entered FOV at any point).
2. Player has spoken to an NPC who tells them about it (e.g. "the road west from Exeter passes Tavistock Abbey before the Tamar bridge").
3. Player has read a document (a pilgrim's itinerary, a stannary writ, a saint's vita with route notes) that names it.

This means the overmap *grows* — early game shows only a small lit patch around Exeter; mid-game an entire road corridor is revealed; late game the whole peninsula is on the map.

### 5.3 Overmap glyph mapping

| Biome | Glyph | Color |
|---|---|---|
| `LowlandFarm` | `.` | dim green |
| `RiverValley` | `~` | bright blue |
| `EstuaryMarsh` | `:` | dim blue |
| `CoastCliff` | `▓` | dark gray |
| `CoastBeach` | `,` | yellow |
| `OakWoodland` | `♣` | dark green |
| `BeechCombe` | `♣` | medium green |
| `DartmoorGranite` | `▲` | dim gray |
| `BodminMoorGranite` | `▲` | dim gray |
| `ExmoorHeath` | `≡` | dim purple |
| `Sea` | `≈` | deep blue |
| `TownEdge` | `□` | bright yellow |
| `RuinHinterland` | `╳` | white |
| `BlightedWaste` | `▒` | dark red |
| Site overlay: cathedral / abbey | `‡` | bright white |
| Site overlay: castle / borough | `╫` | bright yellow |
| Site overlay: holy well / shrine | `○` | bright cyan |
| Site overlay: stone circle / cairn | `Ω` | bright magenta |
| Site overlay: sacred pool / sea cave | `◯` | bright cyan |
| Site overlay: ritual ring | `❂` | bright magenta |

CP437 selections drawn from `assets/CP437_MAP.md`.

---

## 6. Fast-travel queue mechanic

This is the player's core long-distance verb. The design constraints from the user:

1. Selecting a destination on the overmap closes the map.
2. The PC then auto-walks one cell at a time in the **local view** along a computed path.
3. The player **sees terrain stream past** and FOV-reveals dangers in real-time.
4. **Any movement keypress interrupts** the queue.

### 6.1 Pathfinding

The path is computed at the **chunk graph** level for the long-haul segment, then expanded to **cell-level** within the start/end chunks. Algorithm:

1. **Chunk-graph A*** from the player's current chunk to the destination chunk. Edge cost = average biome traverse time per cell × 36 (avg cells crossed per chunk for a diagonal-ish path). Roads (where present) reduce cost by 60%. The graph is sparse; only chunks adjacent to known/visited chunks are considered (the player can't path through unknown territory; the path must hug discovered chunks). If the destination is reached, the player is "blazing a trail" via dead reckoning — we permit this but cost is +50% per unknown chunk.
2. **Cell expansion** at each end: A* within the start and destination chunks to and from the chunk's center on the chosen entry/exit edge. Within intermediate chunks the player travels in a straight-ish line; we don't need cell-perfect paths there because the player is moving through procgen content they'll see streaming past.

The whole path produces a `Vec<(i64, i64)>` of cells to visit in order — the **travel queue**.

### 6.2 Tick loop

Each tick of the queue:

1. Pop next cell from the queue.
2. Compute terrain cost for the destination cell's biome (1.2–4 s; see §1.2). Advance game clock by that many seconds. Tick needs proportionally (the existing needs system already accepts arbitrary second-counts).
3. Set the player position to the next cell. Camera scrolls (the existing follow-cam at `src/main.rs:889–907` handles this — fast-travel just emits the same per-tile movement events the manual-walk path emits).
4. Recompute FOV.
5. If FOV reveals any hostile entity, **auto-cancel** the queue (don't ambush the player into walking into a demon).
6. If an exhaustion/critical-need threshold is hit (sleep < 5, hunger < 5, thirst < 5), **auto-cancel** the queue with a flavor message ("You're too exhausted to keep going."). The player is now at this cell, hungry/thirsty/tired, and must deal with it.
7. If any input is pressed, **auto-cancel** the queue.
8. If the queue is empty, fast-travel completes.

### 6.3 Interrupt UX

When fast-travel is interrupted:

- The player stays at the cell where the interrupt fired (no rewind).
- The overmap remembers the *original destination* — pressing `M` then re-confirming the same destination resumes (recomputing a fresh path from the current position).
- The current frame shows a brief "You stop." flash and re-enables normal controls.

### 6.4 Display during travel

While the queue is running, the local view renders normally, with an unobtrusive indicator (top-of-screen): `→ Travelling to Tintagel · 14 hr remaining · press any key to stop`. Time-remaining is updated from the queue length × avg cost.

### 6.5 Implementation sketch (no code yet)

Add to `World` (or a sibling singleton):

```
struct FastTravelQueue {
    cells: VecDeque<(i64, i64)>,
    destination_site_id: Option<SiteId>,
}
```

Add to the main loop in `main.rs`: if the queue is non-empty and no input has been seen this frame, pop one cell, move the player, tick the world by the per-cell cost. Re-render. Otherwise drain the queue and fall through to the normal input handler.

---

## 7. Sea boundary and boat travel

Sea is hard impassable on foot. Ports unlock a **separate fast-travel mode** between coastal sites:

- Player must be in a port chunk (Dartmouth, Plymouth/Sutton, Lostwithiel, Truro, Helston, Barnstaple).
- Talk to the port-master NPC, pay fare (in pennies — economy gameplay).
- Choose a destination port. The boat hop is rendered as a single "abstracted event" — a fade, a brief vignette of the voyage (weather roll, optional storm/pirate encounter), then arrival at the destination port.
- Sea travel is much faster per real-distance (~10–15 cells per minute equivalent) but costs money and carries risk.

The full coastal-shipping economy (Dartmouth's Gascon-wine trade, Truro's tin shipments) hooks into this in [[Cornwall-Pilgrim]] §4.

---

## 8. The Tamar — special border behavior

The Tamar is **the** axis of identity ([[Cornwall-Pilgrim]] §3). Mechanically:

- The river itself is a `RiverValley` corridor running roughly south from chunk (-300, -800) to (-700, 870).
- Crossings are concentrated at **historical bridge-towns** along the Tamar: Launceston-Polson Bridge area, Greystone Bridge, Horsebridge, Calstock ferry, Plymouth-Saltash ferry.
- Each crossing has a **shire-toll** mechanic: an NPC sergeant demands toll, papers, or identification. Failing to pay/explain leads to:
  - At low identity-tension: lose pennies as bribe; pass.
  - At medium tension (player is wanted in the shire): refused passage; must find an unwatched ford (slower; costs more time; some are dangerous).
  - At high tension: ambush/combat.
- The Tamar is also crossable at **unwatched fords** in marsh-mud sections, but the report's "× 1.6 moor/broken-country" multiplier applies *and* there's a `WoundSpawn` chance (Wound manifestations cluster at liminal places).

---

## 9. References to existing code

- `src/world.rs:25–26` — `CHUNK_W=40`, `CHUNK_H=30`. Stays.
- `src/world.rs:145–149` — `ChunkCoord`. Stays.
- `src/world.rs:152–323` — `TerrainKind` + `TerrainDef`. Will be extended (Heather, Gorse, Granite, Reed, Mud, Sand, CliffEdge, RoadDirt, RoadStone, Marsh, BlightedGrass, BlightedTree — see [[Arthurian-Wound]] for full list).
- `src/world.rs:382–413` — `World`. Will gain `biomes_loaded: HashMap<ChunkCoord, BiomeId>`, `wound_intensities: HashMap<ChunkCoord, f32>`, `fast_travel: Option<FastTravelQueue>`, `discovered_chunks: HashSet<ChunkCoord>`.
- `src/world.rs:520–598` — `chunk_coord_for`, `tile_at`, `ensure_chunk_loaded`, `ensure_chunk_ring`. Stay as-is.
- `src/world.rs:629–644` — `try_move_player`. Stays for manual walking; the fast-travel queue will reuse it for its per-cell move.
- `src/main.rs:889–907` — Follow-cam. Stays; fast-travel emits the same per-cell movement events.
- `src/chunkgen.rs:34–120` — `generate_chunk()`. Will be extended to dispatch on biome.
- `src/save.rs` — `RunSave` will gain `wound_intensities`, `discovered_chunks`, `affinities`, `calendar_day`. Schema bump from 1 → 2; migration required.
- `src/geo.rs` (new) — `wgs84_to_cell`, biome polygon table, named-site coord table.
- `src/authored/` (new dir) — factory functions for hand-coded chunks.
- `assets/authored/<site>/` (new) — text-format layout files for the bulk of authored chunks.
- `assets/biomes.json` (new) — biome polygon definitions.

---

## 10. Open questions / parking lot

These don't block draft acceptance but should be resolved before implementation:

- **Per-biome day/night FOV radius.** Currently a flat 20 / 3. Should `DartmoorGranite` extend day FOV (sweeping moor)? Should `OakWoodland` shorten it? Recommend yes; defer to a table.
- **Per-biome temperature curve.** Dartmoor in winter is dangerous; needs system already supports cold. We'll need a per-biome ambient-temperature offset combined with calendar season.
- **Roads.** Where are roads? The report names the corridor Exeter → Okehampton/Lydford → Launceston → Bodmin → Truro. We need to author a road network as a polyline that the chunkgen renders into chunks as `RoadDirt`/`RoadStone` cells. Should be a single asset file (`assets/roads.json`).
- **Pre-1300 sites the player can visit.** Prehistoric sites (Hurlers, Merry Maidens, Mên-an-Tol, King Arthur's Hall, Rillaton Barrow) are *physically present* at c.1300; the question is whether the c.1300 NPCs already know their later folkloric names. The research is careful here. Recommend: in-game they have neutral local names ("the dancing stones", "the holed stone", "the great cairn", "the giant's hall") with the modern names appearing only in the user-facing gazetteer.
- **Cathedral as starting room.** Should the player be able to *re-enter* Exeter Cathedral as an interior, or only walk past its outside? Recommend interior is authored as a single multi-floor structure later.
- **Save bloat from wound diffusion.** Every chunk getting a `wound_intensity` write each in-game day could blow up the save. Recommend storing only **non-zero** wound chunks in the persistent map (sparse storage), and only writing wound at scheduled save points or chunk eviction, not every diffusion tick.
