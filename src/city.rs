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

use crate::world::{CellState, TerrainKind, CHUNK_H, CHUNK_W, ChunkCoord};

#[derive(Debug, Deserialize)]
pub struct City {
    #[allow(dead_code)] // human label; useful for logging once wired
    pub name: String,
    pub wall: Wall,
    #[serde(default)]
    pub streets: Vec<Street>,
    #[serde(default)]
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

/// A city's parsed RON plus world-cell anchor and bbox cached at load.
/// City stamping intersects this bbox against each chunk (cities can
/// extend well beyond the `NamedSite` biome-override radius, so we
/// can't reuse that for stamping range).
pub struct LoadedCity {
    pub city: City,
    pub anchor: (i64, i64),
    /// Inclusive world-cell bbox: `(min_x, min_y, max_x, max_y)`.
    pub bbox: (i64, i64, i64, i64),
}

impl LoadedCity {
    /// True if this city has any geometry inside the given chunk's
    /// world-cell bounds. Cheap rectangle-vs-rectangle intersect.
    pub fn intersects_chunk(&self, coord: ChunkCoord) -> bool {
        let chunk_min_x = coord.cx as i64 * CHUNK_W as i64;
        let chunk_min_y = coord.cy as i64 * CHUNK_H as i64;
        let chunk_max_x = chunk_min_x + CHUNK_W as i64 - 1;
        let chunk_max_y = chunk_min_y + CHUNK_H as i64 - 1;
        let (cmin_x, cmin_y, cmax_x, cmax_y) = self.bbox;
        !(cmax_x < chunk_min_x
            || cmin_x > chunk_max_x
            || cmax_y < chunk_min_y
            || cmin_y > chunk_max_y)
    }
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
            map.insert(name, LoadedCity { city, anchor, bbox });
        }
        map
    })
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

impl City {
    /// Stamp this city's authored skeleton into the cells of one chunk.
    /// `anchor` is the city's world-cell anchor (`NamedSite.anchor_cell`).
    /// Mutates only cells whose world coords fall inside this chunk.
    ///
    /// Stamp order — bottom layer to top, so the top wins at overlaps:
    /// 1. Streets (CobbleRoad polylines, widened)
    /// 2. Paved-area landmarks (CobbleRoad fill, e.g. Cathedral Close)
    /// 3. Stone-mass landmarks (StoneWall fill, e.g. Cathedral, Rougemont)
    /// 4. Wall polygon edges (StoneWall, with gate cells skipped)
    pub fn stamp_into_chunk(
        &self,
        coord: ChunkCoord,
        cells: &mut [CellState],
        anchor: (i64, i64),
    ) {
        let chunk_origin_x = coord.cx as i64 * CHUNK_W as i64;
        let chunk_origin_y = coord.cy as i64 * CHUNK_H as i64;
        let chunk_max_x = chunk_origin_x + CHUNK_W as i64 - 1;
        let chunk_max_y = chunk_origin_y + CHUNK_H as i64 - 1;

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
        let gate_skip = self.gate_skip_cells(anchor);
        let n = self.wall.polygon.len();
        for i in 0..n {
            let a = self.wall.polygon[i];
            let b = self.wall.polygon[(i + 1) % n];
            let aw = (a.0 + anchor.0, a.1 + anchor.1);
            let bw = (b.0 + anchor.0, b.1 + anchor.1);
            for (wx, wy) in line_cells(aw, bw) {
                if gate_skip.contains(&(wx, wy)) {
                    continue;
                }
                let lx = wx - chunk_origin_x;
                let ly = wy - chunk_origin_y;
                if lx < 0 || ly < 0 || lx >= CHUNK_W as i64 || ly >= CHUNK_H as i64 {
                    continue;
                }
                set_terrain(cells, lx as usize, ly as usize, TerrainKind::StoneWall);
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
        assert!(!loaded.city.streets.is_empty());
        assert!(!loaded.city.landmarks.is_empty());
    }

    #[test]
    fn exeter_bbox_covers_known_features() {
        let loaded = cities().get("Exeter").unwrap();
        let (min_x, min_y, max_x, max_y) = loaded.bbox;
        assert!(min_x <= -398, "min_x = {min_x}, expected ≤ -398 (Exe Bridge)");
        assert!(min_y <= -548, "min_y = {min_y}, expected ≤ -548 (Rougemont)");
        assert!(max_x >= 118, "max_x = {max_x}, expected ≥ 118 (wall E)");
        assert!(max_y >= 290, "max_y = {max_y}, expected ≥ 290 (quay)");
    }

    #[test]
    fn wall_stamps_into_chunks_along_the_circuit() {
        let loaded = cities().get("Exeter").unwrap();
        let cell_count = (CHUNK_W as usize) * (CHUNK_H as usize);
        let mut cells: Vec<CellState> = (0..cell_count)
            .map(|_| CellState::with_terrain(TerrainKind::Grass))
            .collect();
        loaded.city.stamp_into_chunk(
            ChunkCoord { cx: 2, cy: -3 },
            &mut cells,
            loaded.anchor,
        );
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
            loaded.city.stamp_into_chunk(
                ChunkCoord { cx, cy },
                &mut cells,
                loaded.anchor,
            );
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
        assert!(!loaded.intersects_chunk(ChunkCoord { cx: 1000, cy: 1000 }));
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
                loaded
                    .city
                    .stamp_into_chunk(ChunkCoord { cx, cy }, &mut cells, loaded.anchor);
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
        let t = grid.get(&(0i64, 0i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(t, TerrainKind::StoneWall, "Cathedral centroid should be StoneWall");
        let t2 = grid.get(&(40i64, 10i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(t2, TerrainKind::StoneWall, "Cathedral corner should be StoneWall");
    }

    #[test]
    fn cathedral_close_is_paved_outside_cathedral() {
        let grid = build_full_city_grid();
        let t = grid.get(&(40i64, 30i64)).copied().unwrap_or(TerrainKind::Grass);
        assert_eq!(
            t, TerrainKind::CobbleRoad,
            "Close cell (40, 30) outside Cathedral rect should be CobbleRoad"
        );
    }

    #[test]
    fn bridge_and_leat_are_not_stamped() {
        let grid = build_full_city_grid();
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
    fn walls_beat_streets_at_overlaps() {
        let grid = build_full_city_grid();
        // North Street ends at (-188, -70), which is the North gate
        // vertex on the wall. Since walls stamp LAST and gates are
        // skipped, this exact cell should be a gap (Grass after the
        // wall pass — the street stamped CobbleRoad first, the wall
        // pass skipped it as a gate). Important property: the cell
        // should not become a StoneWall.
        let gate_t = grid
            .get(&(-188i64, -70i64))
            .copied()
            .unwrap_or(TerrainKind::Grass);
        assert_ne!(
            gate_t, TerrainKind::StoneWall,
            "Gate cell should not be a wall"
        );
    }
}
