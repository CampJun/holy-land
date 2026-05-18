// Seeded chunk generation for the slice-1 forest tile.
//
// Authored skeleton (every seed identical for the (0, 0) chunk):
//   - A stream enters from the north edge at column 12 and snakes south
//     into a pond near the south-east. Pond is an ellipse centered at
//     (28, 22), radii (5, 3). The pond's outer ring becomes SandShore.
//   - Six "skeleton" trees on the pond's north-east perimeter.
//
// Seeded population (varies by chunk_seed = hash(world_seed, coord)):
//   - 10-20 extra TreeTrunk cells in the chunk's outer ring (cols/rows
//     0-4 and CHUNK_W-5..CHUNK_W-1 etc.) on grass cells only.
//   - 3-5 herb-patch items placed on grass cells.
//   - Per-grass-cell debris rolls per the table below.
//
// Cell debris table (Grass cells; rolls are independent):
//   - 50% chance: 1-3 twigs (5g each)
//   - 30%       : 1-2 sticks (50g each)
//   - 25%       : 1-2 firewood (500g each)
//   - 40%       : 1-2 grass blades (2g each)
//   - 20%       : 1 stone (200g)
//   - 15%       : 1 moss patch (10g)
//   - 10% (next to water) : 1 mud (300g)
//
// Determinism: chunk_rng(coord, world_seed) hashes the chunk coord into
// the world seed so every chunk regenerates identically across reloads
// and across machines. Phase-11 only generates chunk (0, 0); the same
// function will produce phase-12+ wilderness chunks when the player
// crosses chunk boundaries.

use crate::items::{ItemInstance, ItemKind, ItemMetadata};
use crate::skill::Rng;
use crate::world::{CellState, Chunk, ChunkCoord, TerrainKind, CHUNK_H, CHUNK_W};

pub fn generate_chunk(coord: ChunkCoord, world_seed: u64) -> Chunk {
    let mut rng = chunk_rng(coord, world_seed);

    // Step 1: skeleton terrain. Every cell starts as Grass; we overlay
    // the stream + pond + trees in order so later passes override
    // earlier ones (e.g. pond on top of grass).
    let mut cells: Vec<CellState> = (0..(CHUNK_W * CHUNK_H))
        .map(|_| CellState::with_terrain(TerrainKind::Grass))
        .collect();

    apply_stream(&mut cells);
    apply_pond_and_shore(&mut cells);
    apply_skeleton_trees(&mut cells);

    // Step 2: extra trees in the outer ring (seeded count, seeded
    // positions on grass).
    let extra_trees = 10 + (rng.next_u32() % 11) as u32; // 10..=20
    let mut placed = 0u32;
    let mut attempts = 0u32;
    while placed < extra_trees && attempts < 200 {
        attempts += 1;
        let x = (rng.next_u32() % CHUNK_W) as u32;
        let y = (rng.next_u32() % CHUNK_H) as u32;
        if !is_outer_ring(x, y) {
            continue;
        }
        let idx = cell_idx(x, y);
        if cells[idx].terrain != TerrainKind::Grass {
            continue;
        }
        cells[idx].terrain = TerrainKind::TreeTrunk;
        placed += 1;
    }

    // Step 3: herb patches (3-5) on grass cells anywhere on the map.
    let herb_count = 3 + (rng.next_u32() % 3) as u32; // 3..=5
    let mut herb_placed = 0u32;
    let mut herb_attempts = 0u32;
    while herb_placed < herb_count && herb_attempts < 200 {
        herb_attempts += 1;
        let x = (rng.next_u32() % CHUNK_W) as u32;
        let y = (rng.next_u32() % CHUNK_H) as u32;
        let idx = cell_idx(x, y);
        if cells[idx].terrain != TerrainKind::Grass {
            continue;
        }
        if !cells[idx].items.is_empty() {
            continue;
        }
        cells[idx].items.push(ItemInstance::unique(
            ItemKind::Herb,
            10,
            None,
            ItemMetadata::None,
        ));
        herb_placed += 1;
    }

    // Step 4: per-grass-cell debris rolls. Mud roll needs to know
    // whether ANY adjacent cell is water; precompute that into a bool
    // grid first so we don't need to borrow `cells` immutably during
    // the per-cell mutable iteration.
    let near_water = compute_near_water_grid(&cells);
    for y in 0..CHUNK_H {
        for x in 0..CHUNK_W {
            let idx = cell_idx(x, y);
            if cells[idx].terrain != TerrainKind::Grass {
                continue;
            }
            // Skip cells that already have items (herbs); keeps the
            // here-line readable and avoids "tent + axe + 4 other things
            // already on cell" auto-clutter.
            if !cells[idx].items.is_empty() {
                continue;
            }
            roll_debris(&mut cells[idx].items, &mut rng, near_water[idx]);
        }
    }

    Chunk {
        coord,
        cells,
        dirty: false,
    }
}

/// Hash chunk coord into the world seed so each chunk's RNG is unique
/// but deterministic. The constants are the SplitMix64 / Xorshift hash
/// mixers commonly used for spatial seeding.
fn chunk_rng(coord: ChunkCoord, world_seed: u64) -> Rng {
    let cx = coord.cx as u64;
    let cy = coord.cy as u64;
    let hash = world_seed
        .wrapping_add(cx.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(cy.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_mul(0x94D0_49BB_1331_11EB);
    Rng::from_world_seed(hash)
}

fn cell_idx(x: u32, y: u32) -> usize {
    (y * CHUNK_W + x) as usize
}

/// True if `(x, y)` is within 4 cells of any chunk edge — phase-11's
/// "outer ring" where extra trees spawn.
fn is_outer_ring(x: u32, y: u32) -> bool {
    x < 5 || y < 5 || x >= CHUNK_W.saturating_sub(5) || y >= CHUNK_H.saturating_sub(5)
}

// ---- Authored skeleton ----

/// Hand-placed stream path. Enters at the north edge, snakes south,
/// curving slightly east on the way down so it joins the pond's
/// north-west arc.
fn apply_stream(cells: &mut [CellState]) {
    let path: &[(u32, u32)] = &[
        // Vertical run from north edge.
        (12, 0),
        (12, 1),
        (12, 2),
        (12, 3),
        (12, 4),
        (13, 4),
        (13, 5),
        (13, 6),
        (14, 6),
        (14, 7),
        (15, 7),
        (15, 8),
        (16, 8),
        (16, 9),
        (17, 9),
        (17, 10),
        (18, 10),
        (18, 11),
        (19, 11),
        (20, 11),
        (21, 11),
        (22, 11),
        (22, 12),
        (23, 12),
        (23, 13),
        (24, 13),
        (24, 14),
        // Stream merges into the pond at approximately (25, 15).
    ];
    for &(x, y) in path {
        if x < CHUNK_W && y < CHUNK_H {
            cells[cell_idx(x, y)].terrain = TerrainKind::StreamWater;
        }
    }
}

/// Pond is an ellipse centered at (28, 22), semi-axes 5 horizontal, 3
/// vertical. The outer ring (one cell beyond the pond perimeter) becomes
/// SandShore.
fn apply_pond_and_shore(cells: &mut [CellState]) {
    let (cx, cy) = (28i32, 22i32);
    let (rx, ry) = (5i32, 3i32);
    for y in 0..CHUNK_H as i32 {
        for x in 0..CHUNK_W as i32 {
            let dx = x - cx;
            let dy = y - cy;
            let inside = (dx * dx * ry * ry + dy * dy * rx * rx) <= (rx * rx * ry * ry);
            let shore = !inside
                && (dx * dx * ry * ry + dy * dy * rx * rx)
                    <= ((rx + 1) * (rx + 1) * (ry + 1) * (ry + 1));
            if inside {
                cells[cell_idx(x as u32, y as u32)].terrain = TerrainKind::PondWater;
            } else if shore
                && cells[cell_idx(x as u32, y as u32)].terrain == TerrainKind::Grass
            {
                cells[cell_idx(x as u32, y as u32)].terrain = TerrainKind::SandShore;
            }
        }
    }
}

/// Six water-loving trees on the pond's north-east perimeter. Hand-
/// placed; same every seed.
fn apply_skeleton_trees(cells: &mut [CellState]) {
    let trees: &[(u32, u32)] = &[(33, 19), (34, 20), (34, 22), (33, 25), (31, 18), (26, 18)];
    for &(x, y) in trees {
        if x < CHUNK_W && y < CHUNK_H {
            let idx = cell_idx(x, y);
            if cells[idx].terrain == TerrainKind::Grass {
                cells[idx].terrain = TerrainKind::TreeTrunk;
            }
        }
    }
}

// ---- Debris rolls ----

/// Roll the debris items for a single grass cell per the table in this
/// module's header comment. Mutates `out` in place; advances `rng`.
/// `near_water` is the precomputed adjacency flag for this cell.
fn roll_debris(out: &mut Vec<ItemInstance>, rng: &mut Rng, near_water: bool) {
    if rng.next_u32() % 100 < 50 {
        let n = 1 + (rng.next_u32() % 3) as u16; // 1..=3
        out.push(ItemInstance::stack(ItemKind::Twig, n, 5, None, ItemMetadata::None));
    }
    if rng.next_u32() % 100 < 30 {
        let n = 1 + (rng.next_u32() % 2) as u16; // 1..=2
        out.push(ItemInstance::stack(ItemKind::Stick, n, 50, None, ItemMetadata::None));
    }
    if rng.next_u32() % 100 < 25 {
        let n = 1 + (rng.next_u32() % 2) as u16; // 1..=2
        out.push(ItemInstance::stack(
            ItemKind::Firewood,
            n,
            500,
            None,
            ItemMetadata::None,
        ));
    }
    if rng.next_u32() % 100 < 40 {
        let n = 1 + (rng.next_u32() % 2) as u16; // 1..=2
        out.push(ItemInstance::stack(
            ItemKind::GrassBlade,
            n,
            2,
            None,
            ItemMetadata::None,
        ));
    }
    if rng.next_u32() % 100 < 20 {
        out.push(ItemInstance::stack(
            ItemKind::Stone,
            1,
            200,
            None,
            ItemMetadata::None,
        ));
    }
    if rng.next_u32() % 100 < 15 {
        out.push(ItemInstance::stack(
            ItemKind::MossPatch,
            1,
            10,
            None,
            ItemMetadata::None,
        ));
    }
    if rng.next_u32() % 100 < 10 && near_water {
        out.push(ItemInstance::stack(
            ItemKind::Mud,
            1,
            300,
            None,
            ItemMetadata::None,
        ));
    }
}

/// Compute "is this cell adjacent to any water cell" for every cell in
/// the chunk. Returned as a flat Vec<bool> sized CHUNK_W*CHUNK_H.
fn compute_near_water_grid(cells: &[CellState]) -> Vec<bool> {
    let mut out = vec![false; cells.len()];
    for y in 0..CHUNK_H as i32 {
        for x in 0..CHUNK_W as i32 {
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= CHUNK_W as i32 || ny >= CHUNK_H as i32 {
                        continue;
                    }
                    let t = cells[cell_idx(nx as u32, ny as u32)].terrain;
                    if matches!(t, TerrainKind::StreamWater | TerrainKind::PondWater) {
                        out[cell_idx(x as u32, y as u32)] = true;
                        break;
                    }
                }
                if out[cell_idx(x as u32, y as u32)] {
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_zero_zero_has_grass_spawn() {
        let chunk = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 0xC0FFEE);
        // Player spawn cell is (20, 15); must be walkable Grass for
        // every seed (skeleton skipping that cell).
        let t = chunk.cells[cell_idx(20, 15)].terrain;
        assert_eq!(t, TerrainKind::Grass, "spawn cell must be walkable");
    }

    #[test]
    fn chunk_has_stream_and_pond() {
        let chunk = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 0xC0FFEE);
        let stream_count = chunk
            .cells
            .iter()
            .filter(|c| c.terrain == TerrainKind::StreamWater)
            .count();
        let pond_count = chunk
            .cells
            .iter()
            .filter(|c| c.terrain == TerrainKind::PondWater)
            .count();
        assert!(stream_count >= 5, "stream should be visible");
        assert!(pond_count >= 10, "pond should be visible");
    }

    #[test]
    fn chunk_has_trees() {
        let chunk = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 0xC0FFEE);
        let tree_count = chunk
            .cells
            .iter()
            .filter(|c| c.terrain == TerrainKind::TreeTrunk)
            .count();
        // Skeleton 6 + 10..=20 extra = at least 12 trees on any seed.
        assert!(tree_count >= 12, "expected >= 12 trees, got {}", tree_count);
    }

    #[test]
    fn chunk_has_herbs() {
        let chunk = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 0xC0FFEE);
        let herb_count: usize = chunk
            .cells
            .iter()
            .map(|c| c.items.iter().filter(|i| i.kind == ItemKind::Herb).count())
            .sum();
        assert!((3..=5).contains(&herb_count), "herb count out of range: {}", herb_count);
    }

    #[test]
    fn chunk_has_firewood_somewhere() {
        let chunk = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 0xC0FFEE);
        let fw_total: u32 = chunk
            .cells
            .iter()
            .flat_map(|c| c.items.iter())
            .filter(|i| i.kind == ItemKind::Firewood)
            .map(|i| i.count as u32)
            .sum();
        assert!(
            fw_total >= 2,
            "first-fire requires >=2 firewood reachable; got {}",
            fw_total
        );
    }

    #[test]
    fn generation_is_deterministic_for_same_seed() {
        let a = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 0xC0FFEE);
        let b = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 0xC0FFEE);
        // Compare per-cell terrain + item kinds.
        for (ca, cb) in a.cells.iter().zip(b.cells.iter()) {
            assert_eq!(ca.terrain, cb.terrain);
            assert_eq!(ca.items.len(), cb.items.len());
            for (ia, ib) in ca.items.iter().zip(cb.items.iter()) {
                assert_eq!(ia.kind, ib.kind);
                assert_eq!(ia.count, ib.count);
            }
        }
    }

    #[test]
    fn different_seeds_produce_different_chunks() {
        let a = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 1);
        let b = generate_chunk(ChunkCoord { cx: 0, cy: 0 }, 2);
        let mut diff = 0;
        for (ca, cb) in a.cells.iter().zip(b.cells.iter()) {
            if ca.terrain != cb.terrain {
                diff += 1;
            }
            if ca.items.len() != cb.items.len() {
                diff += 1;
            }
        }
        // Skeleton terrain is identical across seeds; differences come
        // from extra trees + herbs + debris rolls. At least a couple
        // cells should differ.
        assert!(diff > 5, "different seeds should produce different layouts");
    }
}
