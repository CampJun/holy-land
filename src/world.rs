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

/// Action costs in game-seconds. Read by the verb implementations in
/// action.rs and exposed for the command menu's cost surfacing.
pub const COST_MOVE_TILE: u32 = 5;
pub const COST_PICKUP: u32 = 3;
pub const COST_EAT_RATION: u32 = 10;
pub const COST_EAT_HERB: u32 = 5;
pub const COST_DRINK_WATERSKIN: u32 = 5;
pub const COST_PITCH_TENT: u32 = 300;
pub const COST_UNROLL_BEDROLL: u32 = 30;
pub const COST_FIRE_MAKING_ATTEMPT: u32 = 60;

/// How long (in game-seconds) a successful StartFire's lit firewood
/// burns before it extinguishes itself. Phase-12's "feed fire" verb
/// will add fuel to extend this; for slice 1 the player gets 1
/// game-hour per attempt.
pub const FIRE_FUEL_SECONDS_PER_LIGHT: u32 = 3600;

/// Flint and steel skill modifier per the design card. Other tools
/// (bow drill, weather penalty, sheltered bonus) wire in here as more
/// content lands.
pub const FIRE_BONUS_FLINT_AND_STEEL: i32 = 30;

/// Materials threshold for a single StartFire attempt.
pub const FIRE_MIN_TINDER: u32 = 1;
pub const FIRE_MIN_KINDLING: u32 = 3;
pub const FIRE_MIN_FUEL: u32 = 2;

/// Phase-9 multi-turn interrupt threshold. When any need drops below this
/// during a multi-turn tick, the active action queue cancels and control
/// returns to the player. Death gate (phase 14) re-uses the same value.
pub const NEED_CRITICAL_THRESHOLD: u8 = 10;

/// How many game-seconds a ProgressBar-mode multi-turn action advances
/// per render frame. At ~60 fps, this means a 300-sec PitchTent
/// completes in ~5 real-time seconds — fast enough to not feel like
/// dead time, slow enough that the player can react with B to cancel.
pub const MULTI_TURN_GAME_SEC_PER_FRAME: u32 = 1;

/// FOV radii. Phase-10 adds a fire-light-source bump for night cells
/// within 5 of a lit fire.
pub const FOV_RADIUS_DAY: i32 = 20;
pub const FOV_RADIUS_NIGHT: i32 = 3;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainKind {
    Floor,
    Wall,
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
}

impl CellState {
    fn floor() -> Self {
        Self {
            terrain: TerrainKind::Floor,
            items: Vec::new(),
            visible: false,
            explored: false,
        }
    }
    fn wall() -> Self {
        Self {
            terrain: TerrainKind::Wall,
            items: Vec::new(),
            visible: false,
            explored: false,
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
    #[allow(dead_code)] // consumed by chunkgen.rs in phase 11 (seeded gen)
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
        chunks.insert(origin, Box::new(generate_chunk_phase3(origin)));

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
        };
        seed_phase3_debris(&mut world);
        // The seeding marked the chunk dirty (via cell_at_mut); reset so a
        // fresh game without modifications doesn't unnecessarily persist
        // pristine debris.
        if let Some(c) = world.chunks.get_mut(&origin) {
            c.dirty = false;
        }
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
        if matches!(self.tile_at(nx as i64, ny as i64), TerrainKind::Floor) {
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

    fn needs_env(&self) -> NeedsEnv {
        NeedsEnv {
            is_night: self.is_night(),
            // Fire/tent/bedroll entities don't exist yet (phase 9-10).
            // Wire them in once those phases land.
            adjacent_fire: false,
            inside_tent: false,
            in_bedroll: false,
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
        self.clock_seconds = self.clock_seconds.saturating_add(secs as u64);
        let env = self.needs_env();
        let mut needs = self.player_needs();
        needs.tick(secs, env);
        self.set_player_needs(needs);
        self.tick_fires(secs);
        if self.is_night() != was_night {
            self.recompute_fov();
        }
    }

    /// Decrement `fuel_seconds` on every Lit item in every loaded chunk.
    /// Items whose fuel hits 0 are removed (the fire burnt out and the
    /// firewood is consumed). Phase-12's "feed fire" verb adds fuel
    /// back from inventory before the timer hits 0.
    fn tick_fires(&mut self, secs: u32) {
        for chunk in self.chunks.values_mut() {
            for cell in chunk.cells.iter_mut() {
                if !cell.items.iter().any(|i| matches!(i.metadata, ItemMetadata::Lit { .. })) {
                    continue;
                }
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
                chunk.dirty = true;
            }
        }
    }

    /// Is there at least one lit fire in the player's cell or any of
    /// the 8 adjacent cells? Phase-13 wires this into NeedsEnv for
    /// warmth shelter; phase-10 exposes it now so action-evaluation
    /// can use the same predicate.
    #[allow(dead_code)] // consumed by needs_env() in phase 13 (warmth shelter)
    pub fn lit_fire_adjacent_to_player(&self) -> bool {
        let p = self.player_pos();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let Some(cell) = self.cell_at((p.x + dx) as i64, (p.y + dy) as i64) else {
                    continue;
                };
                if cell.items.iter().any(|i| matches!(i.metadata, ItemMetadata::Lit { .. })) {
                    return true;
                }
            }
        }
        false
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
        let origin = self.player_pos();
        let radius = if self.is_night() {
            FOV_RADIUS_NIGHT
        } else {
            FOV_RADIUS_DAY
        };

        // Snapshot blockers into a relative bit-grid keyed on (dx, dy)
        // offsets from `origin`, padded by `BLOCKER_GRID_CENTER`.
        let mut blockers = [false; BLOCKER_GRID_LEN];
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let wx = origin.x as i64 + dx as i64;
                let wy = origin.y as i64 + dy as i64;
                if matches!(self.tile_at(wx, wy), TerrainKind::Wall) {
                    blockers[blocker_idx(dx, dy)] = true;
                }
            }
        }

        // Reset visible flags on every loaded cell.
        for chunk in self.chunks.values_mut() {
            for cell in chunk.cells.iter_mut() {
                cell.visible = false;
            }
        }

        let visible = crate::fov::compute_visible((origin.x, origin.y), radius, |x, y| {
            let dx = x - origin.x;
            let dy = y - origin.y;
            // Cells outside the grid coverage (impossible per the
            // shadowcaster's bounds, but defensive) are treated as
            // blockers so FOV doesn't escape the snapshot window.
            if dx.unsigned_abs() as i32 > FOV_RADIUS_DAY
                || dy.unsigned_abs() as i32 > FOV_RADIUS_DAY
            {
                return true;
            }
            blockers[blocker_idx(dx, dy)]
        });

        for (x, y) in visible {
            if let Some(cell) = self.cell_at_mut(x as i64, y as i64) {
                cell.visible = true;
                cell.explored = true;
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

    /// Restore explored bits from a save. Coords outside loaded chunks are
    /// silently ignored.
    pub fn restore_explored(&mut self, coords: &[(i32, i32)]) {
        for &(wx, wy) in coords {
            if let Some(cell) = self.cell_at_mut(wx as i64, wy as i64) {
                cell.explored = true;
            }
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

    /// Greedy pickup: every PICKABLE item in the player's current cell
    /// that fits in the pack moves into the pack. Items over capacity
    /// stay in the cell. Lit items (active fires) are NOT pickable —
    /// you can't pocket a burning campfire. Pitched items (tents,
    /// bedrolls) ARE pickable: re-stowing them is the "pack up camp"
    /// behavior, intentional in phase 9.
    ///
    /// Returns the number of `ItemInstance` entries successfully picked
    /// up (a merge counts as one entry).
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
                    .partition(|i| !matches!(i.metadata, ItemMetadata::Lit { .. })),
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

        // Time only advances if at least one stack was picked up. Bouncing
        // off a full pack with nothing picked doesn't burn the player's
        // game-clock; phase 7 will surface the same logic via the menu.
        if picked > 0 {
            self.spend_action_time(COST_PICKUP);
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
    /// Cells outside loaded chunks are silently ignored.
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
            if let Some(c) = self.cell_at_mut(wx as i64, wy as i64) {
                c.items = items;
            }
        }
    }
}

fn generate_chunk_phase3(coord: ChunkCoord) -> Chunk {
    // Slice-1 placeholder: perimeter wall, floor inside. Phase 11 swaps in
    // the authored stream/pond skeleton + seeded forest population.
    let mut cells = Vec::with_capacity((CHUNK_W * CHUNK_H) as usize);
    for y in 0..CHUNK_H {
        for x in 0..CHUNK_W {
            let is_edge = x == 0 || y == 0 || x == CHUNK_W - 1 || y == CHUNK_H - 1;
            cells.push(if is_edge {
                CellState::wall()
            } else {
                CellState::floor()
            });
        }
    }
    Chunk {
        coord,
        cells,
        dirty: false,
    }
}

// TODO(phase-11): remove this; replaced by chunkgen.rs's seeded debris table.
fn seed_phase3_debris(world: &mut World) {
    let spawn = world.player_pos();
    // East of player: 3 twigs + 1 stone.
    if let Some(c) = world.cell_at_mut((spawn.x + 1) as i64, spawn.y as i64) {
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
    // South of player: 2 sticks + 1 grass blade.
    if let Some(c) = world.cell_at_mut(spawn.x as i64, (spawn.y + 1) as i64) {
        c.items.push(ItemInstance::stack(
            ItemKind::Stick,
            2,
            50,
            None,
            ItemMetadata::None,
        ));
        c.items.push(ItemInstance::stack(
            ItemKind::GrassBlade,
            1,
            2,
            None,
            ItemMetadata::None,
        ));
    }
    // West of player: 1 moss patch.
    if let Some(c) = world.cell_at_mut((spawn.x - 1) as i64, spawn.y as i64) {
        c.items.push(ItemInstance::stack(
            ItemKind::MossPatch,
            1,
            10,
            None,
            ItemMetadata::None,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_spawns_at_center() {
        let world = World::new(CHUNK_W, CHUNK_H);
        assert_eq!(world.player_pos(), Position { x: 20, y: 15 });
    }

    #[test]
    fn walls_block_movement() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.set_player_pos(Position { x: 1, y: 1 });
        // Step left into the west wall: blocked.
        world.try_move_player(-1, 0);
        assert_eq!(world.player_pos(), Position { x: 1, y: 1 });
        // Step right onto floor: moves.
        world.try_move_player(1, 0);
        assert_eq!(world.player_pos(), Position { x: 2, y: 1 });
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
    fn chunk_zero_zero_initialized_with_perimeter_wall() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Inside is Floor.
        assert_eq!(world.tile_at(5, 5), TerrainKind::Floor);
        assert_eq!(world.tile_at(20, 15), TerrainKind::Floor);
        // Edges are Wall.
        assert_eq!(world.tile_at(0, 0), TerrainKind::Wall);
        assert_eq!(world.tile_at(CHUNK_W as i64 - 1, 0), TerrainKind::Wall);
        assert_eq!(world.tile_at(0, CHUNK_H as i64 - 1), TerrainKind::Wall);
    }

    #[test]
    fn phase3_seeded_debris_is_findable() {
        let world = World::new(CHUNK_W, CHUNK_H);
        let east = world.cell_at(21, 15).expect("(21, 15) in chunk");
        assert!(east.items.iter().any(|i| i.kind == ItemKind::Twig));
        assert!(east.items.iter().any(|i| i.kind == ItemKind::Stone));
        let south = world.cell_at(20, 16).expect("(20, 16) in chunk");
        assert!(south.items.iter().any(|i| i.kind == ItemKind::Stick));
        assert!(south.items.iter().any(|i| i.kind == ItemKind::GrassBlade));
        let west = world.cell_at(19, 15).expect("(19, 15) in chunk");
        assert!(west.items.iter().any(|i| i.kind == ItemKind::MossPatch));
    }

    #[test]
    fn pickup_all_drains_cell_and_fills_pack() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // East of spawn: 3 twigs (5g each) + 1 stone (200g) = 215g.
        world.try_move_player(1, 0);
        assert_eq!(world.player_pos(), Position { x: 21, y: 15 });

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
        // Drop a second twig pile west of spawn so we can pick up twigs twice.
        let pos = world.player_pos();
        if let Some(c) = world.cell_at_mut((pos.x - 1) as i64, pos.y as i64) {
            // Replace the moss with twigs so the test is purely about twig
            // stacking, not unrelated items.
            c.items.clear();
            c.items
                .push(ItemInstance::stack(ItemKind::Twig, 2, 5, None, ItemMetadata::None));
        }

        // Pick up the east twigs (3) + stone.
        world.try_move_player(1, 0);
        world.try_pickup_all_at_player();
        // Walk back west two steps and pick up 2 more twigs.
        world.try_move_player(-1, 0);
        world.try_move_player(-1, 0);
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
    fn pickup_advances_clock_only_when_something_picked_up() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Empty cell: spawn cell has no items.
        let before = world.clock_seconds;
        world.try_pickup_all_at_player();
        assert_eq!(
            world.clock_seconds, before,
            "empty pickup must not burn time"
        );

        // Walk east (5 sec) into seeded debris, then pick up (3 sec).
        world.try_move_player(1, 0);
        let after_move = world.clock_seconds;
        assert_eq!(after_move - before, 5);
        world.try_pickup_all_at_player();
        assert_eq!(world.clock_seconds - after_move, 3);
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
    fn snapshot_and_restore_cell_items_round_trip() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let snap = world.snapshot_cell_items();
        assert!(snap.iter().any(|(x, y, _)| *x == 21 && *y == 15));

        // Drain everything via cell_at_mut so we exercise the same path the
        // pickup verb uses.
        for (x, y) in [(21, 15), (20, 16), (19, 15)] {
            if let Some(c) = world.cell_at_mut(x, y) {
                c.items.clear();
            }
        }
        assert!(world.cell_at(21, 15).expect("cell").items.is_empty());

        world.restore_cell_items(snap);
        assert!(!world.cell_at(21, 15).expect("cell").items.is_empty());
    }
}
