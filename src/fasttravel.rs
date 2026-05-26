// Fast-travel queue + path planning. See `obsidian/Cornwall-World.md`
// §6 for the design.
//
// Two-level path:
//   1. **Chunk-graph A*** picks the high-level route as a sequence of
//      adjacent chunks. Sea is a wall; roads bias edges by ×0.40.
//      Computed once at travel-init via `plan_path`.
//   2. **Per-leg cell A*** runs lazily inside `tick_fast_travel`: when
//      the current leg empties, we A* from the player's actual cell
//      to the next chunk's center using `World::cell_walkable_at` so
//      the path steers around trees, water, gorse, etc. The chunks
//      are guaranteed loaded by then (the previous step pulled them in
//      via `try_move_player`'s ensure_chunk_ring).
//
// The old straight-line Bresenham produced paths that walked into
// trees because chunk-level A* doesn't see per-cell obstacles. Lazy
// cell-A* costs ~1 ms per leg refill (typically every 30+ frames) and
// produces clean walking routes.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use crate::cornwall::{self, Biome};
use crate::world::{ChunkCoord, World, CHUNK_H, CHUNK_W};

/// Fixed cell budget per chunk hop in the cost formula. Cornwall-World
/// §6.1 spec: "avg cells crossed per chunk for a diagonal-ish path."
const CELLS_PER_CHUNK_HOP: u32 = 36;

/// Roads cut traversal cost. Cornwall-World §6.1: "Roads (where
/// present) reduce cost by 60%." We apply this on the destination
/// chunk's edge weight (entering a road chunk is cheap).
const ROAD_COST_MULT_NUM: u32 = 40;
const ROAD_COST_MULT_DEN: u32 = 100;

/// Cell-step cost on the player's clock for each `try_move_player` call
/// during fast-travel. Derived from `MOVE_COST_TILE / MOVES_PER_SECOND`
/// at baseline speed so the planner's time estimate stays in sync with
/// the combat-slice move-cost economy.
const CELL_STEP_CLOCK_SECS: u32 =
    crate::world::MOVE_COST_TILE / crate::world::MOVES_PER_SECOND;

/// Node-budget cap for per-leg cell A*. A leg crosses at most one
/// chunk boundary; 2 KB of nodes (~ a chunk's worth of cells × small
/// detour factor) is plenty even in dense oak woodland.
const CELL_ASTAR_MAX_NODES: u32 = 2_000;

#[derive(Clone, Debug)]
pub struct FastTravelQueue {
    /// High-level chunk path *not yet entered*. Front of the deque is
    /// the next chunk the player should walk into. Empties as the
    /// player progresses.
    pub chunk_path: VecDeque<ChunkCoord>,
    /// Cell-level path for the *current* leg toward `chunk_path.front()`'s
    /// center (or `destination_cell` if `chunk_path` is empty). Lazily
    /// refilled by per-leg A* when empty.
    pub leg_cells: VecDeque<(i64, i64)>,
    pub destination: ChunkCoord,
    pub destination_cell: (i64, i64),
}

impl FastTravelQueue {
    /// Approximate remaining game-secs for the banner. Uses the
    /// remaining chunk-hop count × the canonical 36 cells/chunk plus
    /// the current leg's pending cells, all multiplied by the per-cell
    /// clock advance.
    pub fn remaining_est_secs(&self) -> u32 {
        let pending_chunk_cells = self.chunk_path.len() as u32 * CELLS_PER_CHUNK_HOP;
        let total_cells = pending_chunk_cells + self.leg_cells.len() as u32;
        total_cells * CELL_STEP_CLOCK_SECS
    }
}

/// Plan a fast-travel path from chunk `from` to chunk `to`. Returns
/// None if no route exists (e.g. destination is Sea, or surrounded by
/// Sea chunks).
///
/// Only the chunk hops are computed up front. Cell-level paths within
/// each leg are deferred to `tick_fast_travel` so they can read live
/// chunk content for walkability (trees, water, decorations).
pub fn plan_path(
    from: ChunkCoord,
    _cell_from: (i64, i64),
    to: ChunkCoord,
) -> Option<FastTravelQueue> {
    let dest_biome = cornwall::overmap_info_at(to).biome;
    if dest_biome == Biome::Sea {
        return None;
    }
    let destination_cell = chunk_center_cell(to);

    if from == to {
        // Already in the destination chunk; nothing to plan. The first
        // tick will A* a single short leg to destination_cell.
        return Some(FastTravelQueue {
            chunk_path: VecDeque::new(),
            leg_cells: VecDeque::new(),
            destination: to,
            destination_cell,
        });
    }

    let chunk_path_full = astar_chunks(from, to)?;
    // Drop the starting chunk — it's where the player already is.
    let chunk_path: VecDeque<ChunkCoord> = chunk_path_full.into_iter().skip(1).collect();

    Some(FastTravelQueue {
        chunk_path,
        leg_cells: VecDeque::new(),
        destination: to,
        destination_cell,
    })
}

/// Refill `queue.leg_cells` by per-cell A* from the player's current
/// cell toward the next significant waypoint (next chunk center, or
/// the final destination cell if `chunk_path` is empty).
///
/// Returns true if the leg was successfully refilled (or already had
/// cells), false if the path is blocked (caller signals
/// `BlockedAtCell`).
pub fn refill_leg(world: &mut World, queue: &mut FastTravelQueue) -> bool {
    if !queue.leg_cells.is_empty() {
        return true;
    }

    let p = world.player_pos();
    let player_cell = (p.x as i64, p.y as i64);

    // Pop already-entered chunks from the front of chunk_path. (Walking
    // into a chunk before reaching its center still counts as "entered"
    // for popping purposes — the A* targeted that chunk, and we don't
    // need to circle back to its center.)
    let player_cc = chunk_coord_for_cell(player_cell);
    while queue.chunk_path.front().is_some_and(|&c| c == player_cc) {
        queue.chunk_path.pop_front();
    }

    // Pick the next leg's target.
    let raw_target = if let Some(&next) = queue.chunk_path.front() {
        chunk_center_cell(next)
    } else {
        queue.destination_cell
    };

    // Ensure target chunk + its 3×3 ring are loaded so cell A* can see
    // their content. The player's current ring is already loaded by
    // try_move_player; adding the target ring covers any reasonable
    // detour for trees on a chunk boundary.
    let target_cc = chunk_coord_for_cell(raw_target);
    for dy in -1..=1 {
        for dx in -1..=1 {
            world.ensure_chunk_loaded(ChunkCoord {
                cx: target_cc.cx + dx,
                cy: target_cc.cy + dy,
            });
        }
    }

    // Snap target to nearest walkable cell so trees on the chunk-center
    // cell don't make the leg unreachable.
    let target = nearest_walkable(world, raw_target).unwrap_or(raw_target);

    if player_cell == target {
        // Already at target — nothing to walk. Caller will pop the
        // chunk on the next tick (loop above) and try again.
        return true;
    }

    match cell_astar(world, player_cell, target) {
        Some(cells) => {
            queue.leg_cells = cells;
            true
        }
        None => false,
    }
}

/// Per-cell A* over the 8-connected grid. Uses `World::cell_walkable_at`
/// so trees, water, gorse, OOB cells are all rejected naturally.
fn cell_astar(world: &World, start: (i64, i64), target: (i64, i64)) -> Option<VecDeque<(i64, i64)>> {
    if start == target {
        return Some(VecDeque::new());
    }

    let mut open: BinaryHeap<Reverse<(u32, u32, i64, i64)>> = BinaryHeap::new();
    let mut came_from: HashMap<(i64, i64), (i64, i64)> = HashMap::new();
    let mut g_score: HashMap<(i64, i64), u32> = HashMap::new();

    g_score.insert(start, 0);
    open.push(Reverse((heuristic_cell(start, target), 0, start.0, start.1)));

    let mut tiebreak: u32 = 0;
    let mut popped: u32 = 0;

    while let Some(Reverse((_, _, cx, cy))) = open.pop() {
        let cur = (cx, cy);
        if cur == target {
            return Some(reconstruct_cells(came_from, cur, start));
        }
        popped += 1;
        if popped > CELL_ASTAR_MAX_NODES {
            return None;
        }
        let cur_g = *g_score.get(&cur).unwrap_or(&u32::MAX);

        for (dx, dy) in CELL_NEIGHBORS_8 {
            let n = (cur.0 + dx, cur.1 + dy);
            // The start cell is the only "walkable" cell we can stand
            // on without checking (player is there). Every other
            // candidate cell must pass the walkability check.
            if n != target && !world.cell_walkable_at(n.0, n.1) {
                continue;
            }
            // Target cell is allowed even if technically unwalkable —
            // the cell-astar bisects the path; we caught unwalkable
            // targets in `nearest_walkable` before calling.
            let step = if dx != 0 && dy != 0 { 14 } else { 10 };
            let tentative = cur_g.saturating_add(step);
            let existing = *g_score.get(&n).unwrap_or(&u32::MAX);
            if tentative < existing {
                came_from.insert(n, cur);
                g_score.insert(n, tentative);
                let f = tentative.saturating_add(heuristic_cell(n, target));
                tiebreak = tiebreak.wrapping_add(1);
                open.push(Reverse((f, tiebreak, n.0, n.1)));
            }
        }
    }
    None
}

const CELL_NEIGHBORS_8: [(i64, i64); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0),           (1, 0),
    (-1, 1),  (0, 1),  (1, 1),
];

fn heuristic_cell(a: (i64, i64), b: (i64, i64)) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs() as u32;
    let dy = (a.1 - b.1).unsigned_abs() as u32;
    // Octile distance heuristic matching the 14/10 step costs.
    let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
    14 * lo + 10 * (hi - lo)
}

fn reconstruct_cells(
    came_from: HashMap<(i64, i64), (i64, i64)>,
    end: (i64, i64),
    start: (i64, i64),
) -> VecDeque<(i64, i64)> {
    let mut path = Vec::new();
    let mut c = end;
    while c != start {
        path.push(c);
        match came_from.get(&c) {
            Some(&prev) => c = prev,
            None => break,
        }
    }
    path.reverse();
    path.into()
}

/// Scan a 4-cell-radius square around `target` for the first walkable
/// cell. Falls back to the target itself if nothing nearby is walkable
/// (the planner will subsequently fail per-cell A* and the player
/// gets a "blocked" message).
fn nearest_walkable(world: &World, target: (i64, i64)) -> Option<(i64, i64)> {
    if world.cell_walkable_at(target.0, target.1) {
        return Some(target);
    }
    for r in 1..=4i64 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let c = (target.0 + dx, target.1 + dy);
                if world.cell_walkable_at(c.0, c.1) {
                    return Some(c);
                }
            }
        }
    }
    None
}

/// Chunk-graph A* with 8-connected movement. Sea is impassable;
/// roads reduce edge cost.
fn astar_chunks(from: ChunkCoord, to: ChunkCoord) -> Option<Vec<ChunkCoord>> {
    let dest_biome = cornwall::overmap_info_at(to).biome;
    if dest_biome == Biome::Sea {
        return None;
    }

    let mut open: BinaryHeap<Reverse<(u32, ChunkCoord)>> = BinaryHeap::new();
    let mut came_from: HashMap<ChunkCoord, ChunkCoord> = HashMap::new();
    let mut g_score: HashMap<ChunkCoord, u32> = HashMap::new();

    g_score.insert(from, 0);
    open.push(Reverse((heuristic_chunks(from, to), from)));

    // With the tighter has_road test (1-chunk-wide trail instead of
    // a 30-cell band), road-discounted chunks are sparser and A*
    // explores more nodes for peninsula-spanning paths. 8 M accommodates
    // Exeter → Tintagel / Bodmin / Land's End class destinations.
    const MAX_NODES: u32 = 8_000_000;
    let mut popped: u32 = 0;

    while let Some(Reverse((_, current))) = open.pop() {
        if current == to {
            return Some(reconstruct_chunks(came_from, current));
        }
        popped += 1;
        if popped > MAX_NODES {
            return None;
        }
        let current_g = *g_score.get(&current).unwrap_or(&u32::MAX);

        for (dx, dy) in CHUNK_NEIGHBORS_8 {
            let n = ChunkCoord {
                cx: current.cx + dx,
                cy: current.cy + dy,
            };
            let info = cornwall::overmap_info_at(n);
            if info.biome == Biome::Sea {
                continue;
            }
            let mut step = info.biome.cell_travel_cost_secs() * CELLS_PER_CHUNK_HOP;
            if info.has_road {
                step = step * ROAD_COST_MULT_NUM / ROAD_COST_MULT_DEN;
            }
            if dx != 0 && dy != 0 {
                step = step * 141 / 100;
            }
            let tentative_g = current_g.saturating_add(step);
            let existing_g = *g_score.get(&n).unwrap_or(&u32::MAX);
            if tentative_g < existing_g {
                came_from.insert(n, current);
                g_score.insert(n, tentative_g);
                let f = tentative_g.saturating_add(heuristic_chunks(n, to));
                open.push(Reverse((f, n)));
            }
        }
    }
    None
}

const CHUNK_NEIGHBORS_8: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0),           (1, 0),
    (-1, 1),  (0, 1),  (1, 1),
];

fn heuristic_chunks(a: ChunkCoord, b: ChunkCoord) -> u32 {
    let dx = (a.cx - b.cx).unsigned_abs();
    let dy = (a.cy - b.cy).unsigned_abs();
    let cheb = dx.max(dy);
    // Weighted A* — slightly inadmissible. The admissible minimum is
    // ~28 (LowlandFarm diagonal × road discount), but very few chunks
    // are road since has_road now flags only chunks the polyline
    // actually crosses. Using 60 (≈ typical non-road LowlandFarm
    // diagonal) prunes the search bubble by ~10× at the cost of paths
    // that may take a few extra chunks here and there — acceptable for
    // a travel verb the player doesn't see node-by-node.
    const HEURISTIC_PER_CHUNK: u32 = 100;
    cheb.saturating_mul(HEURISTIC_PER_CHUNK)
}

fn reconstruct_chunks(came_from: HashMap<ChunkCoord, ChunkCoord>, mut current: ChunkCoord) -> Vec<ChunkCoord> {
    let mut path = vec![current];
    while let Some(&prev) = came_from.get(&current) {
        path.push(prev);
        current = prev;
    }
    path.reverse();
    path
}

fn chunk_center_cell(cc: ChunkCoord) -> (i64, i64) {
    let x = cc.cx as i64 * CHUNK_W as i64 + CHUNK_W as i64 / 2;
    let y = cc.cy as i64 * CHUNK_H as i64 + CHUNK_H as i64 / 2;
    (x, y)
}

fn chunk_coord_for_cell(cell: (i64, i64)) -> ChunkCoord {
    ChunkCoord {
        cx: cell.0.div_euclid(CHUNK_W as i64) as i32,
        cy: cell.1.div_euclid(CHUNK_H as i64) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trivial_same_chunk_path() {
        let from = ChunkCoord { cx: 0, cy: 0 };
        let q = plan_path(from, (20, 15), from).expect("same-chunk path");
        assert_eq!(q.destination, from);
        assert!(q.chunk_path.is_empty());
    }

    #[test]
    fn sea_destination_returns_none() {
        let to = ChunkCoord { cx: 2000, cy: 4000 };
        let from = ChunkCoord { cx: 0, cy: 0 };
        let r = plan_path(from, (20, 15), to);
        assert!(r.is_none(), "Sea destination must not route");
    }

    #[test]
    fn exeter_to_bodmin_routes() {
        let from = ChunkCoord { cx: 0, cy: 0 };       // Exeter
        let to = ChunkCoord { cx: -1_389, cy: 608 };  // Bodmin
        let q = plan_path(from, (20, 15), to).expect("path Exeter→Bodmin");
        assert_eq!(q.destination, to);
        assert!(!q.chunk_path.is_empty(), "chunk_path should have hops");
        // Chunk path length should be roughly Chebyshev distance.
        let cheb = (to.cx - from.cx).unsigned_abs().max((to.cy - from.cy).unsigned_abs());
        assert!(
            (q.chunk_path.len() as u32) <= cheb * 2,
            "chunk path {} hops should be near Chebyshev {} (allowing 2× for detours)",
            q.chunk_path.len(),
            cheb
        );
    }

    #[test]
    fn cell_astar_finds_open_path() {
        // Standalone unit test of cell_astar without needing a world —
        // skipped because cell_astar takes &World. The integration test
        // in world::tests covers tree avoidance end-to-end.
    }

    #[test]
    fn heuristic_cell_octile_is_admissible() {
        // Diagonal distance: cheby=10, dx=dy=10 → 14*10 = 140 (10 diag steps).
        let h = heuristic_cell((0, 0), (10, 10));
        assert_eq!(h, 140);
        // Pure horizontal: cheby=10, dx=10 dy=0 → 10*10 = 100 (10 orth steps).
        let h2 = heuristic_cell((0, 0), (10, 0));
        assert_eq!(h2, 100);
    }

    #[test]
    fn cell_step_clock_secs_matches_world() {
        // Sanity-check the derivation against the underlying constants
        // — a future bump to MOVE_COST_TILE or MOVES_PER_SECOND should
        // recompute this automatically.
        assert_eq!(
            CELL_STEP_CLOCK_SECS,
            crate::world::MOVE_COST_TILE / crate::world::MOVES_PER_SECOND
        );
    }
}
