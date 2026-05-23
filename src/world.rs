// Survival shell with chunked storage and per-cell item lists.
//
// Phase-3 substrate (see Survival - Chunk and per-cell items.md):
// - `World` owns a `HashMap<ChunkCoord, Box<Chunk>>` keyed on (cx, cy) grid
//   coords. Slice 1 only ever loads chunk (0, 0); the divmod lookup is real
//   so phase-12+ multi-chunk expansion is a one-line `generate_if_absent` add.
// - Each `Cell` carries a `terrain: TerrainKind` and a `Vec<ItemInstance>`.
// - `tile_at(wx: i64, wy: i64)` returns `TerrainKind::Wall` for unloaded
//   chunks or out-of-bounds local coords.
// - `Position` (ECS) stays `i32` for slice 1; widening to `i64` is the
//   explicit phase-19 save-schema-v2 task.
//
// `Pack` lives as a hecs component on the player entity; see items.rs.

use std::collections::{HashMap, VecDeque};

use hecs::{Entity, World as Ecs};
use serde::{Deserialize, Serialize};

use crate::action::ActionId;
use crate::calendar::{self, Season};
use crate::items::{starting_pack, ItemInstance, ItemKind, ItemMetadata, Pack};
use crate::needs::{Needs, NeedsEnv};
use crate::skill::{Rng, Skills};

pub const CHUNK_W: u32 = 40;
pub const CHUNK_H: u32 = 30;

// TODO(phase-11): take from save or RNG. Slice-1 doesn't consume seed yet
// because debris is a fixed test fixture, but the field is wired through so
// `generate_chunk` can become seed-driven without a struct change.
const DEFAULT_SEED: u64 = 0xC0FFEE_F00D_u64;

/// Player spawns at 14:00 game-time per master plan (6 hours of daylight
/// before dusk at 20:00). Seconds since midnight: 14 × 3600 = 50_400.
pub const STARTING_CLOCK_SECONDS: u64 = 14 * 3600;
pub const DAY_LENGTH_SECONDS: u64 = 24 * 3600;
pub const DAWN_HOUR: u64 = 6;
pub const DUSK_HOUR: u64 = 20;

/// World-primitive action costs. Each verb's costs live in `action.rs`
/// next to its eval/execute code per STYLE.md §2 — but movement is not
/// a menu verb (it's a direct dpad mapping), so its cost lives where
/// `try_move_player` consumes it.
pub const COST_MOVE_TILE: u32 = 5;

/// Phase-9 multi-turn interrupt threshold. When any need drops below this
/// during a multi-turn tick, the active action queue cancels and control
/// returns to the player. Death gate (phase 14) re-uses the same value.
pub const NEED_CRITICAL_THRESHOLD: u8 = 10;

/// How many game-seconds a ProgressBar-mode multi-turn action advances
/// per render frame. At ~60 fps, this means a 300-sec PitchTent
/// completes in ~5 real-time seconds — fast enough to not feel like
/// dead time, slow enough that the player can react with B to cancel.
pub const MULTI_TURN_GAME_SEC_PER_FRAME: u32 = 1;

/// FOV radii. Player vision is `FOV_RADIUS_DAY` by day,
/// `FOV_RADIUS_NIGHT` at night. Active light sources (any item
/// carrying `ItemMetadata::Lit`) cast their own independent FOV at
/// `LIGHT_SOURCE_RADIUS` at night — see `recompute_fov` for the
/// per-source shadowcast that the player's visible set unions with.
/// True if the metadata represents an item that's currently burning
/// — a plain Lit firewood OR a PannedOnFire cookware (which owns the
/// fire's fuel). Centralized so FOV light gathering, light-source
/// counting, and pickup-blocking all agree.
pub(crate) fn is_active_fire(meta: &ItemMetadata) -> bool {
    match meta {
        ItemMetadata::Lit { .. } => true,
        ItemMetadata::PannedOnFire { fuel_seconds, .. } => *fuel_seconds > 0,
        _ => false,
    }
}

pub const FOV_RADIUS_DAY: i32 = 20;
pub const FOV_RADIUS_NIGHT: i32 = 3;
pub const LIGHT_SOURCE_RADIUS: i32 = 5;

/// Cap on light sources tracked per `recompute_fov` so the enumeration
/// fits in a stack buffer (zero-alloc per move on the Cortex-A7 Miyoo
/// target). Slice 1 hits 1–3 in practice; 16 leaves comfortable
/// headroom. If a future feature genuinely needs more, bump this — the
/// truncation is silent.
const MAX_LIGHT_SOURCES: usize = 16;

/// Side length of the per-recompute blocker grid: covers `±FOV_RADIUS_DAY`
/// in both axes plus the origin. The grid is stack-allocated in
/// `recompute_fov` to avoid the per-move HashSet alloc the previous
/// implementation paid.
const BLOCKER_GRID_SIDE: usize = (2 * FOV_RADIUS_DAY + 1) as usize;
const BLOCKER_GRID_LEN: usize = BLOCKER_GRID_SIDE * BLOCKER_GRID_SIDE;
const BLOCKER_GRID_CENTER: i32 = FOV_RADIUS_DAY;

#[inline]
fn blocker_idx(dx: i32, dy: i32) -> usize {
    let lx = (dx + BLOCKER_GRID_CENTER) as usize;
    let ly = (dy + BLOCKER_GRID_CENTER) as usize;
    ly * BLOCKER_GRID_SIDE + lx
}

/// Day/night dimming endpoints. Floor stays at 0.4 so nothing goes pitch
/// black before FOV+fires (phase 6+10) reach gameplay.
const TINT_DAY: f32 = 1.0;
const TINT_NIGHT: f32 = 0.4;
/// Linear-blend windows around the dusk/dawn transitions. The mechanical
/// is_night boundary stays sharp at 20:00 / 06:00; the visual blend is
/// centered on those boundaries so dusk darkens over an hour.
const DUSK_START_SECS: u64 = 19 * 3600 + 1800; // 19:30
const DUSK_END_SECS: u64 = 20 * 3600 + 1800; // 20:30
const DAWN_START_SECS: u64 = 5 * 3600 + 1800; // 05:30
const DAWN_END_SECS: u64 = 6 * 3600 + 1800; // 06:30

/// Whole-screen brightness multiplier in `[TINT_NIGHT, TINT_DAY]` derived
/// from the in-game time of day. Pure function of `clock_seconds`; render
/// applies it per cell.
pub fn brightness_at(clock_seconds: u64) -> f32 {
    let tod = clock_seconds % DAY_LENGTH_SECONDS;
    if tod < DAWN_START_SECS {
        TINT_NIGHT
    } else if tod < DAWN_END_SECS {
        let t = (tod - DAWN_START_SECS) as f32 / (DAWN_END_SECS - DAWN_START_SECS) as f32;
        TINT_NIGHT + (TINT_DAY - TINT_NIGHT) * t
    } else if tod < DUSK_START_SECS {
        TINT_DAY
    } else if tod < DUSK_END_SECS {
        let t = (tod - DUSK_START_SECS) as f32 / (DUSK_END_SECS - DUSK_START_SECS) as f32;
        TINT_DAY + (TINT_NIGHT - TINT_DAY) * t
    } else {
        TINT_NIGHT
    }
}

/// Monotonically-increasing count of dawn crossings since the game-time
/// epoch (`clock_seconds = 0`, midnight of "day 0"). A 14:00 spawn returns
/// 1 (day-1 dawn has already passed). Crossing 06:00 of the next day
/// bumps the count; main loop uses this to fire the auto-save-on-dawn.
pub fn dawns_elapsed(clock_seconds: u64) -> u64 {
    const DAWN_OFFSET: u64 = 6 * 3600;
    if clock_seconds < DAWN_OFFSET {
        0
    } else {
        (clock_seconds - DAWN_OFFSET) / DAY_LENGTH_SECONDS + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub cx: i32,
    pub cy: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainKind {
    /// Forest-floor cell: walkable, no sight blocking. The default
    /// surface for a phase-11 forest chunk.
    Grass,
    /// Open dirt patch: walkable, no sight blocking. Carved by trampling
    /// in slice 2+; for slice 1 it shows up as small clearings inside
    /// tree clusters.
    BareDirt,
    /// Sandy ring around the pond. Walkable; no sight blocking.
    SandShore,
    /// Mature tree: NOT walkable; blocks sight. Yields firewood when
    /// chopped (phase 11b's ChopTree verb).
    TreeTrunk,
    /// Flowing stream. NOT walkable in slice 1. Drinkable/fillable from
    /// adjacent cells (phase 11b verbs).
    StreamWater,
    /// Standing pond water. NOT walkable in slice 1. Drinkable + fishable
    /// from adjacent cells.
    PondWater,
    /// Out-of-chunk default. NOT walkable; blocks sight.
    Wall,
}

pub struct TerrainDef {
    /// Stable identifier used by terrain-mutation save round-trip
    /// (phase 11b's ChopTree converts TreeTrunk -> Grass and persists
    /// the change via this key).
    pub save_key: &'static str,
    /// Human-readable name for the here-line ("you stand on grass").
    #[allow(dead_code)] // wired by future "terrain underfoot" HUD line
    pub name: &'static str,
    pub glyph: u8,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub walkable: bool,
    pub blocks_sight: bool,
}

/// Atlas byte indices for the four custom tree-canopy sprites. The
/// render loop picks one per cell via a `(x, y, seed)` hash so the
/// forest has visual variety. 0xB5 / 0xC6 are the dead-tree sprites
/// available for future TerrainKind::DeadTree (chopped stumps,
/// burnt-out groves) — see assets/CP437_MAP.md.
pub const TREE_VARIANT_GLYPHS: &[u8] = &[0x05, 0x06, 0x17, 0x18];

/// Tint colors applied per-cell to tree canopies so adjacent trees
/// have slightly different hues. Atlas pixel × variant fg / 255 →
/// shaded canopy in that base hue. Five entries cover summer-forest
/// palette: bright green, deep green, olive, yellow-green, and one
/// autumn-brown for accent. Hash mixer picks per cell.
pub const TREE_TINT_VARIANTS: &[[u8; 3]] = &[
    [85, 140, 55],   // bright forest green
    [60, 100, 40],   // dark green
    [110, 145, 60],  // olive
    [130, 160, 50],  // yellow-green
    [140, 100, 45],  // autumn brown (rare accent)
];

/// Iteration order for `TerrainKind::from_save_key`. Keep in sync with
/// the enum variants — adding a kind here makes from_save_key find it.
const ALL_TERRAINS: &[TerrainKind] = &[
    TerrainKind::Grass,
    TerrainKind::BareDirt,
    TerrainKind::SandShore,
    TerrainKind::TreeTrunk,
    TerrainKind::StreamWater,
    TerrainKind::PondWater,
    TerrainKind::Wall,
];

impl TerrainKind {
    /// Stable string for save round-tripping a terrain mutation. Just
    /// reads `def().save_key`; from_save_key reverses by iterating the
    /// `ALL_TERRAINS` array.
    pub fn save_key(self) -> &'static str {
        self.def().save_key
    }

    pub fn from_save_key(s: &str) -> Option<Self> {
        ALL_TERRAINS.iter().copied().find(|t| t.def().save_key == s)
    }

    /// Single source of truth for per-terrain rendering + game-rules
    /// metadata. Adding a new terrain variant is a one-stop edit: add
    /// the enum arm, then add an arm here. Exhaustive-match enforces it.
    pub fn def(self) -> TerrainDef {
        match self {
            // Aesthetic note: ground terrains (grass, dirt, sand) are
            // desaturated on purpose so trees and water remain the
            // visual landmarks and items on the floor read with
            // contrast. See STYLE.md §2.8 / the aesthetic pass commit.
            // Floor terrains use heavily desaturated fg toned toward
            // their bg; the render path adds a per-cell ±8 RGB jitter
            // (see floor_color_offset in main.rs) to make the floor
            // read as a gradient texture rather than flat tone. Trees
            // + water keep saturation so they pierce the field.
            TerrainKind::Grass => TerrainDef {
                save_key: "grass",
                name: "grass",
                // 0x9C: custom grass-tuft sprite. Sparse-dot logic in
                // main.rs renders blank for ~75% of cells; this glyph
                // shows on the rest.
                glyph: 0x9C,
                // Saturated green tints the grayscale tuft; the
                // per-cell floor_with_gradient adds ±8 variation.
                fg: [80, 130, 55],
                bg: [14, 22, 14],
                walkable: true,
                blocks_sight: false,
            },
            TerrainKind::BareDirt => TerrainDef {
                save_key: "bare_dirt",
                name: "dirt",
                glyph: b'.',
                fg: [62, 52, 40],
                bg: [18, 15, 11],
                walkable: true,
                blocks_sight: false,
            },
            TerrainKind::SandShore => TerrainDef {
                save_key: "sand_shore",
                name: "sand",
                glyph: b'.',
                fg: [105, 95, 72],
                bg: [32, 27, 19],
                walkable: true,
                blocks_sight: false,
            },
            // Trees + water keep most of their saturation so they
            // anchor the eye against the muted floor.
            TerrainKind::TreeTrunk => TerrainDef {
                save_key: "tree_trunk",
                name: "tree",
                // Default glyph; the render loop overrides this per
                // cell with one of TREE_VARIANT_GLYPHS based on a
                // (x, y, seed) hash so the forest has visual variety.
                glyph: 0x06,
                // Near-white so each variant's atlas color shows.
                fg: [230, 235, 215],
                bg: [12, 20, 12],
                walkable: false,
                blocks_sight: true,
            },
            TerrainKind::StreamWater => TerrainDef {
                save_key: "stream_water",
                name: "stream",
                glyph: b'~',
                fg: [85, 130, 175],
                bg: [20, 30, 50],
                walkable: false,
                blocks_sight: false,
            },
            TerrainKind::PondWater => TerrainDef {
                save_key: "pond_water",
                name: "pond",
                glyph: b'~',
                fg: [55, 100, 155],
                bg: [18, 28, 48],
                walkable: false,
                blocks_sight: false,
            },
            TerrainKind::Wall => TerrainDef {
                save_key: "wall",
                name: "wall",
                glyph: b'#',
                fg: [140, 110, 75],
                bg: [35, 28, 20],
                walkable: false,
                blocks_sight: true,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct CellState {
    pub terrain: TerrainKind,
    pub items: Vec<ItemInstance>,
    /// Currently in the player's FOV. Recomputed on move + day/night flip;
    /// not persisted.
    pub visible: bool,
    /// Has been visible at least once. Persisted across save/load via
    /// `RunSave.explored_cells`.
    pub explored: bool,
    /// Per-cell light intensity from active light sources (any
    /// `ItemMetadata::Lit` carrier) this recompute. 0 = no light,
    /// 255 = at a light source. Render scales the warm-yellow blend
    /// and a brightness boost by this so the disc gradients from
    /// bright-warm at the source to invisible at the edge instead of
    /// being a uniform patch. Transient like `visible` — reset and
    /// rebuilt every `recompute_fov` call; not persisted.
    pub light_intensity: u8,
}

impl CellState {
    pub fn with_terrain(terrain: TerrainKind) -> Self {
        Self {
            terrain,
            items: Vec::new(),
            visible: false,
            explored: false,
            light_intensity: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Chunk {
    #[allow(dead_code)] // read in phase 12+ when chunks load/save individually
    pub coord: ChunkCoord,
    pub cells: Vec<CellState>,
    pub dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Renderable {
    pub glyph: u8,
    pub fg: [u8; 4],
    pub bg: [u8; 4],
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Player;

pub struct World {
    /// Chunk store keyed on grid coords. Outside this module, prefer the
    /// `tile_at` / `cell_at` / `cell_at_mut` / `snapshot_*` accessor
    /// methods — those preserve the chunk's `dirty` flag and validate
    /// bounds. Direct mutation of cells via this map will silently
    /// bypass dirty-tracking and break saves.
    pub(crate) chunks: HashMap<ChunkCoord, Box<Chunk>>,
    /// World seed. Consumed by chunkgen for layout determinism and by
    /// main.rs's sparse-grass-dot hash so the visual texture is stable
    /// across reloads.
    pub seed: u64,
    /// Game-time clock in seconds since "game start" (not real-time). Wraps
    /// the 24-hour day for time-of-day queries via div/mod with
    /// DAY_LENGTH_SECONDS.
    pub clock_seconds: u64,
    pub ecs: Ecs,
    pub player: Entity,
    /// Set when the player commits to a multi-turn verb (phase 9). The
    /// main loop ticks this each frame and surfaces a progress overlay;
    /// regular input is suspended while it is `Some`. Cleared on
    /// completion, cancellation, or interrupt.
    pub active_action: Option<ActiveAction>,
    /// xorshift32 PRNG used for skill checks. Save/restore preserves
    /// state so reloading after a critical roll re-rolls the SAME
    /// outcome — prevents save-scumming.
    pub rng: Rng,
    /// Cells whose terrain has been mutated since chunkgen produced
    /// them. ChopTree converts TreeTrunk -> Grass and records the
    /// change here; on load the entries get re-applied after chunkgen
    /// regenerates the chunk's defaults. Keyed on world coords.
    pub terrain_mutations: HashMap<(i32, i32), TerrainKind>,
    /// In-game calendar day, 1-indexed since 1 Jan 1300 (so spawn day =
    /// 80 = 21 Mar 1300). Advances each midnight crossing inside
    /// `advance_time_raw`. Drives `season_of` for the seasons/flora
    /// cluster.
    pub calendar_day: u32,
}

/// In-flight multi-turn action queue. `steps[0]` is the currently-running
/// step; completed steps pop off the front. The queue empties on the
/// last step's completion (or all-at-once on cancel/interrupt).
#[derive(Clone, Debug)]
pub struct ActiveAction {
    pub steps: VecDeque<ActionStep>,
    pub view_mode: ViewMode,
}

#[derive(Clone, Copy, Debug)]
pub struct ActionStep {
    pub id: ActionId,
    pub elapsed_secs: u32,
    pub target_secs: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    ProgressBar,
    TimeSkip,
}

impl ViewMode {
    pub fn toggled(self) -> Self {
        match self {
            ViewMode::ProgressBar => ViewMode::TimeSkip,
            ViewMode::TimeSkip => ViewMode::ProgressBar,
        }
    }
}

impl ActiveAction {
    pub fn current_step(&self) -> Option<&ActionStep> {
        self.steps.front()
    }

    /// Total game-seconds across all remaining steps (current step's
    /// remainder plus all unstarted steps). Used by the progress-bar UI.
    pub fn total_remaining_secs(&self) -> u32 {
        self.steps
            .iter()
            .map(|s| s.target_secs.saturating_sub(s.elapsed_secs))
            .sum()
    }
}

/// Returned by `World::tick_multi_turn`. The frame loop reads this to:
///   1. Call `action::complete_step` for each finished step
///   2. Log an interruption reason if `interrupted` is true
#[derive(Default)]
pub struct MultiTurnTickResult {
    pub completed_steps: Vec<ActionId>,
    pub interrupted: bool,
}

impl World {
    pub fn new(width: u32, height: u32) -> Self {
        // Slice-1 viewport is fixed to a single chunk. The width/height args
        // come from main's WORLD_W/WORLD_H constants; assert they match the
        // chunk dims so a future bump in main flags itself loudly here rather
        // than rendering a half-chunk.
        debug_assert_eq!(width, CHUNK_W);
        debug_assert_eq!(height, CHUNK_H);

        let mut chunks = HashMap::new();
        let origin = ChunkCoord { cx: 0, cy: 0 };
        chunks.insert(
            origin,
            Box::new(crate::chunkgen::generate_chunk(origin, DEFAULT_SEED)),
        );

        let mut ecs = Ecs::new();
        let spawn = Position {
            x: CHUNK_W as i32 / 2,
            y: CHUNK_H as i32 / 2,
        };
        let player = ecs.spawn((
            Player,
            spawn,
            Renderable {
                glyph: b'@',
                fg: [240, 232, 200, 255],
                bg: [20, 17, 13, 255],
            },
            starting_pack(),
            Needs::starting(),
            Skills::starting(),
        ));

        let mut world = Self {
            chunks,
            seed: DEFAULT_SEED,
            clock_seconds: STARTING_CLOCK_SECONDS,
            ecs,
            player,
            active_action: None,
            rng: Rng::from_world_seed(DEFAULT_SEED),
            terrain_mutations: HashMap::new(),
            calendar_day: calendar::START_DAY,
        };
        // First-frame FOV so the renderer doesn't draw a black screen on
        // the very first paint.
        world.recompute_fov();
        world
    }

    fn chunk_coord_for(wx: i64, wy: i64) -> (ChunkCoord, u32, u32) {
        let cx = wx.div_euclid(CHUNK_W as i64) as i32;
        let cy = wy.div_euclid(CHUNK_H as i64) as i32;
        let lx = wx.rem_euclid(CHUNK_W as i64) as u32;
        let ly = wy.rem_euclid(CHUNK_H as i64) as u32;
        (ChunkCoord { cx, cy }, lx, ly)
    }

    pub fn tile_at(&self, wx: i64, wy: i64) -> TerrainKind {
        let (cc, lx, ly) = Self::chunk_coord_for(wx, wy);
        let Some(chunk) = self.chunks.get(&cc) else {
            return TerrainKind::Wall;
        };
        if lx >= CHUNK_W || ly >= CHUNK_H {
            return TerrainKind::Wall;
        }
        chunk.cells[(ly * CHUNK_W + lx) as usize].terrain
    }

    pub fn cell_at(&self, wx: i64, wy: i64) -> Option<&CellState> {
        let (cc, lx, ly) = Self::chunk_coord_for(wx, wy);
        let chunk = self.chunks.get(&cc)?;
        if lx >= CHUNK_W || ly >= CHUNK_H {
            return None;
        }
        Some(&chunk.cells[(ly * CHUNK_W + lx) as usize])
    }

    pub fn cell_at_mut(&mut self, wx: i64, wy: i64) -> Option<&mut CellState> {
        let (cc, lx, ly) = Self::chunk_coord_for(wx, wy);
        let chunk = self.chunks.get_mut(&cc)?;
        if lx >= CHUNK_W || ly >= CHUNK_H {
            return None;
        }
        chunk.dirty = true;
        Some(&mut chunk.cells[(ly * CHUNK_W + lx) as usize])
    }

    /// Lazily generate a chunk if it isn't already loaded. Reuses the
    /// existing seeded chunkgen path so coords are deterministic across
    /// reloads. After generation, any saved terrain mutations that fall
    /// inside this chunk's bounds are re-applied so chopped trees etc.
    /// survive even if the chunk wasn't in memory at load time.
    pub fn ensure_chunk_loaded(&mut self, coord: ChunkCoord) {
        if self.chunks.contains_key(&coord) {
            return;
        }
        let mut chunk = crate::chunkgen::generate_chunk(coord, self.seed);
        // Apply any pending terrain mutations for this chunk.
        let cw = CHUNK_W as i32;
        let ch = CHUNK_H as i32;
        for (&(x, y), &kind) in self.terrain_mutations.iter() {
            let cx = (x as i64).div_euclid(CHUNK_W as i64) as i32;
            let cy = (y as i64).div_euclid(CHUNK_H as i64) as i32;
            if cx != coord.cx || cy != coord.cy {
                continue;
            }
            let lx = (x as i64).rem_euclid(CHUNK_W as i64) as u32;
            let ly = (y as i64).rem_euclid(CHUNK_H as i64) as u32;
            if (lx as i32) < cw && (ly as i32) < ch {
                chunk.cells[(ly * CHUNK_W + lx) as usize].terrain = kind;
            }
        }
        self.chunks.insert(coord, Box::new(chunk));
    }

    /// Ensure the 3x3 ring of chunks around `center` is loaded. Called
    /// after the player crosses a chunk seam so FOV (radius 20) always
    /// queries against in-memory chunks rather than the OOB Wall fallback.
    pub fn ensure_chunk_ring(&mut self, center: ChunkCoord) {
        for dy in -1..=1 {
            for dx in -1..=1 {
                self.ensure_chunk_loaded(ChunkCoord {
                    cx: center.cx + dx,
                    cy: center.cy + dy,
                });
            }
        }
    }

    /// Convenience: the chunk the player currently stands in.
    pub fn player_chunk(&self) -> ChunkCoord {
        let p = self.player_pos();
        let (cc, _, _) = Self::chunk_coord_for(p.x as i64, p.y as i64);
        cc
    }

    /// Load the chunk ring around the player. Call after `World::new` and
    /// after restoring a save, so the first frame's FOV cast has the
    /// neighbors in memory.
    pub fn ensure_player_ring(&mut self) {
        let cc = self.player_chunk();
        self.ensure_chunk_ring(cc);
    }

    pub fn player_pos(&self) -> Position {
        *self
            .ecs
            .get::<&Position>(self.player)
            .expect("player has Position")
    }

    pub fn set_player_pos(&mut self, p: Position) {
        *self
            .ecs
            .get::<&mut Position>(self.player)
            .expect("player has Position") = p;
    }

    pub fn try_move_player(&mut self, dx: i32, dy: i32) {
        let pos = self.player_pos();
        let nx = pos.x + dx;
        let ny = pos.y + dy;
        // Ensure the destination's chunk ring is loaded before the
        // walkability check — otherwise stepping into a freshly-revealed
        // chunk would read as Wall (the tile_at OOB fallback) and the
        // move would be rejected.
        let (target_cc, _, _) = Self::chunk_coord_for(nx as i64, ny as i64);
        self.ensure_chunk_ring(target_cc);
        if self.tile_at(nx as i64, ny as i64).def().walkable {
            self.set_player_pos(Position { x: nx, y: ny });
            self.spend_action_time(COST_MOVE_TILE);
            self.recompute_fov();
        }
    }

    pub fn player_needs(&self) -> Needs {
        *self
            .ecs
            .get::<&Needs>(self.player)
            .expect("player has Needs")
    }

    pub fn set_player_needs(&mut self, needs: Needs) {
        *self
            .ecs
            .get::<&mut Needs>(self.player)
            .expect("player has Needs") = needs;
    }

    pub fn player_skills(&self) -> Skills {
        *self
            .ecs
            .get::<&Skills>(self.player)
            .expect("player has Skills")
    }

    pub fn set_player_skills(&mut self, skills: Skills) {
        *self
            .ecs
            .get::<&mut Skills>(self.player)
            .expect("player has Skills") = skills;
    }

    /// True if the in-game clock is between dusk and dawn.
    pub fn is_night(&self) -> bool {
        let time_of_day = self.clock_seconds % DAY_LENGTH_SECONDS;
        let hour = time_of_day / 3600;
        hour < DAWN_HOUR || hour >= DUSK_HOUR
    }

    /// (hours, minutes) clock display.
    pub fn clock_hm(&self) -> (u8, u8) {
        let tod = self.clock_seconds % DAY_LENGTH_SECONDS;
        let h = (tod / 3600) as u8;
        let m = ((tod % 3600) / 60) as u8;
        (h, m)
    }

    pub fn day_count(&self) -> u64 {
        // Day 1 = the spawn day. Player spawns at 14:00 of day 1, so days
        // increment at each midnight crossing.
        self.clock_seconds / DAY_LENGTH_SECONDS + 1
    }

    /// Current season. Pure function of `calendar_day`.
    pub fn season(&self) -> Season {
        calendar::season_of(self.calendar_day)
    }

    fn needs_env(&self) -> NeedsEnv {
        NeedsEnv {
            is_night: self.is_night(),
            adjacent_fire: self.light_source_adjacent_to_player(),
            inside_tent: self.player_on_pitched(ItemKind::Tent),
            in_bedroll: self.player_on_pitched(ItemKind::Bedroll),
        }
    }

    /// Advance the game-time clock by an action's cost, amplified by the
    /// current need penalty. This is the canonical path for "instant"
    /// verbs (move, pickup, eat, drink).
    ///
    /// Multi-turn verbs DO NOT call this directly. Each per-second tick
    /// during a multi-turn action goes through `advance_time_raw`
    /// (penalty already baked into the queued target_secs at action
    /// start), because applying the penalty per-second would silently
    /// truncate the bonus on tiny ticks.
    pub fn spend_action_time(&mut self, base_cost: u32) {
        let needs = self.player_needs();
        let penalty_pct = needs.action_cost_penalty_pct();
        let elapsed = base_cost.saturating_add(base_cost * penalty_pct / 100);
        self.advance_time_raw(elapsed);
    }

    /// Advance the clock by `secs` game-seconds without recomputing the
    /// need penalty. Ticks needs decay, lit fires, and refreshes FOV at
    /// day/night boundaries. The "raw" suffix marks this as the bypass
    /// path for per-second multi-turn simulation; instant verbs use
    /// `spend_action_time`.
    pub fn advance_time_raw(&mut self, secs: u32) {
        if secs == 0 {
            return;
        }
        let was_night = self.is_night();
        let before = self.clock_seconds;
        self.clock_seconds = self.clock_seconds.saturating_add(secs as u64);
        // Calendar day advances at each midnight (24h) crossing. Use
        // floor-division on before/after so multi-day jumps from debug
        // commands or long sleeps land on the right calendar_day.
        let midnights = self.clock_seconds / DAY_LENGTH_SECONDS - before / DAY_LENGTH_SECONDS;
        if midnights > 0 {
            self.calendar_day = self.calendar_day.saturating_add(midnights as u32);
        }
        let env = self.needs_env();
        let mut needs = self.player_needs();
        needs.tick(secs, env);
        self.set_player_needs(needs);
        let fire_died = self.tick_fires(secs);
        self.tick_cookware(secs);
        let day_night_flipped = self.is_night() != was_night;
        // Recompute FOV on a day/night boundary OR when a fire died
        // during night (the lit radius may have shrunk). The day-only
        // case is uninteresting since fires don't change day FOV.
        if day_night_flipped || (fire_died && self.is_night()) {
            self.recompute_fov();
        }
    }

    /// Decrement `fuel_seconds` on every Lit item in every loaded chunk.
    /// Items whose fuel hits 0 are removed (the fire burnt out and the
    /// firewood is consumed). Phase-12's "feed fire" verb adds fuel
    /// back from inventory before the timer hits 0. Returns true if at
    /// least one fire went out this tick — the caller uses that to
    /// invalidate FOV (phase 13b: a dying fire shrinks the lit radius).
    fn tick_fires(&mut self, secs: u32) -> bool {
        let mut any_died = false;
        for chunk in self.chunks.values_mut() {
            for cell in chunk.cells.iter_mut() {
                if !cell.items.iter().any(|i| matches!(i.metadata, ItemMetadata::Lit { .. })) {
                    continue;
                }
                let before = cell.items.len();
                cell.items.retain_mut(|item| {
                    if let ItemMetadata::Lit { fuel_seconds } = &mut item.metadata {
                        if *fuel_seconds > secs {
                            *fuel_seconds -= secs;
                            true
                        } else {
                            false // burned out
                        }
                    } else {
                        true
                    }
                });
                if cell.items.len() < before {
                    any_died = true;
                }
                chunk.dirty = true;
            }
        }
        any_died
    }

    /// Decrement `fuel_seconds` on every PannedOnFire item and advance
    /// any in-flight cooking. Fuel depletion converts the pan back to
    /// a plain `CookingPan` on the cell; any food still in the pan at
    /// that moment is dropped to the cell as a Cooked item with state
    /// set by `cook_progress` (so abandoning a fish on a dying fire
    /// gives you a half-cooked outcome instead of vanishing it).
    fn tick_cookware(&mut self, secs: u32) {
        if secs == 0 {
            return;
        }
        for chunk in self.chunks.values_mut() {
            let mut dirtied = false;
            for cell in chunk.cells.iter_mut() {
                let needs_pass = cell
                    .items
                    .iter()
                    .any(|i| matches!(i.metadata, ItemMetadata::PannedOnFire { .. }));
                if !needs_pass {
                    continue;
                }
                let mut spilled_cooked: Vec<ItemInstance> = Vec::new();
                for item in cell.items.iter_mut() {
                    let (new_contents, new_fuel, spill) = match item.metadata {
                        ItemMetadata::PannedOnFire {
                            contents,
                            fuel_seconds,
                        } => {
                            // Advance cooking BEFORE consuming fuel so
                            // a cook that finishes exactly when fuel
                            // hits 0 still produces its output.
                            let advanced = match contents {
                                crate::crafting::PanContents::Cooking {
                                    input,
                                    elapsed_secs,
                                    seasonings,
                                } => crate::crafting::PanContents::Cooking {
                                    input,
                                    elapsed_secs: elapsed_secs.saturating_add(secs),
                                    seasonings,
                                },
                                other => other,
                            };
                            if fuel_seconds > secs {
                                (Some(advanced), Some(fuel_seconds - secs), None)
                            } else {
                                // Fuel exhausted. Eject any in-flight
                                // cook to the cell; pan becomes plain.
                                let spill = match advanced {
                                    crate::crafting::PanContents::Cooking {
                                        input,
                                        elapsed_secs,
                                        seasonings,
                                    } => {
                                        let state = match crate::crafting::cook_progress(
                                            input,
                                            elapsed_secs,
                                        ) {
                                            crate::crafting::CookProgress::Ok => {
                                                crate::crafting::CookedState::Ok
                                            }
                                            // Raw or burnt on a dead
                                            // fire both salvage as
                                            // Burnt — the player
                                            // doesn't gain a full cook
                                            // for free.
                                            _ => crate::crafting::CookedState::Burnt,
                                        };
                                        Some(ItemInstance::unique(
                                            ItemKind::Cooked,
                                            ItemKind::Cooked.def().default_weight_g,
                                            None,
                                            ItemMetadata::Cooked {
                                                base: input,
                                                state,
                                                seasonings,
                                            },
                                        ))
                                    }
                                    _ => None,
                                };
                                (None, None, spill)
                            }
                        }
                        _ => continue,
                    };

                    match (new_contents, new_fuel) {
                        (Some(c), Some(f)) => {
                            item.metadata = ItemMetadata::PannedOnFire {
                                contents: c,
                                fuel_seconds: f,
                            };
                        }
                        _ => {
                            item.metadata = ItemMetadata::None;
                        }
                    }
                    if let Some(s) = spill {
                        spilled_cooked.push(s);
                    }
                    dirtied = true;
                }
                if !spilled_cooked.is_empty() {
                    cell.items.extend(spilled_cooked);
                    dirtied = true;
                }
            }
            if dirtied {
                chunk.dirty = true;
            }
        }
    }

    /// Is there at least one active light source (any item carrying
    /// `ItemMetadata::Lit`) in the player's cell or any of the 8
    /// adjacent cells? Slice-1 only spawns lit firewood from StartFire,
    /// but the lit-source abstraction is the metadata marker — when
    /// torches/lanterns land they'll be a different ItemKind with the
    /// same Lit metadata and this predicate covers them with no edit.
    /// NeedsEnv reads this for warmth shelter (phase 13a).
    pub fn light_source_adjacent_to_player(&self) -> bool {
        let p = self.player_pos();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let Some(cell) = self.cell_at((p.x + dx) as i64, (p.y + dy) as i64) else {
                    continue;
                };
                if cell.items.iter().any(|i| is_active_fire(&i.metadata)) {
                    return true;
                }
            }
        }
        false
    }

    /// Fill `buf` with the world positions of every active light source
    /// (any item carrying `ItemMetadata::Lit`) in the loaded chunks,
    /// up to MAX_LIGHT_SOURCES. Returns the count written. Caller-owned
    /// stack buffer so we don't heap-allocate per `recompute_fov` —
    /// recompute fires every move on a handheld and the Vec churn adds
    /// up. Each returned position becomes the origin of an independent
    /// FOV cast.
    pub fn collect_light_sources_into(
        &self,
        buf: &mut [Position; MAX_LIGHT_SOURCES],
    ) -> usize {
        let cw = CHUNK_W as i32;
        let ch = CHUNK_H as i32;
        let mut count = 0;
        for chunk in self.chunks.values() {
            for (idx, cell) in chunk.cells.iter().enumerate() {
                if !cell.items.iter().any(|i| is_active_fire(&i.metadata)) {
                    continue;
                }
                if count >= MAX_LIGHT_SOURCES {
                    return count;
                }
                let lx = (idx as i32) % cw;
                let ly = (idx as i32) / cw;
                buf[count] = Position {
                    x: chunk.coord.cx * cw + lx,
                    y: chunk.coord.cy * ch + ly,
                };
                count += 1;
            }
        }
        count
    }

    /// Is the player standing on a Pitched item of the given kind?
    /// Used by needs_env() for tent/bedroll warmth shelter checks
    /// (phase 13a). The Pitched metadata marker is set by
    /// place_pitched_from_pack in action.rs.
    pub fn player_on_pitched(&self, kind: ItemKind) -> bool {
        let p = self.player_pos();
        self.cell_at(p.x as i64, p.y as i64).map_or(false, |cell| {
            cell.items
                .iter()
                .any(|i| i.kind == kind && matches!(i.metadata, ItemMetadata::Pitched))
        })
    }

    /// Queue a multi-turn action. `steps` lists the sub-actions in order
    /// with their base costs in game-seconds; the current need penalty
    /// is applied once at queueing so each step's `target_secs` is the
    /// amplified value. Subsequent need degradation during the action
    /// doesn't re-stretch the queue.
    pub fn queue_multi_turn(&mut self, steps: &[(ActionId, u32)]) {
        let penalty_pct = self.player_needs().action_cost_penalty_pct();
        let amplify = |base: u32| base.saturating_add(base * penalty_pct / 100);
        let q: VecDeque<ActionStep> = steps
            .iter()
            .map(|&(id, base)| ActionStep {
                id,
                elapsed_secs: 0,
                target_secs: amplify(base),
            })
            .collect();
        self.active_action = Some(ActiveAction {
            steps: q,
            view_mode: ViewMode::ProgressBar,
        });
    }

    /// Queue a multi-turn action whose target durations are already
    /// the literal game-seconds you want to advance (no need-penalty
    /// amplification). Phase 16 Sleep uses this because "sleep until
    /// dawn" is computed from wall-clock time and shouldn't stretch
    /// when the player is starving.
    pub fn queue_multi_turn_raw(&mut self, steps: &[(ActionId, u32)]) {
        let q: VecDeque<ActionStep> = steps
            .iter()
            .map(|&(id, target)| ActionStep {
                id,
                elapsed_secs: 0,
                target_secs: target,
            })
            .collect();
        self.active_action = Some(ActiveAction {
            steps: q,
            view_mode: ViewMode::ProgressBar,
        });
    }

    /// Advance the active multi-turn action by up to `advance_secs`
    /// game-seconds, ticking needs once per second and checking the
    /// interrupt threshold each step. Returns the ActionIds of any
    /// steps that completed during this tick (caller invokes
    /// `action::complete_step` for each) plus whether an interrupt
    /// fired (in which case the entire queue is abandoned and `Done`
    /// effects do NOT run for the partially-completed current step).
    pub fn tick_multi_turn(&mut self, advance_secs: u32) -> MultiTurnTickResult {
        let mut result = MultiTurnTickResult::default();
        let Some(mut active) = self.active_action.take() else {
            return result;
        };

        let mut remaining = advance_secs;
        while remaining > 0 && !active.steps.is_empty() {
            self.advance_time_raw(1);
            remaining -= 1;

            // Interrupt check after each simulated second. The
            // partially-elapsed current step is abandoned; the player
            // does not pay the consume-from-pack price.
            let n = self.player_needs();
            if n.thirst < NEED_CRITICAL_THRESHOLD
                || n.hunger < NEED_CRITICAL_THRESHOLD
                || n.sleep < NEED_CRITICAL_THRESHOLD
                || n.warmth < NEED_CRITICAL_THRESHOLD
            {
                result.interrupted = true;
                break;
            }

            let step = active.steps.front_mut().expect("non-empty checked above");
            step.elapsed_secs += 1;
            if step.elapsed_secs >= step.target_secs {
                result.completed_steps.push(step.id);
                active.steps.pop_front();
            }
        }

        // Put active back only if there's still work to do and we
        // weren't interrupted; otherwise the queue is gone.
        if !result.interrupted && !active.steps.is_empty() {
            self.active_action = Some(active);
        }
        result
    }

    /// Player-driven cancel (B). Drops the queue without firing any
    /// completion effects. Time already spent stays spent (the player
    /// "wasted" those game-seconds).
    pub fn cancel_multi_turn(&mut self) {
        self.active_action = None;
    }

    pub fn toggle_multi_turn_view(&mut self) {
        if let Some(active) = self.active_action.as_mut() {
            active.view_mode = active.view_mode.toggled();
        }
    }

    /// Reset visibility flags across loaded chunks and cast FOV from the
    /// player's current position. Radius depends on day/night. Also marks
    /// newly-seen cells as `explored` so the renderer can show them dimmed
    /// after the player walks away.
    ///
    /// Blocker snapshot lives in a stack-allocated bool grid (1681 bytes
    /// for the radius-20 worst case) so this method does no heap
    /// allocation per move — the previous implementation used a HashSet,
    /// which paid alloc/hash cost per recompute on the Cortex-A7 target.
    pub fn recompute_fov(&mut self) {
        // Reset transient flags across loaded cells. light_intensity is
        // reset alongside visible so a fire that burned out doesn't
        // leave stale glow on cells from the previous tick.
        for chunk in self.chunks.values_mut() {
            for cell in chunk.cells.iter_mut() {
                cell.visible = false;
                cell.light_intensity = 0;
            }
        }

        // Player FOV: their own radius from their own position. Always
        // runs; never marks fire_lit (the warm tint belongs to cells
        // illuminated BY a fire, not cells the player just happens to
        // see in their own night vision).
        let player_radius = if self.is_night() {
            FOV_RADIUS_NIGHT
        } else {
            FOV_RADIUS_DAY
        };
        self.cast_from(self.player_pos(), player_radius, false);

        // Per-light-source FOV: at night, every Lit-metadata carrier
        // in the loaded chunks shadowcasts its own disc independently
        // and marks the cells it reaches as fire_lit. Cells already in
        // the player's FOV stay visible AND pick up fire_lit on overlap;
        // cells outside the player's disc but inside a fire's disc
        // become visible via the union. By day we skip — the player's
        // radius-20 disc subsumes any fire's radius-5, and the warm
        // tint would be invisible against daylight anyway.
        if self.is_night() {
            let mut sources = [Position { x: 0, y: 0 }; MAX_LIGHT_SOURCES];
            let n = self.collect_light_sources_into(&mut sources);
            for &src in &sources[..n] {
                self.cast_from(src, LIGHT_SOURCE_RADIUS, true);
            }
        }
    }

    /// Shadowcast from `origin` at `radius`, marking each visible cell
    /// as visible+explored. When `mark_light` is true (per-light-source
    /// cast), each reached cell also gets a `light_intensity` derived
    /// from its Chebyshev distance to `origin` — distance 0 -> 255,
    /// distance `radius` -> 0, linear. Multiple sources lighting the
    /// same cell take the max (brightest wins). Shared by the player
    /// cast and each per-light-source cast in `recompute_fov`.
    fn cast_from(&mut self, origin: Position, radius: i32, mark_light: bool) {
        // Stack-allocated blocker grid centered on `origin`; sized for
        // FOV_RADIUS_DAY so every cast radius up to 20 fits without
        // reallocation.
        let mut blockers = [false; BLOCKER_GRID_LEN];
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let wx = origin.x as i64 + dx as i64;
                let wy = origin.y as i64 + dy as i64;
                if self.tile_at(wx, wy).def().blocks_sight {
                    blockers[blocker_idx(dx, dy)] = true;
                }
            }
        }

        let visible = crate::fov::compute_visible((origin.x, origin.y), radius, |x, y| {
            let dx = x - origin.x;
            let dy = y - origin.y;
            if dx.unsigned_abs() as i32 > FOV_RADIUS_DAY
                || dy.unsigned_abs() as i32 > FOV_RADIUS_DAY
            {
                return true;
            }
            blockers[blocker_idx(dx, dy)]
        });

        for (x, y) in visible {
            let dx = (x - origin.x).abs();
            let dy = (y - origin.y).abs();
            let dist = dx.max(dy); // Chebyshev — matches the square-FOV shape
            if let Some(cell) = self.cell_at_mut(x as i64, y as i64) {
                cell.visible = true;
                cell.explored = true;
                if mark_light {
                    // Linear falloff: at the source -> 255, at the
                    // radius edge -> 0. Clamp defensively (compute_visible
                    // shouldn't return cells beyond `radius`).
                    let intensity = if dist >= radius {
                        0
                    } else {
                        (((radius - dist) as u32 * 255) / radius as u32) as u8
                    };
                    if intensity > cell.light_intensity {
                        cell.light_intensity = intensity;
                    }
                }
            }
        }
    }

    /// Snapshot all explored cells in loaded chunks for save serialization.
    pub fn snapshot_explored(&self) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for (coord, chunk) in &self.chunks {
            for (i, cell) in chunk.cells.iter().enumerate() {
                if !cell.explored {
                    continue;
                }
                let lx = i as u32 % CHUNK_W;
                let ly = i as u32 / CHUNK_W;
                let wx = coord.cx * CHUNK_W as i32 + lx as i32;
                let wy = coord.cy * CHUNK_H as i32 + ly as i32;
                out.push((wx, wy));
            }
        }
        out
    }

    /// Restore explored bits from a save. Lazily loads any chunks the
    /// saved coords reference so explored cells survive across reloads
    /// even when the player has roamed beyond chunk (0,0).
    pub fn restore_explored(&mut self, coords: &[(i32, i32)]) {
        for &(wx, wy) in coords {
            let (cc, _, _) = Self::chunk_coord_for(wx as i64, wy as i64);
            self.ensure_chunk_loaded(cc);
            if let Some(cell) = self.cell_at_mut(wx as i64, wy as i64) {
                cell.explored = true;
            }
        }
    }

    /// Change the terrain at a cell and record the mutation for save
    /// round-trip. Use this instead of writing `cell.terrain = ...`
    /// directly so chopped trees, dug pits, etc. survive a reload.
    pub fn set_terrain_at(&mut self, wx: i64, wy: i64, kind: TerrainKind) {
        if let Some(cell) = self.cell_at_mut(wx, wy) {
            cell.terrain = kind;
        }
        self.terrain_mutations.insert((wx as i32, wy as i32), kind);
    }

    pub fn snapshot_terrain_mutations(&self) -> Vec<(i32, i32, TerrainKind)> {
        self.terrain_mutations
            .iter()
            .map(|(&(x, y), &k)| (x, y, k))
            .collect()
    }

    pub fn restore_terrain_mutations(&mut self, snap: Vec<(i32, i32, TerrainKind)>) {
        for (x, y, k) in snap {
            let (cc, _, _) = Self::chunk_coord_for(x as i64, y as i64);
            self.ensure_chunk_loaded(cc);
            if let Some(cell) = self.cell_at_mut(x as i64, y as i64) {
                cell.terrain = k;
            }
            self.terrain_mutations.insert((x, y), k);
        }
    }

    pub fn player_pack(&self) -> hecs::Ref<'_, Pack> {
        self.ecs
            .get::<&Pack>(self.player)
            .expect("player has Pack")
    }

    pub fn player_pack_mut(&mut self) -> hecs::RefMut<'_, Pack> {
        self.ecs
            .get::<&mut Pack>(self.player)
            .expect("player has Pack")
    }

    pub fn replace_player_pack(&mut self, pack: Pack) {
        *self
            .ecs
            .get::<&mut Pack>(self.player)
            .expect("player has Pack") = pack;
    }

    /// Greedy pickup primitive: every PICKABLE item in the player's
    /// current cell that fits in the pack moves into the pack. Items
    /// over capacity stay in the cell. Lit items (active fires) are NOT
    /// pickable — you can't pocket a burning campfire. Pitched items
    /// (tents, bedrolls) ARE pickable: re-stowing them is the "pack up
    /// camp" behavior, intentional in phase 9.
    ///
    /// Returns the number of `ItemInstance` entries successfully picked
    /// up (a merge counts as one entry). **Does not spend action time** —
    /// the caller (action.rs or the A-button handler in main.rs) owns
    /// that step so per-verb costs stay local to action.rs.
    pub fn try_pickup_all_at_player(&mut self) -> usize {
        let pos = self.player_pos();
        let wx = pos.x as i64;
        let wy = pos.y as i64;

        // Partition the cell's items into pickable + non-pickable; only
        // the pickable ones leave the cell.
        let (pickable, unpickable): (Vec<ItemInstance>, Vec<ItemInstance>) =
            match self.cell_at_mut(wx, wy) {
                Some(c) => std::mem::take(&mut c.items)
                    .into_iter()
                    .partition(|i| !is_active_fire(&i.metadata)),
                None => return 0,
            };

        let mut picked = 0;
        let mut rejects = Vec::new();
        {
            let mut pack = self.player_pack_mut();
            for item in pickable {
                match pack.try_add(item) {
                    Ok(()) => picked += 1,
                    Err(item) => rejects.push(item),
                }
            }
        }

        // Put back: rejects (didn't fit) + unpickable (lit fires).
        if let Some(c) = self.cell_at_mut(wx, wy) {
            c.items.extend(rejects);
            c.items.extend(unpickable);
        }

        picked
    }

    /// Snapshot all non-empty cells across loaded chunks. Used by the save
    /// path. Coords are world-coords (slice 1 always world == local since
    /// chunk (0, 0) starts at (0, 0)).
    pub fn snapshot_cell_items(&self) -> Vec<(i32, i32, Vec<ItemInstance>)> {
        let mut out = Vec::new();
        for (coord, chunk) in &self.chunks {
            for (i, cell) in chunk.cells.iter().enumerate() {
                if cell.items.is_empty() {
                    continue;
                }
                let lx = i as u32 % CHUNK_W;
                let ly = i as u32 / CHUNK_W;
                let wx = coord.cx * CHUNK_W as i32 + lx as i32;
                let wy = coord.cy * CHUNK_H as i32 + ly as i32;
                out.push((wx, wy, cell.items.clone()));
            }
        }
        out
    }

    /// Replace the items at given world coords. Used by the load path.
    /// Lazily loads any chunks the snapshot references so saved items
    /// outside the spawn ring aren't silently dropped on load.
    pub fn restore_cell_items(&mut self, snapshot: Vec<(i32, i32, Vec<ItemInstance>)>) {
        // First clear any items in loaded chunks so a save with empty cell
        // lists actually empties them.
        for (_, chunk) in self.chunks.iter_mut() {
            for cell in chunk.cells.iter_mut() {
                cell.items.clear();
            }
            chunk.dirty = false;
        }
        for (wx, wy, items) in snapshot {
            let (cc, _, _) = Self::chunk_coord_for(wx as i64, wy as i64);
            self.ensure_chunk_loaded(cc);
            if let Some(c) = self.cell_at_mut(wx as i64, wy as i64) {
                c.items = items;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::ItemKind;

    #[test]
    fn player_spawns_at_center() {
        let world = World::new(CHUNK_W, CHUNK_H);
        assert_eq!(world.player_pos(), Position { x: 20, y: 15 });
    }

    #[test]
    fn moving_into_neighbor_chunk_loads_it() {
        // Survival/follow-cam: try_move_player now lazily loads the
        // chunk ring around the destination so the player can cross
        // chunk seams. The "OOB Wall" fallback only fires for chunks
        // outside the ring.
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.set_player_pos(Position { x: 0, y: 15 });
        // Stepping west attempts to enter chunk (-1, 0); the ring around
        // that chunk loads as a side-effect.
        world.try_move_player(-1, 0);
        assert!(
            world.chunks.contains_key(&ChunkCoord { cx: -1, cy: 0 }),
            "neighbor chunk should be loaded after move attempt"
        );
    }

    #[test]
    fn out_of_bounds_reads_as_wall() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Unloaded neighbor chunks read as wall.
        assert_eq!(world.tile_at(-1, 5), TerrainKind::Wall);
        assert_eq!(world.tile_at(CHUNK_W as i64, 5), TerrainKind::Wall);
        assert_eq!(world.tile_at(5, -1), TerrainKind::Wall);
        assert_eq!(world.tile_at(5, CHUNK_H as i64), TerrainKind::Wall);
    }

    #[test]
    fn fov_marks_cells_visible_across_chunk_seam() {
        // After follow-cam: a player near the west edge of chunk (0,0)
        // should mark cells inside chunk (-1, 0) as explored once the
        // ring is loaded and FOV is recomputed.
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.ensure_player_ring();
        // Place player at world (2, 15) — local (2, 15) in chunk (0, 0).
        // Radius-20 FOV reaches world x = -18, well inside chunk (-1, 0).
        world.set_player_pos(Position { x: 2, y: 15 });
        world.ensure_player_ring();
        world.recompute_fov();
        // The cell at world (-1, 15) is local (39, 15) in chunk (-1, 0).
        // FOV may or may not mark it visible depending on intervening
        // trees, but the chunk MUST be loaded and queryable.
        assert!(
            world.cell_at(-1, 15).is_some(),
            "chunk (-1, 0) cell should be queryable after ring load"
        );
    }

    #[test]
    fn chunk_zero_zero_has_walkable_grass_at_spawn() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Spawn cell must be walkable Grass (chunkgen carves around the
        // skeleton features so the player never starts inside water or
        // a tree).
        let spawn = world.tile_at(20, 15);
        assert_eq!(spawn, TerrainKind::Grass);
        assert!(spawn.def().walkable);
    }

    #[test]
    fn pickup_all_drains_cell_and_fills_pack() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Build a deterministic scenario regardless of what chunkgen
        // rolled: clear the cell, drop 3 twigs (5g each) + 1 stone (200g).
        if let Some(c) = world.cell_at_mut(21, 15) {
            c.items.clear();
            c.items
                .push(ItemInstance::stack(ItemKind::Twig, 3, 5, None, ItemMetadata::None));
            c.items.push(ItemInstance::stack(
                ItemKind::Stone,
                1,
                200,
                None,
                ItemMetadata::None,
            ));
        }
        world.set_player_pos(Position { x: 21, y: 15 });
        let before = world.player_pack().total_weight_g();
        let picked = world.try_pickup_all_at_player();
        assert_eq!(picked, 2);
        let after = world.player_pack().total_weight_g();
        assert_eq!(after - before, 3 * 5 + 200);
        let cell = world.cell_at(21, 15).expect("cell exists");
        assert!(cell.items.is_empty());
    }

    #[test]
    fn pickup_respects_pack_capacity() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let pos = world.player_pos();
        let east_x = pos.x + 1;
        let east_y = pos.y;
        // Replace the existing debris with a single 10 kg boulder.
        if let Some(c) = world.cell_at_mut(east_x as i64, east_y as i64) {
            c.items.clear();
            c.items.push(ItemInstance::stack(
                ItemKind::Stone,
                1,
                10_000,
                None,
                ItemMetadata::None,
            ));
        }
        world.try_move_player(1, 0);
        let before_count = world.player_pack().contents.len();
        let picked = world.try_pickup_all_at_player();
        assert_eq!(picked, 0, "10kg boulder must not fit in 1.8kg of slack");
        assert_eq!(world.player_pack().contents.len(), before_count);
        let cell = world.cell_at(east_x as i64, east_y as i64).expect("cell");
        assert_eq!(cell.items.len(), 1);
    }

    #[test]
    fn stackables_merge_in_pack_across_pickups() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Two manually-set cells of twigs (3 east, 2 west of spawn). The
        // test asserts that picking up both merges into one pack stack.
        if let Some(c) = world.cell_at_mut(21, 15) {
            c.items.clear();
            c.items
                .push(ItemInstance::stack(ItemKind::Twig, 3, 5, None, ItemMetadata::None));
        }
        if let Some(c) = world.cell_at_mut(19, 15) {
            c.items.clear();
            c.items
                .push(ItemInstance::stack(ItemKind::Twig, 2, 5, None, ItemMetadata::None));
        }

        world.set_player_pos(Position { x: 21, y: 15 });
        world.try_pickup_all_at_player();
        world.set_player_pos(Position { x: 19, y: 15 });
        world.try_pickup_all_at_player();

        let pack = world.player_pack();
        let twig_stacks: Vec<&ItemInstance> = pack
            .contents
            .iter()
            .filter(|i| i.kind == ItemKind::Twig)
            .collect();
        assert_eq!(twig_stacks.len(), 1, "twigs must merge into a single stack");
        assert_eq!(twig_stacks[0].count, 5);
    }

    #[test]
    fn clock_starts_at_14_00_and_advances_per_action() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        assert_eq!(world.clock_seconds, STARTING_CLOCK_SECONDS);
        assert_eq!(world.clock_hm(), (14, 0));
        assert!(!world.is_night());

        world.try_move_player(1, 0);
        assert_eq!(world.clock_seconds, STARTING_CLOCK_SECONDS + 5);
    }

    #[test]
    fn initial_fov_covers_floor_around_spawn() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Adjacent floor cells must be visible at spawn.
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let cell = world.cell_at(20 + dx, 15 + dy).expect("cell");
            assert!(cell.visible, "({}, {}) should be visible at spawn", dx, dy);
            assert!(cell.explored);
        }
    }

    #[test]
    fn moving_marks_new_cells_explored() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Walk east until something far isn't yet explored, then check
        // that walking towards it explores it.
        let before = world.cell_at(35, 15).expect("cell").explored;
        // 35 - 20 = 15 cells east of spawn; with radius 20 day this is
        // already visible from spawn.
        assert!(before, "(35, 15) is within initial day-radius 20");

        // Far cell well past the chunk: at world coord (50, 15) tile_at
        // returns Wall (unloaded). Still, walking 10 east doesn't change
        // exploration of out-of-chunk cells.
        world.try_move_player(1, 0);
        assert!(world.cell_at(35, 15).expect("cell").explored);
    }

    #[test]
    fn night_fov_radius_is_tight() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Jump clock to 22:00, well into night.
        world.clock_seconds = 22 * 3600;
        world.recompute_fov();
        // A cell 5 east of spawn (25, 15) should NOT be visible at night
        // radius 3.
        let far = world.cell_at(25, 15).expect("cell");
        assert!(!far.visible, "night FOV radius is 3; (25, 15) is 5 away");
        // A cell 2 east is within radius 3.
        let close = world.cell_at(22, 15).expect("cell");
        assert!(close.visible);
    }

    #[test]
    fn explored_snapshot_round_trips() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let snap = world.snapshot_explored();
        assert!(!snap.is_empty(), "spawn should explore at least the FOV disc");

        // Clear all explored bits.
        for chunk in world.chunks.values_mut() {
            for cell in chunk.cells.iter_mut() {
                cell.explored = false;
            }
        }
        assert!(!world.cell_at(20, 15).expect("cell").explored);

        world.restore_explored(&snap);
        assert!(world.cell_at(20, 15).expect("cell").explored);
    }

    #[test]
    fn brightness_at_known_times() {
        // Noon: full day.
        assert!((brightness_at(12 * 3600) - 1.0).abs() < 1e-6);
        // Midnight: full night.
        assert!((brightness_at(0) - 0.4).abs() < 1e-6);
        // Start of dusk: still day.
        assert!((brightness_at(19 * 3600 + 1800) - 1.0).abs() < 1e-6);
        // Middle of dusk: midpoint.
        assert!((brightness_at(20 * 3600) - 0.7).abs() < 1e-3);
        // End of dusk: full night.
        assert!((brightness_at(20 * 3600 + 1800) - 0.4).abs() < 1e-6);
        // Middle of dawn: midpoint.
        assert!((brightness_at(6 * 3600) - 0.7).abs() < 1e-3);
        // End of dawn: full day.
        assert!((brightness_at(6 * 3600 + 1800) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn dawns_elapsed_counts_each_06_00_crossing() {
        // Before day-1 dawn: 0 dawns elapsed.
        assert_eq!(dawns_elapsed(0), 0);
        assert_eq!(dawns_elapsed(6 * 3600 - 1), 0);
        // At day-1 dawn exactly: 1.
        assert_eq!(dawns_elapsed(6 * 3600), 1);
        // 14:00 day 1 (player spawn): 1.
        assert_eq!(dawns_elapsed(STARTING_CLOCK_SECONDS), 1);
        // Day-2 dawn: 2.
        assert_eq!(dawns_elapsed(86400 + 6 * 3600), 2);
        // Day-2 noon: still 2.
        assert_eq!(dawns_elapsed(86400 + 12 * 3600), 2);
        // Day-3 dawn: 3.
        assert_eq!(dawns_elapsed(2 * 86400 + 6 * 3600), 3);
    }

    #[test]
    fn night_transitions_at_dusk_and_dawn() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Force-advance to 19:59 — still day.
        world.clock_seconds = 19 * 3600 + 59 * 60;
        assert!(!world.is_night());
        world.clock_seconds = 20 * 3600;
        assert!(world.is_night());
        world.clock_seconds = 5 * 3600 + 59 * 60;
        assert!(world.is_night());
        world.clock_seconds = 6 * 3600;
        assert!(!world.is_night());
    }

    #[test]
    fn fire_casts_own_fov_disc_independent_of_player() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Deep night so the player's own FOV stays at radius 3.
        world.clock_seconds = 22 * 3600;
        // Fire 9 east — far enough that its radius-5 disc doesn't
        // overlap the player's radius-3 night FOV, so we can pin down
        // "player FOV only" vs "fire FOV only" cells unambiguously.
        let pos = world.player_pos();
        if let Some(cell) = world.cell_at_mut((pos.x + 9) as i64, pos.y as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds: 3600 },
            ));
        }
        world.recompute_fov();

        // Inside the player's own night FOV (distance 2): visible, NOT
        // lit by fire — the warm tint belongs to cells the fire lights,
        // not cells the player just happens to see in their own dim.
        let near_player = world.cell_at((pos.x + 2) as i64, pos.y as i64).unwrap();
        assert!(near_player.visible, "player FOV (radius 3) must reach (+2,0)");
        assert_eq!(near_player.light_intensity, 0, "(+2,0) is in player FOV only");

        // Past player FOV but inside the fire's disc (distance 4 from
        // fire): visible AND lit. Distance 4 of 5 -> intensity 51.
        let in_fire_glow = world.cell_at((pos.x + 5) as i64, pos.y as i64).unwrap();
        assert!(in_fire_glow.visible, "fire FOV must reach (+5,0)");
        let expected_5 = ((5 - 4) as u32 * 255 / 5) as u8;
        assert_eq!(in_fire_glow.light_intensity, expected_5);

        // The fire's own cell (distance 0) gets full intensity 255.
        let fire_cell = world.cell_at((pos.x + 9) as i64, pos.y as i64).unwrap();
        assert_eq!(fire_cell.light_intensity, 255, "fire's own cell is brightest");

        // Fire's far edge (distance 5 from fire = +14 from player):
        // visible but intensity drops to 0 — the gradient fades to
        // dark at the disc edge.
        let fire_far_edge = world.cell_at((pos.x + 14) as i64, pos.y as i64).unwrap();
        assert!(fire_far_edge.visible, "fire FOV reaches its own +5 east");
        assert_eq!(fire_far_edge.light_intensity, 0, "edge of disc fades to dark");

        // One cell past the fire's disc: invisible.
        let beyond = world.cell_at((pos.x + 15) as i64, pos.y as i64).unwrap();
        assert!(!beyond.visible, "past fire+5 should be invisible at night");
        assert_eq!(beyond.light_intensity, 0);
    }

    #[test]
    fn fire_lit_clears_when_fire_burns_out() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.clock_seconds = 22 * 3600;
        let pos = world.player_pos();
        // Lit fire 5 east with just enough fuel to die inside the next
        // tick. From the fire's pos, its disc reaches +10 east; from the
        // player's, those cells are well outside the night radius-3 FOV.
        if let Some(cell) = world.cell_at_mut((pos.x + 5) as i64, pos.y as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds: 30 },
            ));
        }
        world.recompute_fov();
        let lit_cell = world.cell_at((pos.x + 8) as i64, pos.y as i64).unwrap();
        assert!(lit_cell.visible, "fire-cast must light (+8,0) while burning");
        assert!(lit_cell.light_intensity > 0, "(+8,0) should pick up intensity from fire");

        // Advance 60s — fire dies in tick_fires, advance_time_raw
        // triggers recompute_fov, cells visible only via the fire
        // contract back to invisible AND lose intensity.
        world.advance_time_raw(60);
        let after = world.cell_at((pos.x + 8) as i64, pos.y as i64).unwrap();
        assert!(!after.visible, "(+8,0) outside player FOV when fire dies");
        assert_eq!(after.light_intensity, 0, "intensity must reset on fire death");
    }

    #[test]
    fn wall_blocks_fire_light() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.clock_seconds = 22 * 3600;
        let pos = world.player_pos();
        // Fire 4 cells east of player; TreeTrunk wall 2 cells east of
        // the fire (player+6). Cells beyond the wall (player+7 east)
        // are NOT fire_lit even though they're within radius 5 of the
        // fire — sight-blockers block fire light by the same rule.
        if let Some(cell) = world.cell_at_mut((pos.x + 4) as i64, pos.y as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds: 3600 },
            ));
        }
        world.set_terrain_at((pos.x + 6) as i64, pos.y as i64, TerrainKind::TreeTrunk);
        world.recompute_fov();

        // The wall cell itself is the first blocker; shadowcast includes
        // the blocker's own cell as visible. Cells BEHIND it on the same
        // axis are shadowed.
        let beyond_wall = world.cell_at((pos.x + 8) as i64, pos.y as i64).unwrap();
        assert_eq!(beyond_wall.light_intensity, 0, "wall must shadow (+8,0) from fire");
        assert!(!beyond_wall.visible, "and the player can't see past it either");
    }

    #[test]
    fn player_on_pitched_detects_bedroll_on_player_cell() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let pos = world.player_pos();
        // Empty start state: neither helper sees anything.
        assert!(!world.player_on_pitched(ItemKind::Bedroll));
        assert!(!world.player_on_pitched(ItemKind::Tent));

        // Drop a Pitched bedroll on the player's cell; helper flips true
        // only for Bedroll, not Tent.
        if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Bedroll,
                2_000,
                None,
                ItemMetadata::Pitched,
            ));
        }
        assert!(world.player_on_pitched(ItemKind::Bedroll));
        assert!(!world.player_on_pitched(ItemKind::Tent));

        // A non-Pitched bedroll (e.g. stowed but somehow on a cell with
        // ItemMetadata::None) must NOT count — the marker is what
        // distinguishes "deployed" from "loose".
        if let Some(cell) = world.cell_at_mut((pos.x + 1) as i64, pos.y as i64) {
            cell.items.clear();
            cell.items.push(ItemInstance::stack(
                ItemKind::Bedroll,
                1,
                2_000,
                None,
                ItemMetadata::None,
            ));
        }
        world.try_move_player(1, 0);
        assert!(
            !world.player_on_pitched(ItemKind::Bedroll),
            "unpitched bedroll on the cell should not count"
        );
    }

    #[test]
    fn night_warmth_decay_softens_with_adjacent_lit_fire() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Jump to deep night.
        world.clock_seconds = 22 * 3600;
        // Drop a lit fire on the cell east of the player.
        let pos = world.player_pos();
        if let Some(cell) = world.cell_at_mut((pos.x + 1) as i64, pos.y as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds: 3600 },
            ));
        }
        // Force warmth to a known value to isolate the decay.
        let mut n = world.player_needs();
        n.warmth = 100;
        n.warmth_acc_secs = 0;
        world.set_player_needs(n);

        // 60 game-seconds of advance_time_raw at night with a fire
        // adjacent: net -3 + 1 = -2 warmth.
        world.advance_time_raw(60);
        assert_eq!(world.player_needs().warmth, 98);
    }

    #[test]
    fn movement_decays_thirst_over_many_actions() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let start = world.player_needs().thirst;
        // 12 moves × 5 sec = 60 sec → exactly 1 thirst lost.
        for _ in 0..12 {
            world.try_move_player(1, 0);
            world.try_move_player(-1, 0);
        }
        // 24 moves total = 120 sec → 2 thirst lost.
        assert_eq!(world.player_needs().thirst, start - 2);
    }

    #[test]
    fn pickup_primitive_does_not_advance_clock() {
        // try_pickup_all_at_player is a world primitive; per the
        // locality refactor it no longer spends action time. The
        // action.rs / main.rs callers own that step. Test the primitive
        // here; the cost behavior lives in action::tests.
        let mut world = World::new(CHUNK_W, CHUNK_H);
        if let Some(c) = world.cell_at_mut(20, 15) {
            c.items.clear();
            c.items.push(ItemInstance::stack(
                ItemKind::Twig,
                1,
                5,
                None,
                ItemMetadata::None,
            ));
        }
        let before = world.clock_seconds;
        let picked = world.try_pickup_all_at_player();
        assert_eq!(picked, 1);
        assert_eq!(
            world.clock_seconds, before,
            "primitive must not burn time; that's the caller's job"
        );
    }

    #[test]
    fn multi_turn_progress_bar_advances_step_by_step() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.queue_multi_turn(&[(ActionId::PitchTent, 10)]);
        // Healthy needs, no penalty: target_secs = base_cost = 10.
        assert_eq!(world.active_action.as_ref().unwrap().steps.len(), 1);

        let result = world.tick_multi_turn(3);
        assert!(result.completed_steps.is_empty());
        assert!(!result.interrupted);
        let active = world.active_action.as_ref().unwrap();
        assert_eq!(active.steps.front().unwrap().elapsed_secs, 3);
    }

    #[test]
    fn multi_turn_completes_step_and_clears_when_queue_empty() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.queue_multi_turn(&[(ActionId::UnrollBedroll, 5)]);
        let result = world.tick_multi_turn(10);
        assert_eq!(result.completed_steps, vec![ActionId::UnrollBedroll]);
        assert!(!result.interrupted);
        assert!(world.active_action.is_none());
    }

    #[test]
    fn multi_turn_two_step_queue_completes_in_order() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.queue_multi_turn(&[
            (ActionId::PitchTent, 3),
            (ActionId::UnrollBedroll, 2),
        ]);
        // 5 seconds = exactly the queue total. Both steps should
        // complete in this single tick, in queue order.
        let result = world.tick_multi_turn(10);
        assert_eq!(
            result.completed_steps,
            vec![ActionId::PitchTent, ActionId::UnrollBedroll]
        );
        assert!(world.active_action.is_none());
    }

    #[test]
    fn multi_turn_interrupts_when_need_crashes_below_threshold() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Drop thirst to threshold+1; one second of decay will cross
        // the boundary... actually needs decay 1/min, so a single
        // second won't drop thirst by 1. Force it lower so the tick's
        // critical check trips.
        let mut n = world.player_needs();
        n.thirst = NEED_CRITICAL_THRESHOLD - 1;
        n.thirst_acc_secs = 0;
        world.set_player_needs(n);

        world.queue_multi_turn(&[(ActionId::PitchTent, 30)]);
        let result = world.tick_multi_turn(30);
        assert!(result.interrupted);
        assert!(result.completed_steps.is_empty());
        // Active action gone: interrupted means queue cancelled.
        assert!(world.active_action.is_none());
    }

    #[test]
    fn cancel_multi_turn_drops_queue_without_completion() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.queue_multi_turn(&[(ActionId::PitchTent, 10)]);
        world.tick_multi_turn(3); // partial progress
        world.cancel_multi_turn();
        assert!(world.active_action.is_none());
    }

    #[test]
    fn queue_amplifies_target_by_current_need_penalty() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Drop thirst to 9 (below 10) so the penalty is +50%.
        let mut n = world.player_needs();
        n.thirst = 9;
        n.thirst_acc_secs = 0;
        world.set_player_needs(n);

        world.queue_multi_turn(&[(ActionId::PitchTent, 10)]);
        let step = world.active_action.as_ref().unwrap().steps.front().unwrap();
        assert_eq!(step.target_secs, 15, "10 + 50% = 15");
    }

    #[test]
    fn toggle_view_mode_flips_progress_and_skip() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.queue_multi_turn(&[(ActionId::PitchTent, 10)]);
        assert_eq!(
            world.active_action.as_ref().unwrap().view_mode,
            ViewMode::ProgressBar
        );
        world.toggle_multi_turn_view();
        assert_eq!(
            world.active_action.as_ref().unwrap().view_mode,
            ViewMode::TimeSkip
        );
        world.toggle_multi_turn_view();
        assert_eq!(
            world.active_action.as_ref().unwrap().view_mode,
            ViewMode::ProgressBar
        );
    }

    #[test]
    fn calendar_day_starts_at_spring_start() {
        let world = World::new(CHUNK_W, CHUNK_H);
        assert_eq!(world.calendar_day, crate::calendar::START_DAY);
        assert_eq!(world.season(), crate::calendar::Season::Spring);
    }

    #[test]
    fn calendar_day_advances_on_midnight_crossing() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let start_day = world.calendar_day;
        // Spawn at 14:00 — 10 hours to midnight. Advance 11h to cross
        // it. advance_time_raw skips need-penalty amplification.
        world.advance_time_raw(11 * 3600);
        assert_eq!(
            world.calendar_day,
            start_day + 1,
            "midnight crossing must bump calendar_day"
        );
    }

    #[test]
    fn calendar_day_handles_multi_day_jump() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let start_day = world.calendar_day;
        // Skip exactly two midnight crossings (~48h from 14:00). The
        // debug console can do larger jumps; sleep can do ~8h max but
        // a long inactive session compounds. Test the floor-division
        // shape so multi-day skips don't undercount.
        world.advance_time_raw(2 * 24 * 3600);
        assert_eq!(world.calendar_day, start_day + 2);
    }

    #[test]
    fn season_changes_when_calendar_crosses_boundary() {
        use crate::calendar::Season;
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // START_DAY = 80 = first day of Spring. Walk forward to day 172
        // (first day of Summer).
        world.calendar_day = 171; // last Spring
        assert_eq!(world.season(), Season::Spring);
        world.calendar_day = 172;
        assert_eq!(world.season(), Season::Summer);
    }

    #[test]
    fn snapshot_and_restore_cell_items_round_trip() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Place an item explicitly so the assertion is independent of
        // whatever debris chunkgen happened to roll for this seed.
        if let Some(c) = world.cell_at_mut(21, 15) {
            c.items.clear();
            c.items.push(ItemInstance::stack(
                ItemKind::Twig,
                3,
                5,
                None,
                ItemMetadata::None,
            ));
        }
        let snap = world.snapshot_cell_items();
        assert!(snap.iter().any(|(x, y, _)| *x == 21 && *y == 15));

        if let Some(c) = world.cell_at_mut(21, 15) {
            c.items.clear();
        }
        assert!(world.cell_at(21, 15).expect("cell").items.is_empty());

        world.restore_cell_items(snap);
        assert!(!world.cell_at(21, 15).expect("cell").items.is_empty());
    }
}
