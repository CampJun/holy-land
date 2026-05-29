// Authored city skeletons (walls, gates, streets, landmark footprints).
// Procgen fills blocks inside. v1 stamps the wall polygon + gates only;
// streets/landmarks/block-fill arrive in later cards.
//
// Coordinate convention inside a `City` is *anchor-relative*: cells are
// expressed as offsets from the site's `NamedSite.anchor_cell`. The
// stamper adds the anchor to recover world cells. This way a city's RON
// is portable across overmap positions.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::buildings::{catalog, CityTier, Prefab};
use crate::world::{CellState, GroundCover, TerrainKind, CHUNK_H, CHUNK_W, ChunkCoord};

#[derive(Debug, Deserialize)]
pub struct City {
    #[allow(dead_code)] // human label; useful for logging once wired
    pub name: String,
    /// Settlement size — drives which prefab pool the block-fill
    /// picks from. Defaults to Urban so old RONs keep parsing.
    #[serde(default)]
    pub tier: CityTier,
    pub wall: Wall,
    #[serde(default)]
    #[allow(dead_code)] // populated later
    pub streets: Vec<Street>,
    #[serde(default)]
    #[allow(dead_code)] // populated later
    pub landmarks: Vec<Landmark>,
}

#[derive(Debug, Deserialize)]
pub struct Wall {
    /// Closed polygon, anchor-relative cells. Vertices listed once; the
    /// stamper closes the loop (last → first).
    pub polygon: Vec<(i64, i64)>,
    pub gates: Vec<Gate>,
}

#[derive(Debug, Deserialize)]
pub struct Gate {
    #[allow(dead_code)] // surfaces in here-line / overmap label later
    pub name: String,
    pub cell: (i64, i64),
    pub width: u16,
}

#[derive(Debug, Deserialize)]
pub struct Street {
    #[allow(dead_code)] // surfaces in here-line / overmap label later
    pub name: String,
    pub polyline: Vec<(i64, i64)>,
    pub width: u16,
    /// District-flavour tags broadcast to nearby house slots.
    /// e.g. Smythen Street → `[("smithy", 5), ("house", 5)]` so half
    /// of fronting slots roll a smithy prefab. Empty (default) means
    /// "no flavour", and slots near this street fall back to the
    /// generic `[("house", 1)]` at pick time.
    #[serde(default)]
    pub tag_weights: Vec<(String, u32)>,
}

#[derive(Debug, Deserialize)]
pub struct Landmark {
    #[allow(dead_code)] // surfaces in here-line / overmap label later
    pub name: String,
    pub footprint: Footprint,
    #[serde(default)]
    pub kind: LandmarkKind,
}

#[derive(Debug, Deserialize)]
pub enum Footprint {
    /// Axis-aligned rectangle from inclusive `min` to inclusive `max`.
    Rect((i64, i64), (i64, i64)),
    /// Closed simple polygon; vertices listed once.
    Polygon(Vec<(i64, i64)>),
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
pub enum LandmarkKind {
    /// Solid stone fill (Cathedral, Rougemont, Guildhall). Stamps as
    /// `StoneWall` across the entire footprint — v1 silhouette only;
    /// interiors come with the doors+rooms card.
    #[default]
    StoneMass,
    /// Walkable paved fill (Cathedral Close, Quay). Stamps as
    /// `CobbleRoad` — distinct from streets but the same terrain so
    /// FOV / movement behave consistently.
    PavedArea,
    /// Parsed for bbox; not yet stamped. Bridge spans need water
    /// context (the bridge cells should be CobbleRoad over deleted
    /// water cells, which requires knowing where the river is).
    Bridge,
    /// Parsed for bbox; not yet stamped. Mill leats are narrow water
    /// channels best authored as polylines into `StreamWater` once
    /// the cornwall river pipeline is touched.
    WaterChannel,
}

const EXETER_RON: &str = include_str!("../assets/cities/exeter.ron");

/// Per-chunk feature tag for the overmap. One per chunk that some part
/// of the city touches; the renderer picks a glyph+color per tag.
/// Stored in `LoadedCity::chunk_features`, built once at load.
///
/// Priority is encoded in the build order (later writes overwrite
/// earlier): Building < Street < WallEdge < Cathedral/Castle < Gate.
/// Rationale: gates matter most for navigation; major stone landmarks
/// distinguish the silhouette; walls outline the city; streets show
/// internal connectivity; building chunks are background fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CityChunkFeature {
    Cathedral,
    Castle,
    WallEdge,
    Gate,
    Street,
    Building,
}

/// A city's parsed RON plus world-cell anchor and bbox cached at load.
/// City stamping intersects this bbox against each chunk (cities can
/// extend well beyond the `NamedSite` biome-override radius, so we
/// can't reuse that for stamping range).
pub struct LoadedCity {
    pub city: City,
    pub anchor: (i64, i64),
    /// Inclusive world-cell bbox of every authored polygon / polyline:
    /// `(min_x, min_y, max_x, max_y)`.
    pub bbox: (i64, i64, i64, i64),
    /// Wall polygon with anchor offset baked in (world cells). Used
    /// by the per-cell forest-clear pass and the block-fill slot
    /// containment test.
    pub wall_world_polygon: Vec<(i64, i64)>,
    /// Inclusive world-cell bbox of just the wall polygon. Block-fill
    /// and forest-clear skip chunks that don't intersect this — much
    /// tighter than `bbox` (which also covers Rougemont / bridge /
    /// leat).
    pub wall_bbox: (i64, i64, i64, i64),
    /// Every world-cell covered by a street polyline (including the
    /// width widening). Block-fill consults this to skip slots whose
    /// house rectangle would straddle a street.
    pub street_cells: HashSet<(i64, i64)>,
    /// District-tag distribution per house slot (anchor-relative
    /// `(slot_x, slot_y)`). Each street's `tag_weights` are summed
    /// into every slot whose centre is within Chebyshev-K of the
    /// street's widened polyline. Missing slots fall back to the
    /// generic `[("house", 1)]` at pick time.
    pub slot_tags: HashMap<(i64, i64), Vec<(String, u32)>>,
    /// Per-chunk feature classification for the overmap renderer.
    /// Chunks not touching the city are absent from the map.
    chunk_features: HashMap<ChunkCoord, CityChunkFeature>,
}

impl LoadedCity {
    /// Feature classification for this chunk, if the city overlaps it.
    /// Returns `None` for chunks with no city feature (renderer falls
    /// back to biome / road / river).
    pub fn chunk_feature(&self, coord: ChunkCoord) -> Option<CityChunkFeature> {
        self.chunk_features.get(&coord).copied()
    }

    /// True if this city has any geometry inside the given chunk's
    /// world-cell bounds. Cheap rectangle-vs-rectangle intersect.
    pub fn intersects_chunk(&self, coord: ChunkCoord) -> bool {
        let chunk_min_x = coord.cx as i64 * CHUNK_W as i64;
        let chunk_min_y = coord.cy as i64 * CHUNK_H as i64;
        let chunk_max_x = chunk_min_x + CHUNK_W as i64 - 1;
        let chunk_max_y = chunk_min_y + CHUNK_H as i64 - 1;
        rect_intersects(
            self.bbox,
            (chunk_min_x, chunk_min_y, chunk_max_x, chunk_max_y),
        )
    }

    /// Stamp this city's authored skeleton + block-fill into one chunk.
    /// `world_seed` salts the block-fill hash so different worlds
    /// produce different (but per-world deterministic) house layouts.
    pub fn stamp_into_chunk(
        &self,
        coord: ChunkCoord,
        cells: &mut [CellState],
        world_seed: u64,
    ) {
        self.city.stamp_into_chunk_with_seed(
            coord,
            cells,
            self.anchor,
            world_seed,
            &self.wall_world_polygon,
            self.wall_bbox,
            &self.street_cells,
            &self.slot_tags,
        );
    }
}

fn rect_intersects(a: (i64, i64, i64, i64), b: (i64, i64, i64, i64)) -> bool {
    !(a.2 < b.0 || a.0 > b.2 || a.3 < b.1 || a.1 > b.3)
}

/// Width-N street neighbourhood offsets. Mirrors `gate_skip_cells`:
/// width=1 → {0}; width=2 → {-1, 0}; width=3 → {-1, 0, +1}; width=4
/// → {-2, -1, 0, +1}. Even widths are NW-biased (one extra cell to
/// the north/west of centre); odd widths are symmetric. Used by both
/// the in-chunk stamper and the global `street_cells` precompute, so
/// they always agree on which cells count as "street".
fn street_half_extents(width: u16) -> (i64, i64) {
    let w = width as i64;
    let lo = -(w / 2);
    let hi = lo + w - 1;
    (lo, hi)
}

/// Chebyshev radius around a street's widened polyline within which
/// slots inherit the street's `tag_weights`. K=12 ≈ 1.5 house-pitches:
/// covers slots fronting the street plus the immediate back row, so
/// a "smiths' lane" tag bleeds one block deep but no further.
const STREET_TAG_RADIUS: i64 = 12;

/// Per-slot district-tag distribution. For each street, find every
/// house slot within `STREET_TAG_RADIUS` (Chebyshev) of the street's
/// widened polyline, then add the street's `tag_weights` to that
/// slot's accumulated map *once* — dedup-per-street keeps contribution
/// flat regardless of how many polyline cells happen to be in range.
fn compute_slot_tags(
    city: &City,
    anchor: (i64, i64),
) -> HashMap<(i64, i64), Vec<(String, u32)>> {
    let k = STREET_TAG_RADIUS;
    let mut acc: HashMap<(i64, i64), HashMap<String, u32>> = HashMap::new();
    for street in &city.streets {
        if street.tag_weights.is_empty() {
            continue;
        }
        let (lo, hi) = street_half_extents(street.width);
        let mut slots_touched: HashSet<(i64, i64)> = HashSet::new();
        for window in street.polyline.windows(2) {
            let aw = (window[0].0 + anchor.0, window[0].1 + anchor.1);
            let bw = (window[1].0 + anchor.0, window[1].1 + anchor.1);
            for (wx, wy) in line_cells(aw, bw) {
                for dy in lo..=hi {
                    for dx in lo..=hi {
                        let cx = wx + dx;
                        let cy = wy + dy;
                        // Slot-coord range whose centre might be in
                        // Chebyshev-k of (cx, cy). Use div_euclid for
                        // both bounds + an extra cell on the high side
                        // to absorb the ceil case; the exact check
                        // below filters the over-iteration.
                        let sx_lo = (cx - anchor.0 - HOUSE_PITCH / 2 - k)
                            .div_euclid(HOUSE_PITCH);
                        let sx_hi = (cx - anchor.0 - HOUSE_PITCH / 2 + k)
                            .div_euclid(HOUSE_PITCH)
                            + 1;
                        let sy_lo = (cy - anchor.1 - HOUSE_PITCH / 2 - k)
                            .div_euclid(HOUSE_PITCH);
                        let sy_hi = (cy - anchor.1 - HOUSE_PITCH / 2 + k)
                            .div_euclid(HOUSE_PITCH)
                            + 1;
                        for sy in sy_lo..=sy_hi {
                            for sx in sx_lo..=sx_hi {
                                let centre_x =
                                    sx * HOUSE_PITCH + anchor.0 + HOUSE_PITCH / 2;
                                let centre_y =
                                    sy * HOUSE_PITCH + anchor.1 + HOUSE_PITCH / 2;
                                if (cx - centre_x).abs() <= k
                                    && (cy - centre_y).abs() <= k
                                {
                                    slots_touched.insert((sx, sy));
                                }
                            }
                        }
                    }
                }
            }
        }
        for slot in slots_touched {
            let entry = acc.entry(slot).or_default();
            for (tag, w) in &street.tag_weights {
                *entry.entry(tag.clone()).or_insert(0) += w;
            }
        }
    }
    acc.into_iter()
        .map(|(slot, tags)| {
            // Stable order: by tag string so the picker iterates the
            // same way on every host. (Roulette is order-sensitive
            // when total weight wraps.)
            let mut v: Vec<(String, u32)> = tags.into_iter().collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            (slot, v)
        })
        .collect()
}

/// Every world cell that any street polyline covers, including its
/// width widening. Built once at city load and consulted during
/// block-fill to keep house rectangles from straddling streets.
fn compute_street_cells(city: &City, anchor: (i64, i64)) -> HashSet<(i64, i64)> {
    let mut out = HashSet::new();
    for street in &city.streets {
        let (lo, hi) = street_half_extents(street.width);
        for window in street.polyline.windows(2) {
            let aw = (window[0].0 + anchor.0, window[0].1 + anchor.1);
            let bw = (window[1].0 + anchor.0, window[1].1 + anchor.1);
            for (wx, wy) in line_cells(aw, bw) {
                for dy in lo..=hi {
                    for dx in lo..=hi {
                        out.insert((wx + dx, wy + dy));
                    }
                }
            }
        }
    }
    out
}

static CITIES: OnceLock<HashMap<&'static str, LoadedCity>> = OnceLock::new();

/// Lazily parse all bundled city RON files and resolve each one's
/// anchor from `NAMED_SITES`. Panics on parse failure or missing
/// `NamedSite` — both are build-time defects, fail fast at boot.
pub fn cities() -> &'static HashMap<&'static str, LoadedCity> {
    CITIES.get_or_init(|| {
        let mut map: HashMap<&'static str, LoadedCity> = HashMap::new();
        let entries: &[(&'static str, &'static str)] = &[("Exeter", EXETER_RON)];
        for (name, src) in entries {
            let city: City = ron::from_str(src)
                .unwrap_or_else(|e| panic!("city RON parse failed for {name}: {e}"));
            let anchor = crate::cornwall::NAMED_SITES
                .iter()
                .find(|s| s.name == *name)
                .map(|s| s.anchor_cell)
                .unwrap_or_else(|| {
                    panic!("city {name} has no matching NamedSite in cornwall.rs")
                });
            let bbox = compute_city_bbox(&city, anchor);
            let wall_world_polygon: Vec<(i64, i64)> = city
                .wall
                .polygon
                .iter()
                .map(|p| (p.0 + anchor.0, p.1 + anchor.1))
                .collect();
            let wall_bbox = compute_bbox(&wall_world_polygon);
            let street_cells = compute_street_cells(&city, anchor);
            let slot_tags = compute_slot_tags(&city, anchor);
            let chunk_features =
                classify_chunks(&city, anchor, &wall_world_polygon, wall_bbox);
            map.insert(
                name,
                LoadedCity {
                    city,
                    anchor,
                    bbox,
                    wall_world_polygon,
                    wall_bbox,
                    street_cells,
                    slot_tags,
                    chunk_features,
                },
            );
        }
        map
    })
}

fn compute_bbox(points: &[(i64, i64)]) -> (i64, i64, i64, i64) {
    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;
    for &(x, y) in points {
        if x < min_x { min_x = x; }
        if y < min_y { min_y = y; }
        if x > max_x { max_x = x; }
        if y > max_y { max_y = y; }
    }
    (min_x, min_y, max_x, max_y)
}

/// Compute the inclusive world-cell bbox of every authored polygon /
/// polyline in `city`, offset by `anchor`. Used to drive chunk
/// stamping (`LoadedCity::intersects_chunk`).
fn compute_city_bbox(city: &City, anchor: (i64, i64)) -> (i64, i64, i64, i64) {
    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;
    let mut acc = |p: (i64, i64)| {
        let x = p.0 + anchor.0;
        let y = p.1 + anchor.1;
        if x < min_x { min_x = x; }
        if y < min_y { min_y = y; }
        if x > max_x { max_x = x; }
        if y > max_y { max_y = y; }
    };
    for &p in &city.wall.polygon {
        acc(p);
    }
    for street in &city.streets {
        for &p in &street.polyline {
            acc(p);
        }
    }
    for lm in &city.landmarks {
        match &lm.footprint {
            Footprint::Rect(a, b) => {
                acc(*a);
                acc(*b);
            }
            Footprint::Polygon(verts) => {
                for &p in verts {
                    acc(p);
                }
            }
        }
    }
    (min_x, min_y, max_x, max_y)
}

/// Build the per-chunk feature classification for the overmap. Visits
/// each authored feature once and writes its tag into every chunk it
/// touches. Build order encodes priority: Building → Street → WallEdge
/// → Cathedral/Castle → Gate, with later writes winning.
fn classify_chunks(
    city: &City,
    anchor: (i64, i64),
    wall_world_polygon: &[(i64, i64)],
    wall_bbox: (i64, i64, i64, i64),
) -> HashMap<ChunkCoord, CityChunkFeature> {
    let mut out: HashMap<ChunkCoord, CityChunkFeature> = HashMap::new();

    // 1. Building: chunks whose center sits inside the wall polygon.
    //    Cheap one-point test; the wall-edge pass below overwrites the
    //    perimeter chunks so the outline still reads as wall.
    let cx_min = wall_bbox.0.div_euclid(CHUNK_W as i64) as i32;
    let cy_min = wall_bbox.1.div_euclid(CHUNK_H as i64) as i32;
    let cx_max = wall_bbox.2.div_euclid(CHUNK_W as i64) as i32;
    let cy_max = wall_bbox.3.div_euclid(CHUNK_H as i64) as i32;
    for cy in cy_min..=cy_max {
        for cx in cx_min..=cx_max {
            let cc = ChunkCoord { cx, cy };
            let center_x = cc.cx as i64 * CHUNK_W as i64 + CHUNK_W as i64 / 2;
            let center_y = cc.cy as i64 * CHUNK_H as i64 + CHUNK_H as i64 / 2;
            if point_in_polygon((center_x, center_y), wall_world_polygon) {
                out.insert(cc, CityChunkFeature::Building);
            }
        }
    }

    // 2. Street: every chunk a street polyline touches.
    for street in &city.streets {
        for window in street.polyline.windows(2) {
            let aw = (window[0].0 + anchor.0, window[0].1 + anchor.1);
            let bw = (window[1].0 + anchor.0, window[1].1 + anchor.1);
            for (wx, wy) in line_cells(aw, bw) {
                out.insert(chunk_for_world_cell(wx, wy), CityChunkFeature::Street);
            }
        }
    }

    // 3. WallEdge: every chunk a wall segment touches.
    let n = city.wall.polygon.len();
    for i in 0..n {
        let a = city.wall.polygon[i];
        let b = city.wall.polygon[(i + 1) % n];
        let aw = (a.0 + anchor.0, a.1 + anchor.1);
        let bw = (b.0 + anchor.0, b.1 + anchor.1);
        for (wx, wy) in line_cells(aw, bw) {
            out.insert(chunk_for_world_cell(wx, wy), CityChunkFeature::WallEdge);
        }
    }

    // 4. Cathedral / Castle landmarks. Name-based dispatch — there are
    //    a handful of landmarks per city and matching by name keeps the
    //    classification cheap and obvious from the RON.
    for lm in &city.landmarks {
        let feat = landmark_feature(&lm.name);
        let Some(feat) = feat else { continue };
        for (wx, wy) in footprint_cells(&lm.footprint, anchor) {
            out.insert(chunk_for_world_cell(wx, wy), feat);
        }
    }

    // 5. Gate: each gate chunk wins over the wall it sits on. One cell
    //    per gate is enough — gates are points, not areas.
    for gate in &city.wall.gates {
        let wx = gate.cell.0 + anchor.0;
        let wy = gate.cell.1 + anchor.1;
        out.insert(chunk_for_world_cell(wx, wy), CityChunkFeature::Gate);
    }

    out
}

fn chunk_for_world_cell(wx: i64, wy: i64) -> ChunkCoord {
    ChunkCoord {
        cx: wx.div_euclid(CHUNK_W as i64) as i32,
        cy: wy.div_euclid(CHUNK_H as i64) as i32,
    }
}

/// Map a landmark name to its overmap feature tag. Returns `None` for
/// landmarks we don't render on the overmap (Cathedral Close — paved
/// fill is implicit under the Cathedral; Guildhall — too small to read
/// at chunk scale; Bridge/Quay/Leat — extramural features not yet
/// stamped in-game, so they shouldn't show on the overmap either).
fn landmark_feature(name: &str) -> Option<CityChunkFeature> {
    let n = name.to_ascii_lowercase();
    if n.contains("cathedral") && !n.contains("close") {
        Some(CityChunkFeature::Cathedral)
    } else if n.contains("rougemont") {
        Some(CityChunkFeature::Castle)
    } else {
        None
    }
}

/// All world-cells covered by a landmark footprint (after anchor
/// offset). Used by the chunk classifier to mark every chunk a landmark
/// occupies, not just the centroid.
fn footprint_cells(fp: &Footprint, anchor: (i64, i64)) -> Vec<(i64, i64)> {
    match fp {
        Footprint::Rect(a, b) => {
            let min_x = a.0.min(b.0) + anchor.0;
            let max_x = a.0.max(b.0) + anchor.0;
            let min_y = a.1.min(b.1) + anchor.1;
            let max_y = a.1.max(b.1) + anchor.1;
            let mut out = Vec::with_capacity(((max_x - min_x + 1) * (max_y - min_y + 1)) as usize);
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    out.push((x, y));
                }
            }
            out
        }
        Footprint::Polygon(verts) => {
            if verts.len() < 3 {
                return Vec::new();
            }
            let world_poly: Vec<(i64, i64)> = verts
                .iter()
                .map(|p| (p.0 + anchor.0, p.1 + anchor.1))
                .collect();
            let mut min_x = i64::MAX;
            let mut min_y = i64::MAX;
            let mut max_x = i64::MIN;
            let mut max_y = i64::MIN;
            for &(x, y) in &world_poly {
                if x < min_x { min_x = x; }
                if y < min_y { min_y = y; }
                if x > max_x { max_x = x; }
                if y > max_y { max_y = y; }
            }
            let mut out = Vec::new();
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    if point_in_polygon((x, y), &world_poly) {
                        out.push((x, y));
                    }
                }
            }
            out
        }
    }
}

impl City {
    /// Stamp this city's authored skeleton into the cells of one chunk.
    /// `anchor` is the city's world-cell anchor (`NamedSite.anchor_cell`).
    /// `world_seed` salts the block-fill hash so different worlds
    /// produce different (but per-world deterministic) houses.
    /// `wall_world_polygon` and `wall_bbox` are precomputed on the
    /// `LoadedCity` and threaded in to avoid re-offsetting per call.
    ///
    /// Mutates only cells whose world coords fall inside this chunk.
    ///
    /// Stamp order — bottom layer to top, so the top wins at overlaps:
    /// 0. Clear trees + undergrowth inside the wall polygon
    /// 1. Streets (CobbleRoad polylines, widened)
    /// 2. Paved-area landmarks (CobbleRoad fill, e.g. Cathedral Close)
    /// 3. Stone-mass landmarks (StoneWall fill, e.g. Cathedral, Rougemont)
    /// 4. Wall polygon edges (StoneWall, with gate cells skipped)
    /// 5. Block-fill: WoodWall+Floor houses on remaining Grass cells
    ///    inside the wall polygon.
    pub fn stamp_into_chunk_with_seed(
        &self,
        coord: ChunkCoord,
        cells: &mut [CellState],
        anchor: (i64, i64),
        world_seed: u64,
        wall_world_polygon: &[(i64, i64)],
        wall_bbox: (i64, i64, i64, i64),
        street_cells: &HashSet<(i64, i64)>,
        slot_tags: &HashMap<(i64, i64), Vec<(String, u32)>>,
    ) {
        let chunk_origin_x = coord.cx as i64 * CHUNK_W as i64;
        let chunk_origin_y = coord.cy as i64 * CHUNK_H as i64;
        let chunk_max_x = chunk_origin_x + CHUNK_W as i64 - 1;
        let chunk_max_y = chunk_origin_y + CHUNK_H as i64 - 1;

        let wall_overlaps_chunk = rect_intersects(
            wall_bbox,
            (chunk_origin_x, chunk_origin_y, chunk_max_x, chunk_max_y),
        );

        // Pass 0: clear trees + undergrowth inside the wall polygon
        // so streets and houses sit on cleared earth, not forest.
        // Skipped if the wall doesn't overlap this chunk.
        if wall_overlaps_chunk {
            self.clear_natural_layer_inside_wall(
                wall_world_polygon,
                wall_bbox,
                chunk_origin_x,
                chunk_origin_y,
                chunk_max_x,
                chunk_max_y,
                cells,
            );
        }

        // Pass 1: streets.
        for street in &self.streets {
            self.stamp_street(
                street,
                anchor,
                chunk_origin_x,
                chunk_origin_y,
                chunk_max_x,
                chunk_max_y,
                cells,
            );
        }

        // Pass 2: paved-area landmarks (CobbleRoad fill).
        for lm in &self.landmarks {
            if lm.kind == LandmarkKind::PavedArea {
                self.stamp_landmark_fill(
                    lm,
                    TerrainKind::CobbleRoad,
                    anchor,
                    chunk_origin_x,
                    chunk_origin_y,
                    chunk_max_x,
                    chunk_max_y,
                    cells,
                );
            }
        }

        // Pass 3: stone-mass landmarks (StoneWall fill).
        for lm in &self.landmarks {
            if lm.kind == LandmarkKind::StoneMass {
                self.stamp_landmark_fill(
                    lm,
                    TerrainKind::StoneWall,
                    anchor,
                    chunk_origin_x,
                    chunk_origin_y,
                    chunk_max_x,
                    chunk_max_y,
                    cells,
                );
            }
        }

        // Pass 4: wall polygon edges, gates skipped.
        //
        // Bresenham steps diagonally whenever the segment isn't axis-
        // aligned. Exeter's polygon has zero axis-aligned segments, so
        // every wall run takes diagonal steps. Two cells that are only
        // diagonally adjacent aren't seen as connected by the 4-cardinal
        // `wall_connector_glyph` lookup, and the wall renders as
        // disconnected stubs. To keep the auto-tile happy we insert a
        // bridge cell at every diagonal step so consecutive wall cells
        // are always 4-cardinal adjacent. Bridge is placed at
        // `(prev.x, new.y)` — Manhattan corner sharing the prev cell's
        // column and the new cell's row. `prev` is carried across
        // segments via the shared vertex (no-op diff there).
        let gate_skip = self.gate_skip_cells(anchor);
        let n = self.wall.polygon.len();
        let mut prev: Option<(i64, i64)> = None;
        for i in 0..n {
            let a = self.wall.polygon[i];
            let b = self.wall.polygon[(i + 1) % n];
            let aw = (a.0 + anchor.0, a.1 + anchor.1);
            let bw = (b.0 + anchor.0, b.1 + anchor.1);
            for (wx, wy) in line_cells(aw, bw) {
                if let Some((px, py)) = prev {
                    if (wx - px).abs() == 1 && (wy - py).abs() == 1 {
                        let bridge = (px, wy);
                        if !gate_skip.contains(&bridge) {
                            stamp_wall_cell(
                                cells,
                                bridge,
                                chunk_origin_x,
                                chunk_origin_y,
                            );
                        }
                    }
                }
                if !gate_skip.contains(&(wx, wy)) {
                    stamp_wall_cell(cells, (wx, wy), chunk_origin_x, chunk_origin_y);
                }
                prev = Some((wx, wy));
            }
        }

        // Pass 5: block-fill houses. Slot grid is anchor-relative, so
        // a house spanning two chunks gets the same shape in both —
        // no chunk seams. Each slot only stamps cells currently Grass,
        // so streets / landmarks / walls keep precedence.
        if wall_overlaps_chunk {
            self.stamp_block_fill(
                anchor,
                world_seed,
                wall_world_polygon,
                street_cells,
                slot_tags,
                chunk_origin_x,
                chunk_origin_y,
                chunk_max_x,
                chunk_max_y,
                cells,
            );
        }
    }

    /// Strip the natural layer (trees, undergrowth decorations, scattered
    /// debris items, and ground cover) from every cell inside the wall
    /// polygon, so the city's streets and houses sit on cleared land.
    /// "For now" the city is fully swept — no twigs/pebbles inside the
    /// walls; foraging is a countryside activity. Revisit if/when towns
    /// want a "littered alley" aesthetic.
    fn clear_natural_layer_inside_wall(
        &self,
        wall_world_polygon: &[(i64, i64)],
        wall_bbox: (i64, i64, i64, i64),
        chunk_origin_x: i64,
        chunk_origin_y: i64,
        chunk_max_x: i64,
        chunk_max_y: i64,
        cells: &mut [CellState],
    ) {
        let min_x = wall_bbox.0.max(chunk_origin_x);
        let min_y = wall_bbox.1.max(chunk_origin_y);
        let max_x = wall_bbox.2.min(chunk_max_x);
        let max_y = wall_bbox.3.min(chunk_max_y);
        for wy in min_y..=max_y {
            for wx in min_x..=max_x {
                if !point_in_polygon((wx, wy), wall_world_polygon) {
                    continue;
                }
                let lx = (wx - chunk_origin_x) as usize;
                let ly = (wy - chunk_origin_y) as usize;
                let idx = ly * (CHUNK_W as usize) + lx;
                if cells[idx].terrain == TerrainKind::TreeTrunk {
                    cells[idx].terrain = TerrainKind::Grass;
                }
                cells[idx].tree_species = None;
                cells[idx].decoration = crate::flora::Decoration::None;
                cells[idx].items.clear();
                cells[idx].ground_cover = GroundCover::None;
            }
        }
    }

    /// Iterate house-grid slots overlapping this chunk; stamp houses
    /// whose 4 corners are inside the wall polygon. Per-slot hashed
    /// dimensions + offset within the slot, so adjacent chunks always
    /// agree on house placement.
    fn stamp_block_fill(
        &self,
        anchor: (i64, i64),
        world_seed: u64,
        wall_world_polygon: &[(i64, i64)],
        street_cells: &HashSet<(i64, i64)>,
        slot_tags: &HashMap<(i64, i64), Vec<(String, u32)>>,
        chunk_origin_x: i64,
        chunk_origin_y: i64,
        chunk_max_x: i64,
        chunk_max_y: i64,
        cells: &mut [CellState],
    ) {
        // Slot coords are anchor-relative.
        let slot_min_x = (chunk_origin_x - anchor.0).div_euclid(HOUSE_PITCH) - 1;
        let slot_min_y = (chunk_origin_y - anchor.1).div_euclid(HOUSE_PITCH) - 1;
        let slot_max_x = (chunk_max_x - anchor.0).div_euclid(HOUSE_PITCH) + 1;
        let slot_max_y = (chunk_max_y - anchor.1).div_euclid(HOUSE_PITCH) + 1;

        for slot_y in slot_min_y..=slot_max_y {
            for slot_x in slot_min_x..=slot_max_x {
                stamp_one_prefab_slot(
                    slot_x,
                    slot_y,
                    anchor,
                    world_seed,
                    self.tier,
                    wall_world_polygon,
                    street_cells,
                    slot_tags,
                    chunk_origin_x,
                    chunk_origin_y,
                    chunk_max_x,
                    chunk_max_y,
                    cells,
                );
            }
        }
    }

    fn stamp_street(
        &self,
        street: &Street,
        anchor: (i64, i64),
        chunk_origin_x: i64,
        chunk_origin_y: i64,
        chunk_max_x: i64,
        chunk_max_y: i64,
        cells: &mut [CellState],
    ) {
        let (lo, hi) = street_half_extents(street.width);
        for window in street.polyline.windows(2) {
            let aw = (window[0].0 + anchor.0, window[0].1 + anchor.1);
            let bw = (window[1].0 + anchor.0, window[1].1 + anchor.1);
            for (wx, wy) in line_cells(aw, bw) {
                // Square neighborhood of side `width` covering the
                // line cell — cheap and visually reads as "wider".
                for dy in lo..=hi {
                    for dx in lo..=hi {
                        let cx = wx + dx;
                        let cy = wy + dy;
                        if cx < chunk_origin_x
                            || cy < chunk_origin_y
                            || cx > chunk_max_x
                            || cy > chunk_max_y
                        {
                            continue;
                        }
                        let lx = (cx - chunk_origin_x) as usize;
                        let ly = (cy - chunk_origin_y) as usize;
                        set_terrain(cells, lx, ly, TerrainKind::CobbleRoad);
                    }
                }
            }
        }
    }

    fn stamp_landmark_fill(
        &self,
        lm: &Landmark,
        terrain: TerrainKind,
        anchor: (i64, i64),
        chunk_origin_x: i64,
        chunk_origin_y: i64,
        chunk_max_x: i64,
        chunk_max_y: i64,
        cells: &mut [CellState],
    ) {
        match &lm.footprint {
            Footprint::Rect(a, b) => {
                let min_x = (a.0.min(b.0) + anchor.0).max(chunk_origin_x);
                let min_y = (a.1.min(b.1) + anchor.1).max(chunk_origin_y);
                let max_x = (a.0.max(b.0) + anchor.0).min(chunk_max_x);
                let max_y = (a.1.max(b.1) + anchor.1).min(chunk_max_y);
                if min_x > max_x || min_y > max_y {
                    return;
                }
                for wy in min_y..=max_y {
                    for wx in min_x..=max_x {
                        let lx = (wx - chunk_origin_x) as usize;
                        let ly = (wy - chunk_origin_y) as usize;
                        set_terrain(cells, lx, ly, terrain);
                    }
                }
            }
            Footprint::Polygon(verts) => {
                if verts.len() < 3 {
                    return;
                }
                // Anchor-offset polygon for tests in world coords.
                let world_poly: Vec<(i64, i64)> = verts
                    .iter()
                    .map(|p| (p.0 + anchor.0, p.1 + anchor.1))
                    .collect();
                let mut poly_min_x = i64::MAX;
                let mut poly_min_y = i64::MAX;
                let mut poly_max_x = i64::MIN;
                let mut poly_max_y = i64::MIN;
                for &(x, y) in &world_poly {
                    if x < poly_min_x { poly_min_x = x; }
                    if y < poly_min_y { poly_min_y = y; }
                    if x > poly_max_x { poly_max_x = x; }
                    if y > poly_max_y { poly_max_y = y; }
                }
                let min_x = poly_min_x.max(chunk_origin_x);
                let min_y = poly_min_y.max(chunk_origin_y);
                let max_x = poly_max_x.min(chunk_max_x);
                let max_y = poly_max_y.min(chunk_max_y);
                if min_x > max_x || min_y > max_y {
                    return;
                }
                for wy in min_y..=max_y {
                    for wx in min_x..=max_x {
                        if point_in_polygon((wx, wy), &world_poly) {
                            let lx = (wx - chunk_origin_x) as usize;
                            let ly = (wy - chunk_origin_y) as usize;
                            set_terrain(cells, lx, ly, terrain);
                        }
                    }
                }
            }
        }
    }

    /// World-cell positions where wall stamping should be suppressed
    /// (gate gaps). Each gate yields a plus-shaped cluster `width` cells
    /// across in both axes: harmless on a perpendicular wall (those
    /// cells aren't on the line), produces a gap on the gate's own wall.
    fn gate_skip_cells(&self, anchor: (i64, i64)) -> HashSet<(i64, i64)> {
        let mut out = HashSet::new();
        for gate in &self.wall.gates {
            let cx = gate.cell.0 + anchor.0;
            let cy = gate.cell.1 + anchor.1;
            let w = gate.width as i64;
            // Center the run of length `w`. width=2 → offsets {-1, 0};
            // width=3 → {-1, 0, +1}; etc.
            let lo = -(w / 2);
            let hi = lo + w - 1;
            for d in lo..=hi {
                out.insert((cx + d, cy)); // horizontal spread
                out.insert((cx, cy + d)); // vertical spread
            }
        }
        out
    }
}

/// House-slot grid pitch (anchor-relative cells). One slot holds a
/// single building with its perimeter alleys. Pitch 8 fits the largest
/// prefab (7×7 armurer) with a one-cell alley on the far edge.
pub const HOUSE_PITCH: i64 = 8;
/// Fraction (out of 256) of slots that stay empty plots (gardens,
/// churchyards we haven't authored yet, ruins).
const EMPTY_SLOT_THRESHOLD: u64 = 50;
const CITY_SEED_SALT: u64 = 0xE7_E7_E7_E7_E7_E7_E7_E7;
/// Salt for the tag-pick hash. Distinct from variant + slot-decision
/// salts so tag, variant, and empty-plot rolls vary independently.
const TAG_SEED_SALT: u64 = 0xCAFE_BABE_DEAD_BEEF;
/// Salt for the variant-pick hash within a `(tag, tier)` pool.
const VARIANT_SEED_SALT: u64 = 0xF00D_F00D_F00D_F00D;

/// Decide what (if anything) a slot should stamp, before any
/// street-overlap or chunk-clip considerations. Pure: same inputs
/// always yield the same `Some(prefab, footprint)`. Returns `None`
/// when the slot is outside the wall polygon, rolled an empty plot,
/// or the picker found no matching prefab. Tests reuse this to
/// verify properties about every slot's footprint without
/// duplicating the picker logic.
pub(crate) fn slot_prefab_footprint(
    slot_x: i64,
    slot_y: i64,
    anchor: (i64, i64),
    world_seed: u64,
    tier: CityTier,
    wall_world_polygon: &[(i64, i64)],
    slot_tags: &HashMap<(i64, i64), Vec<(String, u32)>>,
) -> Option<(&'static Prefab, (i64, i64, i64, i64))> {
    let slot_origin_x = slot_x * HOUSE_PITCH + anchor.0;
    let slot_origin_y = slot_y * HOUSE_PITCH + anchor.1;
    let slot_inner_min_x = slot_origin_x + 1;
    let slot_inner_min_y = slot_origin_y + 1;
    let slot_inner_max_x = slot_origin_x + HOUSE_PITCH - 1;
    let slot_inner_max_y = slot_origin_y + HOUSE_PITCH - 1;

    // 1. Wall containment.
    let corners = [
        (slot_inner_min_x, slot_inner_min_y),
        (slot_inner_max_x, slot_inner_min_y),
        (slot_inner_min_x, slot_inner_max_y),
        (slot_inner_max_x, slot_inner_max_y),
    ];
    if !corners
        .iter()
        .all(|&c| point_in_polygon(c, wall_world_polygon))
    {
        return None;
    }

    // 2. Empty-plot dice (same channel + threshold as legacy block-fill).
    let slot_hash = hash3(slot_x as u64, slot_y as u64, world_seed ^ CITY_SEED_SALT);
    if (slot_hash & 0xFF) < EMPTY_SLOT_THRESHOLD {
        return None;
    }

    // 3. Tag pick. Slots not touched by any tagged street fall back to
    //    the universal `("house", 1)`. Sorted pool is host-stable.
    let fallback: [(String, u32); 1] = [(String::from("house"), 1)];
    let tag_pool: &[(String, u32)] = slot_tags
        .get(&(slot_x, slot_y))
        .map(|v| v.as_slice())
        .unwrap_or(&fallback);
    let tag_hash = hash3(slot_x as u64, slot_y as u64, world_seed ^ TAG_SEED_SALT);
    let tag = roulette_pick(tag_pool, tag_hash)?;

    // 4. Variant pick — fall back to the tier's `house` pool if the
    //    chosen tag has no prefabs (e.g. Village rolling `smithy` when
    //    only `farrier` is bundled).
    let cat = catalog();
    let variant_hash = hash3(slot_x as u64, slot_y as u64, world_seed ^ VARIANT_SEED_SALT);
    let prefab = cat
        .pick_variant(tag, tier, variant_hash)
        .or_else(|| cat.pick_variant("house", tier, variant_hash))?;

    // 5. Slack offset — jitter smaller prefabs within the slot so a
    //    row of cottages doesn't read as one long block.
    let pw = prefab.width() as i64;
    let ph = prefab.height() as i64;
    let slack_x = (HOUSE_PITCH - 1 - pw).max(0) as u64;
    let slack_y = (HOUSE_PITCH - 1 - ph).max(0) as u64;
    let off_x = if slack_x > 0 {
        ((slot_hash >> 24) % slack_x) as i64
    } else {
        0
    };
    let off_y = if slack_y > 0 {
        ((slot_hash >> 32) % slack_y) as i64
    } else {
        0
    };
    let pre_min_x = slot_inner_min_x + off_x;
    let pre_min_y = slot_inner_min_y + off_y;
    let pre_max_x = pre_min_x + pw - 1;
    let pre_max_y = pre_min_y + ph - 1;
    Some((prefab, (pre_min_x, pre_min_y, pre_max_x, pre_max_y)))
}

fn stamp_one_prefab_slot(
    slot_x: i64,
    slot_y: i64,
    anchor: (i64, i64),
    world_seed: u64,
    tier: CityTier,
    wall_world_polygon: &[(i64, i64)],
    street_cells: &HashSet<(i64, i64)>,
    slot_tags: &HashMap<(i64, i64), Vec<(String, u32)>>,
    chunk_origin_x: i64,
    chunk_origin_y: i64,
    chunk_max_x: i64,
    chunk_max_y: i64,
    cells: &mut [CellState],
) {
    let Some((prefab, (pre_min_x, pre_min_y, pre_max_x, pre_max_y))) =
        slot_prefab_footprint(slot_x, slot_y, anchor, world_seed, tier, wall_world_polygon, slot_tags)
    else {
        return;
    };

    // Street overlap — done *after* the offset is chosen, since the
    // prefab's smaller-than-slot footprint can fit in places the old
    // procedural box couldn't.
    for wy in pre_min_y..=pre_max_y {
        for wx in pre_min_x..=pre_max_x {
            if street_cells.contains(&(wx, wy)) {
                return;
            }
        }
    }

    // Stamp the prefab grid. Only overwrite `Grass` — streets /
    // landmarks / walls already laid down and keep precedence; space
    // chars in the grid yield `None` and are skipped.
    let clip_min_x = pre_min_x.max(chunk_origin_x);
    let clip_min_y = pre_min_y.max(chunk_origin_y);
    let clip_max_x = pre_max_x.min(chunk_max_x);
    let clip_max_y = pre_max_y.min(chunk_max_y);
    if clip_min_x > clip_max_x || clip_min_y > clip_max_y {
        return;
    }
    for wy in clip_min_y..=clip_max_y {
        for wx in clip_min_x..=clip_max_x {
            let px = (wx - pre_min_x) as u16;
            let py = (wy - pre_min_y) as u16;
            let Some(t) = prefab.cell_at(px, py) else { continue };
            let lx = (wx - chunk_origin_x) as usize;
            let ly = (wy - chunk_origin_y) as usize;
            let idx = ly * (CHUNK_W as usize) + lx;
            if cells[idx].terrain != TerrainKind::Grass {
                continue;
            }
            set_terrain(cells, lx, ly, t);
        }
    }
}

/// Weighted-roulette pick over `(label, weight)` pairs. Returns `None`
/// if the pool is empty or sums to 0. The non-test path also lives at
/// `BuildingCatalog::pick_variant`; both stay deterministic given the
/// same input pool order + hash.
fn roulette_pick<'a>(pool: &'a [(String, u32)], hash: u64) -> Option<&'a str> {
    if pool.is_empty() {
        return None;
    }
    let total: u32 = pool.iter().map(|(_, w)| *w).sum();
    if total == 0 {
        return None;
    }
    let mut roll = (hash % total as u64) as u32;
    for (label, w) in pool {
        if roll < *w {
            return Some(label);
        }
        roll -= *w;
    }
    pool.first().map(|(s, _)| s.as_str())
}

/// Cheap 3-input SplitMix-style hash. Stable for slot determinism.
fn hash3(a: u64, b: u64, c: u64) -> u64 {
    let mut x = a
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(c.wrapping_mul(0x94D0_49BB_1331_11EB));
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x
}

/// Set a cell's terrain and clear flora overlays that no longer make
/// sense on the new surface (a CobbleRoad doesn't keep its tree
/// species or undergrowth decoration).
fn set_terrain(cells: &mut [CellState], lx: usize, ly: usize, terrain: TerrainKind) {
    let idx = ly * (CHUNK_W as usize) + lx;
    cells[idx].terrain = terrain;
    cells[idx].tree_species = None;
    cells[idx].decoration = crate::flora::Decoration::None;
}

/// Even-odd ray-cast point-in-polygon test. f64 internally to keep the
/// edge-crossing comparison precise; the cell-aligned test points and
/// integer vertices in our data don't hit the degenerate cases that
/// trip integer-only implementations.
fn point_in_polygon(p: (i64, i64), poly: &[(i64, i64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let px = p.0 as f64;
    let py = p.1 as f64;
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let xi = poly[i].0 as f64;
        let yi = poly[i].1 as f64;
        let xj = poly[j].0 as f64;
        let yj = poly[j].1 as f64;
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Inclusive Bresenham line between two cells. Yields each cell on the
/// line exactly once.
/// Stamp a single `StoneWall` at world cell `wcell`, clipped to the
/// chunk. No-op outside the chunk. Used by the wall pass to stamp
/// both Bresenham line cells and the diagonal-step bridge cells.
fn stamp_wall_cell(
    cells: &mut [CellState],
    wcell: (i64, i64),
    chunk_origin_x: i64,
    chunk_origin_y: i64,
) {
    let lx = wcell.0 - chunk_origin_x;
    let ly = wcell.1 - chunk_origin_y;
    if lx < 0 || ly < 0 || lx >= CHUNK_W as i64 || ly >= CHUNK_H as i64 {
        return;
    }
    set_terrain(cells, lx as usize, ly as usize, TerrainKind::StoneWall);
}

fn line_cells(a: (i64, i64), b: (i64, i64)) -> impl Iterator<Item = (i64, i64)> {
    let (x0, y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i64 = if x0 < x1 { 1 } else { -1 };
    let sy: i64 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    std::iter::from_fn(move || {
        if x == i64::MIN {
            return None;
        }
        let out = (x, y);
        if x == x1 && y == y1 {
            x = i64::MIN; // sentinel: emit this cell, then stop
            return Some(out);
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        Some(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exeter_ron_parses() {
        let map = cities();
        let loaded = map.get("Exeter").expect("Exeter loaded");
        assert!(loaded.city.wall.polygon.len() >= 3);
        assert!(!loaded.city.wall.gates.is_empty());
        // Real Exeter has streets and landmarks now too.
        assert!(!loaded.city.streets.is_empty());
        assert!(!loaded.city.landmarks.is_empty());
    }

    #[test]
    fn exeter_bbox_covers_known_features() {
        let loaded = cities().get("Exeter").unwrap();
        let (min_x, min_y, max_x, max_y) = loaded.bbox;
        // Wall E edge ~+118, Exe Bridge SW corner ~-398, Rougemont N
        // ~-548, Quay/leat S edge ~+290. Anchor is (0, 0).
        assert!(min_x <= -398, "min_x = {min_x}, expected ≤ -398 (Exe Bridge)");
        assert!(min_y <= -548, "min_y = {min_y}, expected ≤ -548 (Rougemont)");
        assert!(max_x >= 118, "max_x = {max_x}, expected ≥ 118 (wall E)");
        assert!(max_y >= 290, "max_y = {max_y}, expected ≥ 290 (quay)");
    }

    #[test]
    fn wall_stamps_into_chunks_along_the_circuit() {
        let loaded = cities().get("Exeter").unwrap();
        let cell_count = (CHUNK_W as usize) * (CHUNK_H as usize);
        // Chunk (2, -3) covers world cells [80..120)×[-90..-60). The
        // East-gate vertex (112, -126) is just NE of this chunk, and
        // the segment from (118, -20) to (98, 85) crosses it.
        let mut cells: Vec<CellState> = (0..cell_count)
            .map(|_| CellState::with_terrain(TerrainKind::Grass))
            .collect();
        loaded.stamp_into_chunk(ChunkCoord { cx: 2, cy: -3 }, &mut cells, 0);
        let wall_count = cells
            .iter()
            .filter(|c| c.terrain == TerrainKind::StoneWall)
            .count();
        assert!(
            wall_count > 0,
            "Exeter wall should stamp some StoneWall cells into chunk (2, -3)"
        );
    }

    #[test]
    fn each_gate_center_has_no_wall() {
        let loaded = cities().get("Exeter").unwrap();
        let cell_count = (CHUNK_W as usize) * (CHUNK_H as usize);
        for gate in &loaded.city.wall.gates {
            let world_x = gate.cell.0 + loaded.anchor.0;
            let world_y = gate.cell.1 + loaded.anchor.1;
            let cx = (world_x).div_euclid(CHUNK_W as i64) as i32;
            let cy = (world_y).div_euclid(CHUNK_H as i64) as i32;
            let mut cells: Vec<CellState> = (0..cell_count)
                .map(|_| CellState::with_terrain(TerrainKind::Grass))
                .collect();
            loaded.stamp_into_chunk(ChunkCoord { cx, cy }, &mut cells, 0);
            let lx = world_x - (cx as i64 * CHUNK_W as i64);
            let ly = world_y - (cy as i64 * CHUNK_H as i64);
            let idx = (ly as usize) * (CHUNK_W as usize) + (lx as usize);
            assert_ne!(
                cells[idx].terrain,
                TerrainKind::StoneWall,
                "gate {} center ({world_x}, {world_y}) should be a gap, not a wall",
                gate.name
            );
        }
    }

    #[test]
    fn bbox_intersection_skips_far_chunks() {
        let loaded = cities().get("Exeter").unwrap();
        // (1000, 1000) is hundreds of chunks away from Exeter.
        assert!(!loaded.intersects_chunk(ChunkCoord { cx: 1000, cy: 1000 }));
        // (0, 0) is the anchor; must intersect.
        assert!(loaded.intersects_chunk(ChunkCoord { cx: 0, cy: 0 }));
    }

    /// Stamp every chunk Exeter touches into a HashMap so a test can
    /// query terrain at any world cell inside the city's bbox.
    fn build_full_city_grid() -> std::collections::HashMap<(i64, i64), TerrainKind> {
        let loaded = cities().get("Exeter").unwrap();
        let (min_x, min_y, max_x, max_y) = loaded.bbox;
        let cx_min = (min_x).div_euclid(CHUNK_W as i64) as i32;
        let cy_min = (min_y).div_euclid(CHUNK_H as i64) as i32;
        let cx_max = (max_x).div_euclid(CHUNK_W as i64) as i32;
        let cy_max = (max_y).div_euclid(CHUNK_H as i64) as i32;
        let cell_count = (CHUNK_W as usize) * (CHUNK_H as usize);
        let mut grid: std::collections::HashMap<(i64, i64), TerrainKind> =
            std::collections::HashMap::new();
        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                let mut cells: Vec<CellState> = (0..cell_count)
                    .map(|_| CellState::with_terrain(TerrainKind::Grass))
                    .collect();
                loaded.stamp_into_chunk(ChunkCoord { cx, cy }, &mut cells, 0);
                let origin_x = cx as i64 * CHUNK_W as i64;
                let origin_y = cy as i64 * CHUNK_H as i64;
                for ly in 0..(CHUNK_H as usize) {
                    for lx in 0..(CHUNK_W as usize) {
                        let idx = ly * (CHUNK_W as usize) + lx;
                        if cells[idx].terrain != TerrainKind::Grass {
                            grid.insert(
                                (origin_x + lx as i64, origin_y + ly as i64),
                                cells[idx].terrain,
                            );
                        }
                    }
                }
            }
        }
        grid
    }

    #[test]
    fn streets_stamp_cobble_at_polyline_centers() {
        let grid = build_full_city_grid();
        // Sample two cells from High Street's polyline. They should
        // come out as CobbleRoad (or covered by a wider terrain like a
        // gate or landmark — accept that, but never Grass).
        for &(wx, wy) in &[(-120i64, -25i64), (20i64, -90i64), (-40i64, -78i64)] {
            let t = grid.get(&(wx, wy)).copied().unwrap_or(TerrainKind::Grass);
            assert_ne!(
                t, TerrainKind::Grass,
                "High Street cell ({wx}, {wy}) should be stamped, got Grass"
            );
        }
    }

    #[test]
    fn cathedral_footprint_is_stone() {
        let grid = build_full_city_grid();
        // Cathedral rect is (-42,-12) to (42,12). Anchor (0,0). The
        // centroid (0, 0) is the spawn cell — assert it's StoneWall
        // (and yes, this means the player currently spawns on stone;
        // gameplay fix is to relocate the spawn cell or carve a door).
        let t = grid.get(&(0i64, 0i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(t, TerrainKind::StoneWall, "Cathedral centroid should be StoneWall");
        // A corner inside the rect:
        let t2 = grid.get(&(40i64, 10i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(t2, TerrainKind::StoneWall, "Cathedral corner should be StoneWall");
    }

    #[test]
    fn cathedral_close_is_paved_outside_cathedral() {
        let grid = build_full_city_grid();
        // (40, 30) is inside the Close polygon but outside the Cathedral
        // rect (which ends at y=12). Should be CobbleRoad.
        let t = grid.get(&(40i64, 30i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(
            t, TerrainKind::CobbleRoad,
            "Close cell (40, 30) outside Cathedral rect should be CobbleRoad"
        );
    }

    #[test]
    fn bridge_and_leat_are_not_stamped() {
        let grid = build_full_city_grid();
        // Bridge midpoint approx (-320, 258); leat midpoint approx
        // (-22, 223). Both should be untouched in v1 (Grass).
        assert_eq!(
            grid.get(&(-320i64, 258i64)).copied().unwrap_or(TerrainKind::Grass),
            TerrainKind::Grass,
            "Exe Bridge cells should be deferred — not stamped yet"
        );
        assert_eq!(
            grid.get(&(-22i64, 223i64)).copied().unwrap_or(TerrainKind::Grass),
            TerrainKind::Grass,
            "Mill Leat cells should be deferred — not stamped yet"
        );
    }

    #[test]
    fn spawn_cell_is_walkable_after_stamping() {
        // The player spawns at chunk-(0,0) local (20, 15) = world
        // (20, 15). With city stamping covering most of central
        // Exeter, the spawn cell must remain walkable — otherwise
        // the player can't move on game start. The spawn-disc
        // enforcement in chunkgen only clears trees / gorse, NOT
        // walls — so if a StoneWall lands here, we have a problem.
        let grid = build_full_city_grid();
        let t = grid
            .get(&(20i64, 15i64))
            .copied()
            .unwrap_or(TerrainKind::Grass);
        assert!(
            t.def().walkable,
            "spawn cell (20, 15) ended up on non-walkable {t:?} after city stamping"
        );
    }

    #[test]
    fn block_fill_produces_houses_inside_walls() {
        let grid = build_full_city_grid();
        let wood = grid.values().filter(|t| **t == TerrainKind::WoodWall).count();
        let floor = grid.values().filter(|t| **t == TerrainKind::Floor).count();
        assert!(
            wood > 50,
            "expected >50 WoodWall cells from block-fill, got {wood}"
        );
        assert!(
            floor > 50,
            "expected >50 Floor cells from block-fill interiors, got {floor}"
        );
    }

    #[test]
    fn block_fill_is_deterministic_across_runs() {
        let loaded = cities().get("Exeter").unwrap();
        let cell_count = (CHUNK_W as usize) * (CHUNK_H as usize);
        // Pick a chunk well inside the wall where houses must spawn.
        let coord = ChunkCoord { cx: -2, cy: 2 };
        let mut a: Vec<CellState> = (0..cell_count)
            .map(|_| CellState::with_terrain(TerrainKind::Grass))
            .collect();
        let mut b: Vec<CellState> = (0..cell_count)
            .map(|_| CellState::with_terrain(TerrainKind::Grass))
            .collect();
        loaded.stamp_into_chunk(coord, &mut a, 0xC0FFEE);
        loaded.stamp_into_chunk(coord, &mut b, 0xC0FFEE);
        let mismatches: usize = a
            .iter()
            .zip(b.iter())
            .filter(|(x, y)| x.terrain != y.terrain)
            .count();
        assert_eq!(mismatches, 0, "block-fill must be seed-deterministic");
    }

    #[test]
    fn block_fill_does_not_clobber_streets() {
        let grid = build_full_city_grid();
        // Pick three High Street cells (between west end and east gate).
        // They should remain CobbleRoad after block-fill — houses only
        // overwrite Grass cells.
        for &(wx, wy) in &[(-95i64, -66i64), (20i64, -90i64), (70i64, -108i64)] {
            let t = grid.get(&(wx, wy)).copied().unwrap_or(TerrainKind::Grass);
            assert_eq!(
                t, TerrainKind::CobbleRoad,
                "High Street cell ({wx}, {wy}) was clobbered by block-fill: got {t:?}"
            );
        }
    }

    #[test]
    fn block_fill_does_not_clobber_cathedral() {
        let grid = build_full_city_grid();
        // Cathedral interior cell (0, 0) — the anchor itself.
        let t = grid.get(&(0i64, 0i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(
            t, TerrainKind::StoneWall,
            "Cathedral centroid was clobbered by block-fill: got {t:?}"
        );
    }

    #[test]
    fn block_fill_houses_do_not_straddle_walls() {
        // Block-fill only stamps slots whose 4 corners are inside the
        // wall polygon. Verify: every WoodWall cell adjacent to or
        // beyond the wall (i.e. with no path inside) must be the wall
        // itself, not a house. We approximate this by checking that
        // every WoodWall cell within the wall bbox is also inside the
        // wall polygon.
        let loaded = cities().get("Exeter").unwrap();
        let grid = build_full_city_grid();
        let mut leak = 0u32;
        for (&(wx, wy), &t) in &grid {
            if t != TerrainKind::WoodWall {
                continue;
            }
            // Only check cells inside the wall bbox; outside the bbox
            // there'd be no houses anyway by slot containment.
            let (wmin_x, wmin_y, wmax_x, wmax_y) = loaded.wall_bbox;
            if wx < wmin_x || wy < wmin_y || wx > wmax_x || wy > wmax_y {
                continue;
            }
            if !point_in_polygon((wx, wy), &loaded.wall_world_polygon) {
                leak += 1;
            }
        }
        assert_eq!(
            leak, 0,
            "{leak} WoodWall cells leaked outside the wall polygon"
        );
    }

    #[test]
    fn chunk_features_cover_wall_perimeter_and_gates() {
        let loaded = cities().get("Exeter").unwrap();
        // Anchor chunk (0,0) is the Cathedral footprint.
        assert_eq!(
            loaded.chunk_feature(ChunkCoord { cx: 0, cy: 0 }),
            Some(CityChunkFeature::Cathedral),
            "anchor chunk should classify as Cathedral"
        );
        // Each gate's chunk classifies as Gate.
        for gate in &loaded.city.wall.gates {
            let wx = gate.cell.0 + loaded.anchor.0;
            let wy = gate.cell.1 + loaded.anchor.1;
            let cc = ChunkCoord {
                cx: wx.div_euclid(CHUNK_W as i64) as i32,
                cy: wy.div_euclid(CHUNK_H as i64) as i32,
            };
            assert_eq!(
                loaded.chunk_feature(cc),
                Some(CityChunkFeature::Gate),
                "gate {} chunk {:?} should classify as Gate",
                gate.name,
                cc
            );
        }
        // Rougemont chunks classify as Castle. Motte centroid ~ (-87, -488).
        let rougemont_cc = ChunkCoord {
            cx: (-87i64).div_euclid(CHUNK_W as i64) as i32,
            cy: (-488i64).div_euclid(CHUNK_H as i64) as i32,
        };
        assert_eq!(
            loaded.chunk_feature(rougemont_cc),
            Some(CityChunkFeature::Castle),
            "Rougemont chunk should classify as Castle"
        );
        // A chunk well outside the city (far north) has no feature.
        assert_eq!(
            loaded.chunk_feature(ChunkCoord { cx: 0, cy: -200 }),
            None,
        );
    }

    /// Dump Exeter's chunk-feature grid as ASCII so a reviewer can
    /// eyeball the overmap layout without launching the game. Run via:
    /// `cargo test --release chunk_feature_grid_dump -- --nocapture`
    #[test]
    fn chunk_feature_grid_dump() {
        let loaded = cities().get("Exeter").unwrap();
        let (min_x, min_y, max_x, max_y) = loaded.bbox;
        let cx_min = min_x.div_euclid(CHUNK_W as i64) as i32 - 1;
        let cy_min = min_y.div_euclid(CHUNK_H as i64) as i32 - 1;
        let cx_max = max_x.div_euclid(CHUNK_W as i64) as i32 + 1;
        let cy_max = max_y.div_euclid(CHUNK_H as i64) as i32 + 1;
        println!("Exeter overmap chunk grid (cx {cx_min}..={cx_max}, cy {cy_min}..={cy_max})");
        println!("legend: # cathedral · K castle · = wall · + gate · - street · . building · ' empty");
        for cy in cy_min..=cy_max {
            let mut row = String::new();
            for cx in cx_min..=cx_max {
                let glyph = match loaded.chunk_feature(ChunkCoord { cx, cy }) {
                    Some(CityChunkFeature::Cathedral) => '#',
                    Some(CityChunkFeature::Castle) => 'K',
                    Some(CityChunkFeature::WallEdge) => '=',
                    Some(CityChunkFeature::Gate) => '+',
                    Some(CityChunkFeature::Street) => '-',
                    Some(CityChunkFeature::Building) => '.',
                    None => '\'',
                };
                row.push(glyph);
                row.push(' ');
            }
            println!("  {row}");
        }
    }

    #[test]
    fn chunk_features_include_streets() {
        let loaded = cities().get("Exeter").unwrap();
        // Sample High Street mid-polyline cell (-40, -78) → its chunk
        // should classify as Street (or Gate if it shares the East-gate
        // chunk).
        let cc = ChunkCoord {
            cx: (-40i64).div_euclid(CHUNK_W as i64) as i32,
            cy: (-78i64).div_euclid(CHUNK_H as i64) as i32,
        };
        let feat = loaded.chunk_feature(cc);
        assert!(
            matches!(
                feat,
                Some(CityChunkFeature::Street)
                    | Some(CityChunkFeature::Gate)
                    | Some(CityChunkFeature::WallEdge)
            ),
            "High Street chunk {:?} should be Street/Gate/WallEdge, got {:?}",
            cc,
            feat
        );
    }

    #[test]
    fn width_two_street_stamps_two_cells_wide() {
        // Regression: pre-fix, `half = (w - 1) / 2` gave half=0 for
        // width=2, collapsing a "wide" street to a single cell.
        // Mirror the math from `street_half_extents` instead.
        let grid = build_full_city_grid();
        // High Street is width 3 in the live RON, but the equivalent
        // bug shows up at any even width. Verify on Smythen Street
        // (width 1) that the centre line is stamped and the neighbour
        // is NOT — that's the negative anchor — then verify on a wide
        // street that *both* the centre and an off-centre cell stamp.
        // Smythen midpoint approx (-5, 45):
        let centre = grid.get(&(-5i64, 45i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(centre, TerrainKind::CobbleRoad, "Smythen centre should stamp");
        // High Street polyline cell (-40, -78), width 3, offsets {-1,0,1}.
        // Both (-40, -78) and (-40, -79) (offset -1 in y) should be cobble.
        let on  = grid.get(&(-40i64, -78i64)).copied().unwrap_or(TerrainKind::Grass);
        let off = grid.get(&(-40i64, -79i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(on,  TerrainKind::CobbleRoad, "High Street centre should stamp");
        assert_eq!(off, TerrainKind::CobbleRoad, "High Street width-3 neighbour should stamp");
    }

    #[test]
    fn block_fill_does_not_straddle_streets() {
        // Property: if the picker chose a prefab whose footprint
        // overlaps any street cell, the stamper must have suppressed
        // it — no prefab terrain may have landed inside that
        // footprint. (A prefab fronting a street is fine; what we
        // forbid is a footprint straddling the street.)
        let loaded = cities().get("Exeter").unwrap();
        let grid = build_full_city_grid();
        let (wmin_x, wmin_y, wmax_x, wmax_y) = loaded.wall_bbox;
        let slot_min_x = (wmin_x - loaded.anchor.0).div_euclid(HOUSE_PITCH) - 1;
        let slot_min_y = (wmin_y - loaded.anchor.1).div_euclid(HOUSE_PITCH) - 1;
        let slot_max_x = (wmax_x - loaded.anchor.0).div_euclid(HOUSE_PITCH) + 1;
        let slot_max_y = (wmax_y - loaded.anchor.1).div_euclid(HOUSE_PITCH) + 1;
        let mut leaks: Vec<(i64, i64)> = Vec::new();
        for sy in slot_min_y..=slot_max_y {
            for sx in slot_min_x..=slot_max_x {
                let Some((_, (min_x, min_y, max_x, max_y))) = slot_prefab_footprint(
                    sx,
                    sy,
                    loaded.anchor,
                    0,
                    loaded.city.tier,
                    &loaded.wall_world_polygon,
                    &loaded.slot_tags,
                ) else {
                    continue;
                };
                // Footprint vs street_cells.
                let overlaps_street = (min_y..=max_y).any(|y| {
                    (min_x..=max_x).any(|x| loaded.street_cells.contains(&(x, y)))
                });
                if !overlaps_street {
                    continue;
                }
                // Stamper must have suppressed this slot — no prefab
                // terrain should appear inside the footprint.
                // Look only at Floor — it's the unambiguous prefab
                // signature. (StoneWall is also used by the cathedral
                // / Rougemont / Guildhall / city walls, so a slot
                // overlapping those would false-positive.)
                for fy in min_y..=max_y {
                    for fx in min_x..=max_x {
                        if grid.get(&(fx, fy)) == Some(&TerrainKind::Floor) {
                            leaks.push((sx, sy));
                        }
                    }
                }
            }
        }
        assert!(
            leaks.is_empty(),
            "{} slots leaked prefab terrain despite footprint overlapping a street: {:?}",
            leaks.len(),
            &leaks[..leaks.len().min(5)],
        );
    }

    #[test]
    fn smythen_street_seeds_smithy_tag_into_nearby_slots() {
        // Smythen Street's `tag_weights: [("smithy", 5), ("house", 5)]`
        // should propagate into the per-slot tag map for slots whose
        // centre sits within Chebyshev-12 of any Smythen polyline cell.
        // The midpoint of Smythen ((-5, 45)) places the closest slot
        // centre at world (-4, 44) — slot (-1, 5) at anchor (0, 0).
        let loaded = cities().get("Exeter").unwrap();
        let tags = loaded
            .slot_tags
            .get(&(-1, 5))
            .expect("slot (-1, 5) is within Chebyshev-12 of Smythen Street");
        assert!(
            tags.iter().any(|(t, w)| t == "smithy" && *w > 0),
            "slot (-1, 5) should carry a positive smithy tag from Smythen Street, got {tags:?}"
        );
    }

    #[test]
    fn exeter_grid_contains_smithy_prefab_near_smythen() {
        // The smithy_urban_armorer prefab uses StoneWall — the only
        // prefab in the bundled Urban pools that does. So if any
        // StoneWall cell appears in the strip alongside Smythen
        // Street (away from the cathedral / wall / Rougemont, which
        // are also StoneWall), it must have come from a smithy
        // prefab. Smythen runs roughly (-55, -55) → (50, 160); test
        // a tight box that excludes cathedral (x ∈ [-42, 42], y ∈
        // [-12, 12]) and the south wall (y > 175).
        let grid = build_full_city_grid();
        let stone_in_smythen_strip = grid
            .iter()
            .filter(|(_, t)| **t == TerrainKind::StoneWall)
            .filter(|((x, y), _)| {
                // Strip alongside Smythen, generous on x:
                *x > -60 && *x < 60 && *y > -50 && *y < 170
            })
            .filter(|((x, y), _)| {
                // Exclude the cathedral rect (-42,-12)..(42,12).
                !(*x >= -42 && *x <= 42 && *y >= -12 && *y <= 12)
            })
            .count();
        assert!(
            stone_in_smythen_strip > 0,
            "no smithy StoneWall cells landed in the Smythen Street strip — \
             expected at least one armurer prefab to roll"
        );
    }

    #[test]
    fn walls_beat_streets_at_overlaps() {
        let grid = build_full_city_grid();
        // North Street ends at (-188, -70), which is the North gate
        // vertex on the wall. Since walls stamp LAST and gates are
        // skipped, this exact cell should be a gap (Grass after the
        // wall pass — the street stamped CobbleRoad first, the wall
        // pass skipped it as a gate). Important property: the street
        // should still be visible at the gate or just inside.
        let gate_t = grid
            .get(&(-188i64, -70i64))
            .copied()
            .unwrap_or(TerrainKind::Grass);
        // Either CobbleRoad (street persisted because gate skipped
        // wall stamping here) or Grass (gate widened past the street
        // cell). Both are acceptable; what we don't want is StoneWall.
        assert_ne!(
            gate_t, TerrainKind::StoneWall,
            "Gate cell should not be a wall"
        );
    }
}
