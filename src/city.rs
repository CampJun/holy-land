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
#[allow(dead_code)] // wired in the streets+landmarks card
pub struct Street {
    pub name: String,
    pub polyline: Vec<(i64, i64)>,
    pub width: u16,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // wired in the streets+landmarks card
pub struct Landmark {
    pub name: String,
    pub footprint: Footprint,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub enum Footprint {
    Rect((i64, i64), (i64, i64)),
    Polygon(Vec<(i64, i64)>),
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
    pub fn stamp_into_chunk(
        &self,
        coord: ChunkCoord,
        cells: &mut [CellState],
        anchor: (i64, i64),
    ) {
        let gate_skip = self.gate_skip_cells(anchor);
        let chunk_origin_x = coord.cx as i64 * CHUNK_W as i64;
        let chunk_origin_y = coord.cy as i64 * CHUNK_H as i64;

        // Walk every polygon edge (closing last → first), Bresenham each
        // segment, and stamp cells that land inside this chunk.
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
                let idx = (ly as usize) * (CHUNK_W as usize) + (lx as usize);
                cells[idx].terrain = TerrainKind::StoneWall;
                cells[idx].tree_species = None;
                cells[idx].decoration = crate::flora::Decoration::None;
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
        // (1000, 1000) is hundreds of chunks away from Exeter.
        assert!(!loaded.intersects_chunk(ChunkCoord { cx: 1000, cy: 1000 }));
        // (0, 0) is the anchor; must intersect.
        assert!(loaded.intersects_chunk(ChunkCoord { cx: 0, cy: 0 }));
    }
}
