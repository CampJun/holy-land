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

use crate::world::{CellState, GroundCover, TerrainKind, CHUNK_H, CHUNK_W, ChunkCoord};

#[derive(Debug, Deserialize)]
pub struct City {
    #[allow(dead_code)] // human label; useful for logging once wired
    pub name: String,
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
        );
    }
}

fn rect_intersects(a: (i64, i64, i64, i64), b: (i64, i64, i64, i64)) -> bool {
    !(a.2 < b.0 || a.0 > b.2 || a.3 < b.1 || a.1 > b.3)
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
                stamp_one_house_slot(
                    slot_x,
                    slot_y,
                    anchor,
                    world_seed,
                    wall_world_polygon,
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
        let half = (street.width as i64).saturating_sub(1) / 2;
        for window in street.polyline.windows(2) {
            let aw = (window[0].0 + anchor.0, window[0].1 + anchor.1);
            let bw = (window[1].0 + anchor.0, window[1].1 + anchor.1);
            for (wx, wy) in line_cells(aw, bw) {
                // Square neighborhood of side `width` centered on the
                // line cell — cheap and visually reads as "wider".
                for dy in -half..=half {
                    for dx in -half..=half {
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
/// single house with its perimeter alleys. Pitch 8 + houses 5..=7
/// yields 1..=3 cell alleys between buildings — medieval-dense.
const HOUSE_PITCH: i64 = 8;
const HOUSE_MIN: i64 = 5;
const HOUSE_MAX: i64 = 7;
/// Fraction (out of 256) of slots that stay empty plots (gardens,
/// churchyards we haven't authored yet, ruins).
const EMPTY_SLOT_THRESHOLD: u64 = 50;
const CITY_SEED_SALT: u64 = 0xE7_E7_E7_E7_E7_E7_E7_E7;

fn stamp_one_house_slot(
    slot_x: i64,
    slot_y: i64,
    anchor: (i64, i64),
    world_seed: u64,
    wall_world_polygon: &[(i64, i64)],
    chunk_origin_x: i64,
    chunk_origin_y: i64,
    chunk_max_x: i64,
    chunk_max_y: i64,
    cells: &mut [CellState],
) {
    // Slot origin in world coords (top-left of the slot).
    let slot_origin_x = slot_x * HOUSE_PITCH + anchor.0;
    let slot_origin_y = slot_y * HOUSE_PITCH + anchor.1;
    let slot_inner_min_x = slot_origin_x + 1;
    let slot_inner_min_y = slot_origin_y + 1;
    let slot_inner_max_x = slot_origin_x + HOUSE_PITCH - 1;
    let slot_inner_max_y = slot_origin_y + HOUSE_PITCH - 1;

    // Containment: all 4 corners of the slot's usable interior must
    // be inside the wall polygon. Cheaper than testing the house
    // rectangle and avoids houses jutting through walls.
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
        return;
    }

    // Slot decision hash.
    let h = hash3(slot_x as u64, slot_y as u64, world_seed ^ CITY_SEED_SALT);
    if (h & 0xFF) < EMPTY_SLOT_THRESHOLD {
        return;
    }

    let span = (HOUSE_MAX - HOUSE_MIN + 1) as u64;
    let house_w = HOUSE_MIN + ((h >> 8) % span) as i64;
    let house_h = HOUSE_MIN + ((h >> 16) % span) as i64;
    let slack_x = (HOUSE_PITCH - 1 - house_w).max(0) as u64;
    let slack_y = (HOUSE_PITCH - 1 - house_h).max(0) as u64;
    let off_x = if slack_x > 0 { ((h >> 24) % slack_x) as i64 } else { 0 };
    let off_y = if slack_y > 0 { ((h >> 32) % slack_y) as i64 } else { 0 };

    let house_min_x = slot_inner_min_x + off_x;
    let house_min_y = slot_inner_min_y + off_y;
    let house_max_x = house_min_x + house_w - 1;
    let house_max_y = house_min_y + house_h - 1;

    // Stamp cells of this house that fall inside the current chunk.
    // Only overwrite Grass — streets / landmarks / walls already
    // stamped and must keep precedence.
    let clip_min_x = house_min_x.max(chunk_origin_x);
    let clip_min_y = house_min_y.max(chunk_origin_y);
    let clip_max_x = house_max_x.min(chunk_max_x);
    let clip_max_y = house_max_y.min(chunk_max_y);
    if clip_min_x > clip_max_x || clip_min_y > clip_max_y {
        return;
    }
    for wy in clip_min_y..=clip_max_y {
        for wx in clip_min_x..=clip_max_x {
            let lx = (wx - chunk_origin_x) as usize;
            let ly = (wy - chunk_origin_y) as usize;
            let idx = ly * (CHUNK_W as usize) + lx;
            if cells[idx].terrain != TerrainKind::Grass {
                continue;
            }
            let is_perim = wx == house_min_x
                || wx == house_max_x
                || wy == house_min_y
                || wy == house_max_y;
            set_terrain(
                cells,
                lx,
                ly,
                if is_perim { TerrainKind::WoodWall } else { TerrainKind::Floor },
            );
        }
    }
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
