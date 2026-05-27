mod action;
mod calendar;
mod chunkgen;
mod city;
mod combat;
mod cornwall;
mod crafting;
#[cfg(not(target_arch = "arm"))]
mod debug_console;
mod fasttravel;
mod flora;
mod fov;
mod input;
mod items;
mod logging;
mod needs;
mod platform;
mod render;
mod save;
mod skill;
mod world;

use std::io::Write;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::surface::Surface;

use input::{Action, Input};
use items::{ItemInstance, Pack};
use needs::Needs;
use render::{draw_glyph, load_atlas, CELL_SIZE};
use save::{
    ActionStepSave, ActiveActionSave, CellItemsSave, DecorationMutationSave, MetaSave, NeedsSave,
    RunSave, SaveHeader, SkillSave, SkillsSave, StaminaSave, TerrainMutationSave,
    TreeSpeciesMutationSave,
};
use skill::{Rng, Skill, SkillKind, Skills};
use world::{
    brightness_at, dawns_elapsed, ChunkCoord, FastTravelStep, GroundCover, Position, TerrainKind,
    ViewMode, World, MULTI_TURN_GAME_SEC_PER_FRAME, TREE_VARIANT_GLYPHS,
};

const WORLD_W: u32 = 40;
const WORLD_H: u32 = 30;
// Bundled character-set atlases. Player picks which one the binary loads
// at boot via a plain-text `atlas.txt` in `save_dir` (one of: "cp437",
// "aesomatica"). Missing / unrecognized → Cp437. See AtlasChoice below.
const CP437_PNG: &[u8] = include_bytes!("../assets/cp437_16x16.png");
const AESOMATICA_PNG: &[u8] = include_bytes!("../assets/Aesomatica_16x16.png");
const ATLAS_CONFIG_FILE: &str = "atlas.txt";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AtlasChoice {
    Cp437,
    Aesomatica,
}

impl AtlasChoice {
    fn from_save_key(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "aesomatica" => Self::Aesomatica,
            _ => Self::Cp437,
        }
    }

    fn png_bytes(self) -> &'static [u8] {
        match self {
            Self::Cp437 => CP437_PNG,
            Self::Aesomatica => AESOMATICA_PNG,
        }
    }
}
const META_FILE: &str = "meta.cbor";
const RUN_FILE: &str = "run.cbor";
const TARGET_FRAME: Duration = Duration::from_micros(16_667);
#[cfg(target_arch = "arm")]
const SLEEP_GUARD: Duration = Duration::from_millis(10);
#[cfg(not(target_arch = "arm"))]
const SLEEP_GUARD: Duration = Duration::from_millis(3);
const TIMING_LOG_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    glyph: u8,
    fg: Color,
    bg: Color,
}

/// Start-button pause-menu options. Render order = display order.
const PAUSE_OPTIONS: &[(PauseAction, &str)] = &[
    (PauseAction::Save, "Save"),
    (PauseAction::Quit, "Quit to desktop"),
    (PauseAction::ResetSave, "Delete save and reset"),
    (PauseAction::GlyphPalette, "CP437 glyph palette (dev)"),
];

#[derive(Clone, Copy, PartialEq)]
enum PauseAction {
    Save,
    Quit,
    ResetSave,
    GlyphPalette,
}

/// Phase 15 hold-Y radial overlay. Tap-Y (press+release within
/// HOLD_THRESHOLD) opens the full vertical menu as before; holding Y
/// past the threshold opens this 4-direction radial of common verbs,
/// and pressing a dpad direction while held fires the bound verb and
/// closes the overlay. Releasing Y without choosing a direction
/// closes silently.
const HOLD_THRESHOLD: Duration = Duration::from_millis(250);

const RADIAL_BINDINGS: [(RadialDir, action::ActionId, &str); 4] = [
    (RadialDir::Up, action::ActionId::Pickup, "Pickup"),
    (RadialDir::Right, action::ActionId::EatRation, "Eat"),
    (RadialDir::Down, action::ActionId::PickHerb, "Pick herb"),
    (RadialDir::Left, action::ActionId::DrinkWaterskin, "Drink"),
];

#[derive(Clone, Copy, PartialEq)]
enum RadialDir {
    Up,
    Down,
    Left,
    Right,
}

/// Cause of death: whichever need hit 0 first. Priority order picks
/// one when multiple zero out on the same tick (rare but possible).
#[derive(Clone, Copy, Debug)]
enum DeathCause {
    Thirst,
    Hunger,
    Cold,
    Exhaustion,
    Combat,
}

impl DeathCause {
    fn from_needs(n: &Needs) -> Option<Self> {
        if n.thirst == 0 {
            Some(Self::Thirst)
        } else if n.hunger == 0 {
            Some(Self::Hunger)
        } else if n.warmth == 0 {
            Some(Self::Cold)
        } else if n.sleep == 0 {
            Some(Self::Exhaustion)
        } else {
            None
        }
    }

    fn epitaph(self) -> &'static str {
        match self {
            Self::Thirst => "You died of thirst.",
            Self::Hunger => "You died of starvation.",
            Self::Cold => "You froze to death.",
            Self::Exhaustion => "You died of exhaustion.",
            Self::Combat => "Slain by a bandit.",
        }
    }
}

/// Select-button info hub tabs. Display order = `INFO_TABS`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InfoTab {
    Inventory,
    Crafting,
    Skills,
}

const INFO_TABS: &[InfoTab] = &[InfoTab::Inventory, InfoTab::Crafting, InfoTab::Skills];

impl InfoTab {
    fn label(self) -> &'static str {
        match self {
            InfoTab::Inventory => "Inventory",
            InfoTab::Crafting => "Crafting",
            InfoTab::Skills => "Skills",
        }
    }
    fn index(self) -> usize {
        INFO_TABS.iter().position(|&t| t == self).unwrap_or(0)
    }
    fn next(self) -> Self {
        INFO_TABS[(self.index() + 1) % INFO_TABS.len()]
    }
    fn prev(self) -> Self {
        INFO_TABS[(self.index() + INFO_TABS.len() - 1) % INFO_TABS.len()]
    }
}

struct InfoMenuState {
    tab: InfoTab,
    /// Cursor row within the currently-active tab. Reset to 0 when the
    /// tab changes.
    selected: usize,
}

/// Ranged targeting cursor — modal input state opened by the `Aim`
/// verb. Dpad moves the cursor; A commits the shot; B cancels. The
/// cursor lives in world coords so it lines up with the bandit's
/// rendered glyph regardless of camera scroll.
struct TargetCursor {
    pos: world::Position,
    max_range: u8,
}

impl TargetCursor {
    /// Open the cursor; snap onto the nearest visible hostile within
    /// the wielded bow's max range. Falls back to the player's tile.
    fn open(world: &World) -> Self {
        let player = world.player_pos();
        // Look up the wielded bow's range.
        let max_range = world
            .player_main_hand_kind()
            .and_then(|k| k.def().ranged.map(|r| r.max_range))
            .unwrap_or(10);
        let pos = world
            .nearest_visible_hostile(player, max_range)
            .and_then(|e| world.position_of(e))
            .unwrap_or(player);
        Self { pos, max_range }
    }

    fn move_by(&mut self, dx: i32, dy: i32) {
        self.pos.x = self.pos.x.saturating_add(dx);
        self.pos.y = self.pos.y.saturating_add(dy);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let save_dir = platform::save_dir();
    logging::init(&save_dir);
    log_info!("save dir: {}", save_dir.display());

    let mut meta = match save::load_meta(&save_dir.join(META_FILE)) {
        Ok(m) => {
            log_info!(
                "loaded meta save (counter={}, device={})",
                m.header.save_counter,
                m.header.device_id
            );
            m
        }
        Err(e) => {
            log_info!("no meta save loaded ({}); starting fresh", e);
            MetaSave::empty(SaveHeader::fresh(None))
        }
    };

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    let logical_w = WORLD_W * CELL_SIZE;
    let logical_h = WORLD_H * CELL_SIZE;

    let window = video
        .window("Survival", logical_w, logical_h)
        .position_centered()
        .resizable()
        .build()?;

    let mut canvas = window.into_canvas().accelerated().build()?;
    canvas.set_logical_size(logical_w, logical_h)?;
    {
        let info = canvas.info();
        log_info!(
            "renderer: {} (flags={:#x}) max_texture={}x{}",
            info.name,
            info.flags,
            info.max_texture_width,
            info.max_texture_height
        );
    }

    // Surface composition: every cell blits onto a CPU framebuffer; once per
    // frame the framebuffer is uploaded to a single streaming texture and that
    // texture is the only thing the renderer presents. This is the documented
    // working pattern on Onion's libSDL2 (mmiyoo backend) — its renderer drops
    // every per-cell call.
    let texture_creator = canvas.texture_creator();
    // Boot-time atlas pick. The config sits next to the binary (not in
    // save_dir) — easy to find and edit. On Miyoo this lands at
    // `App/HolyLand/atlas.txt`; on desktop, next to the built binary
    // (e.g. `target/release/atlas.txt`). Bootstraps with "cp437" on
    // first launch so the player has a discoverable file to edit later.
    // Failures are silent; the game still runs.
    let atlas_path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(ATLAS_CONFIG_FILE);
    if !atlas_path.exists() {
        let _ = std::fs::write(&atlas_path, "cp437\n");
    }
    let atlas_choice = std::fs::read_to_string(&atlas_path)
        .map(|s| AtlasChoice::from_save_key(&s))
        .unwrap_or(AtlasChoice::Cp437);
    log_info!("atlas: {:?} ({})", atlas_choice, atlas_path.display());
    let mut atlas = load_atlas(atlas_choice.png_bytes())?;
    let mut framebuf = Surface::new(logical_w, logical_h, PixelFormatEnum::ARGB8888)?;
    let mut present_tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::ARGB8888, logical_w, logical_h)?;

    let mut events = sdl.event_pump()?;
    let mut input = Input::new();

    // Boot seed selection. If a run save exists we use its seed (so the
    // wilderness layout reloads identically). Otherwise we pick a fresh
    // seed from the wall clock — this is what "generate a new world"
    // means in v1: same authored Cornwall, fresh per-chunk procgen.
    let maybe_run = save::load_run(&save_dir.join(RUN_FILE)).ok();
    let world_seed = maybe_run
        .as_ref()
        .map(|r| r.seed)
        .unwrap_or_else(fresh_world_seed);
    log_info!("world seed: 0x{:016X}", world_seed);
    let mut world = World::with_seed(WORLD_W, WORLD_H, world_seed);
    world.ensure_player_ring();
    let mut prev_meta_header = meta.header.clone();
    let mut prev_run_header: Option<SaveHeader> = None;

    if let Some(run) = maybe_run {
        log_info!(
            "loaded run save (player at {},{}, pack {}g, {} non-empty cells, clock {}s)",
            run.player_x,
            run.player_y,
            run.pack.capacity_g,
            run.cell_items.len(),
            run.clock_seconds,
        );
        prev_run_header = Some(run.header.clone());
        world.set_player_pos(Position {
            x: run.player_x,
            y: run.player_y,
        });
        // A pre-phase-3 save has no pack data (capacity_g == 0 and empty
        // contents); keep the freshly-built starting pack in that case.
        if run.pack.capacity_g > 0 || !run.pack.contents.is_empty() {
            world.replace_player_pack(Pack::from_save(&run.pack));
        }
        if !run.cell_items.is_empty() {
            let snapshot: Vec<(i32, i32, Vec<ItemInstance>)> = run
                .cell_items
                .iter()
                .map(|cs| {
                    let items = cs
                        .items
                        .iter()
                        .filter_map(ItemInstance::from_save)
                        .collect();
                    (cs.x, cs.y, items)
                })
                .collect();
            world.restore_cell_items(snapshot);
        }
        // Phase-4 clock + needs. Legacy saves (pre-phase-4) write zeros for
        // these defaults; treat zeros as "no data, keep starting state".
        if run.clock_seconds > 0 {
            world.clock_seconds = run.clock_seconds;
        }
        // Schema-v2 calendar_day. Defaults to START_DAY on saves
        // written before this field existed (via #[serde(default)]),
        // so a non-default value always reflects an explicit write.
        world.calendar_day = run.calendar_day;
        if run.needs.warmth > 0
            || run.needs.thirst > 0
            || run.needs.hunger > 0
            || run.needs.sleep > 0
        {
            world.set_player_needs(Needs {
                thirst: run.needs.thirst,
                hunger: run.needs.hunger,
                sleep: run.needs.sleep,
                warmth: run.needs.warmth,
                thirst_acc_secs: run.needs.thirst_acc_secs,
                hunger_acc_secs: run.needs.hunger_acc_secs,
                sleep_acc_secs: run.needs.sleep_acc_secs,
                warmth_acc_secs: run.needs.warmth_acc_secs,
            });
        }
        if !run.explored_cells.is_empty() {
            world.restore_explored(&run.explored_cells);
        }
        // Phase-9 active-action restore: rehydrate the queue + view mode
        // so a save mid-PitchTent resumes correctly. Steps with unknown
        // ActionId strings are dropped silently (forward-compat).
        if let Some(saved) = run.active_action.as_ref() {
            let steps: Vec<(action::ActionId, u32)> = saved
                .steps
                .iter()
                .filter_map(|s| {
                    action::ActionId::from_save_key(&s.id).map(|id| (id, s.target_secs))
                })
                .collect();
            if !steps.is_empty() {
                world.queue_multi_turn(&steps);
                // Restore elapsed counters for the current step (which
                // queue_multi_turn just zeroed) and the view mode.
                if let Some(active) = world.active_action.as_mut() {
                    for (i, src) in saved.steps.iter().enumerate() {
                        if let Some(dst) = active.steps.get_mut(i) {
                            dst.elapsed_secs = src.elapsed_secs;
                            dst.target_secs = src.target_secs;
                        }
                    }
                    active.view_mode = match saved.view_mode.as_str() {
                        "time_skip" => ViewMode::TimeSkip,
                        _ => ViewMode::ProgressBar,
                    };
                }
            }
        }
        // Phase-10 skills + RNG. Legacy saves leave SkillsSave at all-
        // zero defaults; treat all-zero as "no data, keep the freshly
        // built Skills::starting()" so loaded games don't suddenly
        // start with Fire Making 0.
        let saved_fm = run.skills.fire_making;
        if saved_fm.value > 0 || saved_fm.daily_xp > 0 {
            let saved_fo = run.skills.foraging;
            // Saves written before Phase C carry foraging=all-zero. Fall
            // back to the starting Foraging value so an existing player
            // doesn't get their (never-touched) foraging stat read as 0.
            let foraging = if saved_fo.value > 0 || saved_fo.daily_xp > 0 {
                Skill {
                    value: saved_fo.value,
                    daily_xp: saved_fo.daily_xp,
                }
            } else {
                Skills::starting().foraging
            };
            let conv = |s: save::SkillSave| Skill { value: s.value, daily_xp: s.daily_xp };
            let prof = run.skills.proficiencies;
            let proficiencies = skill::Proficiencies {
                knife: conv(prof.knife),
                sword: conv(prof.sword),
                falchion: conv(prof.falchion),
                axe: conv(prof.axe),
                mace_cudgel: conv(prof.mace_cudgel),
                quarterstaff: conv(prof.quarterstaff),
                spear_lance: conv(prof.spear_lance),
                gisarme_bill: conv(prof.gisarme_bill),
                unarmed: conv(prof.unarmed),
                bow: conv(prof.bow),
                crossbow: conv(prof.crossbow),
            };
            world.set_player_skills(Skills {
                fire_making: Skill {
                    value: saved_fm.value,
                    daily_xp: saved_fm.daily_xp,
                },
                foraging,
                melee: conv(run.skills.melee),
                ranged: conv(run.skills.ranged),
                dodge: conv(run.skills.dodge),
                block: conv(run.skills.block),
                light_armor: conv(run.skills.light_armor),
                medium_armor: conv(run.skills.medium_armor),
                heavy_armor: conv(run.skills.heavy_armor),
                proficiencies,
            });
            // Re-sync combat stats from the loaded URW skill values so
            // the player's in-fight bonuses reflect their long-run
            // training right after load.
            world.sync_combat_skills_from_skills();
        }
        if run.rng_state != 0 {
            world.rng = Rng::from_state(run.rng_state);
        }
        // Combat foundation: per-actor speed. Saves written before this
        // field existed default to BASELINE via #[serde(default)], so
        // loading is a one-liner — no zero-guard needed.
        world.set_player_speed(run.speed);
        // Schema v3 combat: restore player + hostiles. A v2 save has
        // both fields default and the World::new defaults stand. v3+
        // saves may carry either the legacy single-pool `player_health`
        // (no-op now — phase 2 ignores it) or the new body-part split.
        if let Some(bp) = run.player_body_parts.as_ref() {
            world.set_player_body(save_bp_to_world(bp));
        }
        if let Some(eq) = run.player_equipment.as_ref() {
            world.set_player_equipment(save_equipment_to_world(eq));
        }
        if let Some(stam) = run.player_stamina.as_ref() {
            world.set_player_stamina(world::Stamina {
                cur: stam.cur,
                max: stam.max,
                recent_combat_secs: stam.recent_combat_secs,
            });
        }
        if !run.hostiles.is_empty() {
            // Cornish-bandit literal is the only flavor we restore as
            // of phase 3. Unknown flavors fall through the default in
            // `restore_hostiles`.
            let static_flavor = |s: &str| -> &'static str {
                match s {
                    "cornish_bandit" => "cornish_bandit",
                    _ => "unknown",
                }
            };
            let restored: Vec<world::HostileSnapshot> = run
                .hostiles
                .iter()
                .map(|h| {
                    let main_hand = if h.wielded_kind.is_empty() {
                        None
                    } else {
                        items::ItemKind::from_save_key(&h.wielded_kind)
                    };
                    let off_hand = if h.off_hand_kind.is_empty() {
                        None
                    } else {
                        items::ItemKind::from_save_key(&h.off_hand_kind)
                    };
                    let worn_kinds: Vec<items::ItemKind> = h
                        .worn_kinds
                        .iter()
                        .filter_map(|s| items::ItemKind::from_save_key(s))
                        .collect();
                    let body = match h.body_parts.as_ref() {
                        Some(bp) => save_bp_to_world(bp),
                        None => world::BodyParts::starting_human(),
                    };
                    let stamina = h.stamina.as_ref().map(|s| world::Stamina {
                        cur: s.cur,
                        max: s.max,
                        recent_combat_secs: s.recent_combat_secs,
                    });
                    world::HostileSnapshot {
                        pos: world::Position { x: h.x, y: h.y },
                        body,
                        main_hand,
                        off_hand,
                        worn_kinds,
                        flavor: static_flavor(&h.flavor),
                        stamina,
                    }
                })
                .collect();
            world.restore_hostiles(restored);
        }
        // Phase-11b: restore terrain mutations (chopped trees, etc.)
        // after chunkgen has produced the chunk defaults.
        if !run.terrain_mutations.is_empty() {
            let snapshot: Vec<(i32, i32, world::TerrainKind)> = run
                .terrain_mutations
                .iter()
                .filter_map(|tm| world::TerrainKind::from_save_key(&tm.kind).map(|k| (tm.x, tm.y, k)))
                .collect();
            world.restore_terrain_mutations(snapshot);
        }
        // Phase-D: restore tree_species + decoration mutations. Empty
        // `species_key` decodes as None (chopped cell); unknown
        // species keys also collapse to None (forward-compat). These
        // overlay the chunkgen defaults that ensure_chunk_loaded just
        // re-applied via restore_terrain_mutations above.
        if !run.tree_species_mutations.is_empty() {
            let snap: Vec<(i32, i32, Option<flora::TreeSpecies>)> = run
                .tree_species_mutations
                .iter()
                .map(|m| {
                    let species = if m.species_key.is_empty() {
                        None
                    } else {
                        flora::TreeSpecies::from_save_key(&m.species_key)
                    };
                    (m.x, m.y, species)
                })
                .collect();
            world.restore_tree_species_mutations(snap);
        }
        if !run.decoration_mutations.is_empty() {
            let snap: Vec<(i32, i32, flora::Decoration)> = run
                .decoration_mutations
                .iter()
                .map(|m| (m.x, m.y, m.decoration))
                .collect();
            world.restore_decoration_mutations(snap);
        }
        // After restoring position, ensure the chunk ring around the
        // loaded player coord is in memory — otherwise the first FOV
        // cast on a non-origin load would see all-Wall around the player.
        world.ensure_player_ring();
        // Recompute FOV after restoring position so the visible set is
        // correct for the loaded clock + player coord. (World::new already
        // did a recompute, but the loaded position may differ.)
        world.recompute_fov();
    }

    // First-encounter bandit. Drops in on a fresh run AND on v2 saves
    // that predate the hostile-save field — both leave `has_any_hostile`
    // false. v3+ saves with hostiles restored skip this.
    if !world.has_any_hostile() {
        world.spawn_starter_bandit();
    }

    // Track dawn crossings for auto-save-on-dawn. Init from the (possibly
    // loaded) clock so a loaded save mid-day doesn't immediately re-save.
    let mut last_dawn_idx = dawns_elapsed(world.clock_seconds);

    // Tap-Y command menu state. None = closed; Some(i) = open, with row i
    // selected. Phase 15 adds the hold-Y radial overlay alongside this.
    let mut command_menu: Option<usize> = None;

    // Start-button pause menu (Save / Quit / Delete-save-and-reset). When
    // open, every other input mode is suspended and multi-turn actions
    // stop ticking. Replaces the previous "Start quits immediately".
    let mut pause_menu: Option<usize> = None;

    // Select-button info hub: tabbed read-only overlay. L/R cycle
    // between tabs (Inventory, Skills, ... extensible). View-only for
    // now; drop / examine verbs hook off the selected row in future
    // phases.
    let mut info_menu: Option<InfoMenuState> = None;

    // Phase 14 death gate. Set when player_needs().is_dead() flips
    // true (any need at 0 with DEATH_ENABLED on). Highest input
    // priority — blocks all other modes. A=new run, Start=quit.
    let mut dead: Option<DeathCause> = None;

    // Phase 15 hold-Y radial. y_press_at is set on the press edge
    // (detected per-frame by watching input.is_held(Y) transition).
    // If Y stays held past HOLD_THRESHOLD, radial_open flips true; a
    // dpad press while radial_open fires the bound verb, closes the
    // radial, and sets radial_consumed so the eventual Y release
    // doesn't ALSO open the vertical menu (the tap-fallback). A quick
    // press+release (Y up before HOLD_THRESHOLD, no direction chosen)
    // = tap = open vertical menu.
    let mut y_press_at: Option<Instant> = None;
    let mut radial_open: bool = false;
    let mut radial_consumed: bool = false;

    // Dev tool: X-button toggles a CP437 glyph palette overlay so we
    // can audit which bytes have which sprites in our custom atlas.
    // Browse with dpad; the header shows the highlighted byte's value
    // so we can pick replacements for the items.rs / world.rs glyph
    // fields.
    let mut glyph_palette: Option<u8> = None;

    // Ranged-targeting cursor. Opened by the `Aim` verb (via
    // `ExecuteOutcome::OpenAim`); A commits the shot, B cancels.
    // Sits between command_menu and pause priority — see input loop
    // below.
    let mut target_cursor: Option<TargetCursor> = None;

    // Overmap mode. `M` (desktop) toggles it. Cursor lives on the mode
    // struct; `last_overmap_destination` survives close/reopen so the
    // resume-from-interrupt UX (design doc §6.3) works.
    let mut overmap_mode: Option<OvermapMode> = None;
    let mut last_overmap_destination: Option<ChunkCoord> = None;
    // Per-frame counter for the flashing player @ on the overmap.
    let mut overmap_frame_count: u64 = 0;

    #[cfg(not(target_arch = "arm"))]
    let debug = debug_console::DebugConsole::spawn();

    let palette = Palette::default();

    // B-style per-cell diff renderer. `prev_cells` mirrors what we last painted
    // into `framebuf`; each frame we recompute the visible cells and only blit
    // the ones that differ. None entries force a paint on the first frame.
    //
    // CDDA follow-cam: when the camera shifts, we memmove the CPU framebuffer
    // by the same offset and shift `prev_cells` to match, so the "diff" loop
    // only repaints newly-revealed edge cells. The shifted texture must then
    // be uploaded full-rect (the streaming texture has no equivalent memmove).
    // See plan: ~/.claude/plans/lets-start-planning-our-dynamic-starfish.md.
    let viewport_cells = (WORLD_W * WORLD_H) as usize;
    let mut prev_cells: Vec<Option<Cell>> = vec![None; viewport_cells];
    let _ = framebuf.fill_rect(None, palette.letterbox);

    // Tracked across frames so each frame can compute (dx, dy) cell deltas
    // for the scroll-shift path. Initialised to a sentinel that forces the
    // first frame to fall through the non-scroll path (cam_x == cam_x_prev).
    let mut cam_x_prev: i64 = (world.player_pos().x as i64) - WORLD_W as i64 / 2;
    let mut cam_y_prev: i64 = (world.player_pos().y as i64) - WORLD_H as i64 / 2;

    let mut fps_count: u32 = 0;
    let mut fps_window = Instant::now();
    let mut timing_accum = FrameTiming::default();
    let mut prev_frame: Option<Instant> = None;

    'main: loop {
        let frame_start = Instant::now();
        let _dt = prev_frame
            .map(|p| frame_start.saturating_duration_since(p))
            .unwrap_or_default();
        prev_frame = Some(frame_start);

        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                _ => input.handle_sdl_event(&event),
            }
        }
        input.poll_gamepad();

        // Hold-Y radial detection (phase 15). Watch the press/release
        // edges of Y via is_held; the queued press events go through
        // the input loop below as usual but the Y key itself is
        // suppressed there so this state machine owns the menu open
        // semantics for Y.
        let y_held = input.is_held(Action::Y);
        if y_press_at.is_none() && y_held {
            // Press edge.
            y_press_at = Some(frame_start);
            radial_open = false;
            radial_consumed = false;
        } else if y_press_at.is_some() && !y_held {
            // Release edge.
            if !radial_open && !radial_consumed {
                // Quick tap with no direction chosen -> open vertical
                // menu (preserves the existing tap-Y behavior).
                if command_menu.is_none() && pause_menu.is_none() && dead.is_none() {
                    command_menu = Some(0);
                }
            }
            y_press_at = None;
            radial_open = false;
            radial_consumed = false;
        } else if let Some(t0) = y_press_at {
            // Still held; promote to radial once past threshold and
            // no higher-priority mode is on screen.
            if !radial_open
                && frame_start.saturating_duration_since(t0) >= HOLD_THRESHOLD
                && pause_menu.is_none()
                && dead.is_none()
                && command_menu.is_none()
            {
                radial_open = true;
            }
        }

        let drained_actions = input.drain();
        // Fast-travel watcher: any input action (other than the M
        // toggle that opens the map) cancels the queue. We compute this
        // BEFORE the input loop so the cancel fires for the SAME frame
        // the player pressed, even if the loop consumes the action
        // for another purpose. M is exempted so opening the overmap
        // during travel just cancels-and-shows-map naturally below.
        let cancels_fast_travel = world.fast_travel.is_some()
            && drained_actions
                .iter()
                .any(|a| !matches!(a, Action::Y | Action::OpenOvermap));
        for input_action in drained_actions {
            // Y press events are owned by the hold-Y radial state
            // machine above; drop them here so they don't double-fire
            // any menu open.
            if input_action == Action::Y {
                continue;
            }

            // Radial overlay (phase 15). Active while Y is held past
            // HOLD_THRESHOLD. Dpad direction fires the bound verb and
            // closes; everything else is dropped (so the player
            // doesn't accidentally walk while choosing).
            if radial_open {
                let dir = match input_action {
                    Action::Up => Some(RadialDir::Up),
                    Action::Down => Some(RadialDir::Down),
                    Action::Left => Some(RadialDir::Left),
                    Action::Right => Some(RadialDir::Right),
                    _ => None,
                };
                if let Some(d) = dir {
                    if let Some((_, id, name)) =
                        RADIAL_BINDINGS.iter().find(|(rd, _, _)| *rd == d)
                    {
                        match action::evaluate(&world, *id) {
                            action::Availability::Available { .. } => {
                                match action::execute(&mut world, *id) {
                                    action::ExecuteOutcome::Done(msg) => {
                                        log_info!("[radial] {}", msg);
                                    }
                                    action::ExecuteOutcome::OpenAim => {
                                        target_cursor = Some(TargetCursor::open(&world));
                                    }
                                }
                            }
                            action::Availability::Unavailable { reason } => {
                                log_info!("[radial] can't '{}': {}", name, reason);
                            }
                        }
                    }
                    radial_open = false;
                    radial_consumed = true;
                }
                continue;
            }

            // Death gate: highest-priority mode once the player has
            // expired. Only A (new run) and Start (quit) do anything;
            // everything else is silently dropped so the player can't
            // wander off the death screen by accident.
            if dead.is_some() {
                match input_action {
                    Action::A => {
                        let _ = std::fs::remove_file(save_dir.join(RUN_FILE));
                        world = World::new(WORLD_W, WORLD_H);
                        world.spawn_starter_bandit();
                        prev_run_header = None;
                        last_dawn_idx = dawns_elapsed(world.clock_seconds);
                        command_menu = None;
                        info_menu = None;
                        dead = None;
                        log_info!("[death] new run started");
                    }
                    Action::Start => break 'main,
                    _ => {}
                }
                continue;
            }

            // Pause menu is the highest-priority input mode below the
            // death gate. While open, every other state (active_action,
            // command_menu, world) is frozen. Multi-turn actions also
            // stop ticking — see the "if pause_menu.is_none()" guard
            // further down.
            if let Some(selected) = pause_menu {
                let count = PAUSE_OPTIONS.len();
                match input_action {
                    Action::Up => {
                        pause_menu = Some(selected.saturating_sub(1));
                    }
                    Action::Down => {
                        pause_menu = Some((selected + 1).min(count - 1));
                    }
                    Action::A => {
                        let (chosen, _) = PAUSE_OPTIONS[selected];
                        match chosen {
                            PauseAction::Save => {
                                save_game(
                                    &save_dir,
                                    &mut meta,
                                    &world,
                                    &mut prev_meta_header,
                                    &mut prev_run_header,
                                );
                                pause_menu = None;
                            }
                            PauseAction::Quit => break 'main,
                            PauseAction::ResetSave => {
                                let _ = std::fs::remove_file(save_dir.join(META_FILE));
                                let _ = std::fs::remove_file(save_dir.join(RUN_FILE));
                                world = World::new(WORLD_W, WORLD_H);
                                world.spawn_starter_bandit();
                                meta = MetaSave::empty(SaveHeader::fresh(None));
                                prev_meta_header = meta.header.clone();
                                prev_run_header = None;
                                last_dawn_idx = dawns_elapsed(world.clock_seconds);
                                command_menu = None;
                                pause_menu = None;
                                log_info!("[menu] save deleted; in-memory state reset");
                            }
                            PauseAction::GlyphPalette => {
                                pause_menu = None;
                                glyph_palette = Some(0);
                            }
                        }
                    }
                    Action::B | Action::Start => {
                        pause_menu = None;
                    }
                    _ => {}
                }
                continue;
            }

            // Ranged-targeting cursor. Modal: dpad moves the cursor,
            // A commits the shot via World::perform_ranged_attack,
            // B cancels. Closes on commit OR cancel.
            if let Some(ref mut tc) = target_cursor {
                match input_action {
                    Action::Up => tc.move_by(0, -1),
                    Action::Down => tc.move_by(0, 1),
                    Action::Left => tc.move_by(-1, 0),
                    Action::Right => tc.move_by(1, 0),
                    Action::A => {
                        if let Some(target) = world.hostile_at(tc.pos.x, tc.pos.y) {
                            world.perform_ranged_attack(world.player, target);
                            // Hostile turn after the shot, matching the
                            // melee bump flow.
                            world.tick_hostiles();
                        } else {
                            world.push_message("No target there.".to_string());
                        }
                        target_cursor = None;
                    }
                    Action::B => {
                        target_cursor = None;
                    }
                    _ => {}
                }
                continue;
            }

            // Overmap mode (full-screen Cornwall map). Modal: while
            // open, the player can move a cursor + initiate fast-travel
            // but cannot walk, interact, or open other menus. Toggle
            // with `M` (Action::OpenOvermap).
            if let Some(ref mut mode) = overmap_mode {
                match input_action {
                    Action::Up => mode.move_cursor(0, -1),
                    Action::Down => mode.move_cursor(0, 1),
                    Action::Left => mode.move_cursor(-1, 0),
                    Action::Right => mode.move_cursor(1, 0),
                    Action::A => {
                        // Initiate fast-travel to cursor. Plan path
                        // from the player's current cell to the cursor
                        // chunk; if it succeeds, store the queue on
                        // World and close the map.
                        let from = world.player_chunk();
                        let p = world.player_pos();
                        let cell_from = (p.x as i64, p.y as i64);
                        match fasttravel::plan_path(from, cell_from, mode.cursor) {
                            Some(queue) => {
                                log_info!(
                                    "[fast-travel] {} chunk hops planned to ({}, {})",
                                    queue.chunk_path.len(),
                                    mode.cursor.cx,
                                    mode.cursor.cy
                                );
                                last_overmap_destination = Some(mode.cursor);
                                world.fast_travel = Some(queue);
                                overmap_mode = None;
                            }
                            None => {
                                log_info!(
                                    "[fast-travel] no route to ({}, {})",
                                    mode.cursor.cx,
                                    mode.cursor.cy
                                );
                            }
                        }
                    }
                    Action::B | Action::OpenOvermap => {
                        overmap_mode = None;
                    }
                    _ => {}
                }
                continue;
            }

            // Open the overmap from any other "neutral" state (no
            // menus open, no active action). Toggling out lives in
            // the overmap-input arm above.
            if input_action == Action::OpenOvermap
                && command_menu.is_none()
                && info_menu.is_none()
                && glyph_palette.is_none()
                && world.active_action.is_none()
            {
                overmap_mode = Some(OvermapMode::open(&world, last_overmap_destination));
                continue;
            }

            // Dev glyph-palette overlay (X). Highest non-pause priority
            // so it overlays whatever else is open.
            if let Some(cursor) = glyph_palette {
                match input_action {
                    Action::Up => glyph_palette = Some(cursor.wrapping_sub(16)),
                    Action::Down => glyph_palette = Some(cursor.wrapping_add(16)),
                    Action::Left => glyph_palette = Some(cursor.wrapping_sub(1)),
                    Action::Right => glyph_palette = Some(cursor.wrapping_add(1)),
                    Action::B => glyph_palette = None,
                    Action::Start => pause_menu = Some(0),
                    _ => {}
                }
                continue;
            }

            // Info hub (Select). Tabbed, read-only for slice 1.
            // L/R cycle tabs, dpad navigates within the current tab,
            // B/Select closes. Sits between pause menu and command
            // menu in priority.
            if let Some(ref mut state) = info_menu {
                let row_count = info_tab_row_count(&world, state.tab);
                match input_action {
                    Action::L => {
                        state.tab = state.tab.prev();
                        state.selected = 0;
                    }
                    Action::R => {
                        state.tab = state.tab.next();
                        state.selected = 0;
                    }
                    Action::Up => {
                        if row_count > 0 {
                            state.selected = state.selected.saturating_sub(1);
                        }
                    }
                    Action::Down => {
                        if row_count > 0 {
                            state.selected = (state.selected + 1).min(row_count - 1);
                        }
                    }
                    Action::A => {
                        match state.tab {
                            InfoTab::Crafting => {
                                if let Some(recipe) = crafting::RECIPES.get(state.selected) {
                                    match action::evaluate(&world, recipe.action) {
                                        action::Availability::Available { .. } => {
                                            match action::execute(&mut world, recipe.action) {
                                                action::ExecuteOutcome::Done(msg) => {
                                                    log_info!("[craft] {}", msg);
                                                    info_menu = None;
                                                }
                                                action::ExecuteOutcome::OpenAim => {
                                                    // Crafting recipes never open the aim
                                                    // cursor, but match exhaustively to keep
                                                    // the variant disciplined.
                                                    info_menu = None;
                                                }
                                            }
                                        }
                                        action::Availability::Unavailable { reason } => {
                                            log_info!(
                                                "[craft] can't '{}': {}",
                                                recipe.name,
                                                reason
                                            );
                                        }
                                    }
                                }
                            }
                            InfoTab::Inventory => {
                                // Equip / unequip toggles per the row
                                // category. The world helpers handle
                                // pack ↔ slot bouncing and the derived
                                // Wielded / Worn / OffHand sync.
                                match inventory_row_at(&world, state.selected) {
                                    Some(InventoryRow::EquipSlot(slot)) => {
                                        let msg = world.unequip_to_pack(slot);
                                        world.push_message(msg);
                                    }
                                    Some(InventoryRow::PackItem(kind)) => {
                                        let msg = world.equip_from_pack(kind);
                                        world.push_message(msg);
                                    }
                                    None => {}
                                }
                            }
                            InfoTab::Skills => {}
                        }
                    }
                    Action::B | Action::Select => {
                        info_menu = None;
                    }
                    Action::Start => {
                        pause_menu = Some(0);
                    }
                    _ => {}
                }
                continue;
            }

            // Multi-turn-action mode: world is auto-ticking the queued
            // verb. The only inputs that mean anything are B (cancel),
            // Select (toggle view mode), and Start (open pause menu).
            // Everything else is dropped so the player can't move/menu
            // mid-pitch.
            if world.active_action.is_some() {
                match input_action {
                    Action::B => {
                        world.cancel_multi_turn();
                        log_info!("[action] cancelled");
                    }
                    Action::Select => {
                        world.toggle_multi_turn_view();
                        let mode = world
                            .active_action
                            .as_ref()
                            .map(|a| a.view_mode)
                            .unwrap_or(ViewMode::ProgressBar);
                        log_info!("[action] view mode = {:?}", mode);
                    }
                    Action::Start => {
                        pause_menu = Some(0);
                    }
                    _ => {}
                }
                continue;
            }

            // Menu mode: dpad navigates, A confirms (if available), B/Y
            // closes. Everything else is dropped so the world doesn't tick
            // while the player is browsing the catalog.
            if let Some(selected) = command_menu {
                match input_action {
                    Action::Up => {
                        command_menu = Some(selected.saturating_sub(1));
                    }
                    Action::Down => {
                        let max = action::ALL_ACTIONS.len().saturating_sub(1);
                        command_menu = Some((selected + 1).min(max));
                    }
                    Action::A => {
                        let id = action::ALL_ACTIONS[selected].id;
                        match action::evaluate(&world, id) {
                            action::Availability::Available { .. } => {
                                match action::execute(&mut world, id) {
                                    action::ExecuteOutcome::Done(msg) => {
                                        log_info!("[menu] {}", msg);
                                        command_menu = None;
                                    }
                                    action::ExecuteOutcome::OpenAim => {
                                        target_cursor = Some(TargetCursor::open(&world));
                                        command_menu = None;
                                    }
                                }
                            }
                            action::Availability::Unavailable { reason } => {
                                // Stay open so the player can pick another.
                                log_info!(
                                    "[menu] can't '{}': {}",
                                    action::ALL_ACTIONS[selected].name,
                                    reason
                                );
                            }
                        }
                    }
                    Action::B => {
                        command_menu = None;
                    }
                    Action::Start => {
                        pause_menu = Some(0);
                    }
                    _ => {}
                }
                continue;
            }

            match input_action {
                Action::Up => world.try_move_player(0, -1),
                Action::Down => world.try_move_player(0, 1),
                Action::Left => world.try_move_player(-1, 0),
                Action::Right => world.try_move_player(1, 0),
                Action::A => {
                    // Route through the same dispatcher the command
                    // menu uses so Pickup's cost + side-effects stay
                    // in one place (action.rs). Pickup never returns
                    // OpenAim, but match exhaustively for discipline.
                    match action::execute(&mut world, action::ActionId::Pickup) {
                        action::ExecuteOutcome::Done(msg) => log_debug!("{}", msg),
                        action::ExecuteOutcome::OpenAim => {}
                    }
                }
                Action::Start => {
                    pause_menu = Some(0);
                }
                Action::Select => {
                    info_menu = Some(InfoMenuState {
                        tab: InfoTab::Inventory,
                        selected: 0,
                    });
                }
                _ => {}
            }
        }

        // Fast-travel tick. Cancels on any input this frame (the watcher
        // computed at the top of the loop), else advances one cell. The
        // overmap mode being open suspends ticking — the player is
        // staring at the map, not walking.
        if pause_menu.is_none() && dead.is_none() && overmap_mode.is_none() {
            if cancels_fast_travel && world.fast_travel.is_some() {
                log_info!("[fast-travel] you stop.");
                world.fast_travel = None;
            } else if world.fast_travel.is_some() {
                match world.tick_fast_travel() {
                    FastTravelStep::Stepped => {}
                    FastTravelStep::Completed => {
                        log_info!("[fast-travel] arrived.");
                        world.fast_travel = None;
                        last_overmap_destination = None;
                    }
                    FastTravelStep::BlockedAtCell => {
                        log_info!("[fast-travel] the path is blocked.");
                        world.fast_travel = None;
                    }
                    FastTravelStep::NeedCritical => {
                        log_info!("[fast-travel] you're too exhausted to keep going.");
                        world.fast_travel = None;
                    }
                }
            }
        }

        // Multi-turn action tick. Runs per-frame; advance rate depends
        // on view_mode. Completed steps trigger action::complete_step
        // (which fires the verb's consume-from-pack and structure-place
        // effects). Interrupts (need < critical threshold) cancel the
        // entire queue. Pause menu blocks ticking so the world stops
        // when the player opens the menu mid-pitch.
        let multi_view = world.active_action.as_ref().map(|a| a.view_mode);
        if pause_menu.is_none() && dead.is_none() {
            if let Some(view_mode) = multi_view {
                let advance = match view_mode {
                    ViewMode::ProgressBar => MULTI_TURN_GAME_SEC_PER_FRAME,
                    ViewMode::TimeSkip => world
                        .active_action
                        .as_ref()
                        .map(|a| a.total_remaining_secs())
                        .unwrap_or(0),
                };
                let result = world.tick_multi_turn(advance);
                for step_id in result.completed_steps {
                    if let Some(msg) = action::complete_step(&mut world, step_id) {
                        log_info!("[action] {}", msg);
                    }
                }
                if result.interrupted {
                    log_info!("[action] interrupted (need critical)");
                }
            }
        }

        #[cfg(not(target_arch = "arm"))]
        {
            let mut new_world_request: Option<Option<u64>> = None;
            debug.drain(|cmd| {
                if let Some(eff) = debug_console::apply_debug_command(&mut world, cmd) {
                    match eff {
                        debug_console::DebugSideEffect::NewWorld(s) => {
                            new_world_request = Some(s);
                        }
                    }
                }
            });
            if let Some(seed_opt) = new_world_request {
                let new_seed = seed_opt.unwrap_or_else(fresh_world_seed);
                log_info!("[newworld] rebuilding with seed 0x{:016X}", new_seed);
                let _ = std::fs::remove_file(save_dir.join(RUN_FILE));
                world = World::with_seed(WORLD_W, WORLD_H, new_seed);
                world.ensure_player_ring();
                world.spawn_starter_bandit();
                world.recompute_fov();
                prev_run_header = None;
                command_menu = None;
                info_menu = None;
                dead = None;
                last_dawn_idx = dawns_elapsed(world.clock_seconds);
            }
        }

        // Death detection. Runs after action+tick so an action that
        // pushed a need to 0 surfaces this frame. is_dead() is the
        // gate (honors DEATH_ENABLED); from_needs picks which need
        // killed us for the epitaph. Cancel any active multi-turn
        // queue so the death overlay isn't competing with a ticking
        // pitch-tent banner.
        if dead.is_none() {
            let needs_now = world.player_needs();
            let combat_kill = world.player_killed_by_combat;
            let needs_cause = if needs_now.is_dead() {
                DeathCause::from_needs(&needs_now)
            } else {
                None
            };
            // Combat kill takes priority over needs decay so the right
            // epitaph shows when a bandit finishes a thirsty player.
            let cause = if combat_kill {
                Some(DeathCause::Combat)
            } else {
                needs_cause
            };
            if let Some(cause) = cause {
                if world.active_action.is_some() {
                    world.cancel_multi_turn();
                }
                command_menu = None;
                info_menu = None;
                log_info!("[death] {:?}", cause);
                dead = Some(cause);
            }
        }

        // Auto-save on dawn crossing. Detects forward crossings via
        // `dawns_elapsed` increments. Debug commands can rewind time, in
        // which case we silently resync without firing save (and the
        // subsequent forward crossing fires normally).
        let now_dawn_idx = dawns_elapsed(world.clock_seconds);
        if now_dawn_idx > last_dawn_idx {
            log_info!(
                "auto-save: crossed dawn (day {} -> {})",
                last_dawn_idx,
                now_dawn_idx
            );
            // Daily skill-XP caps reset at each dawn so the player can
            // train each skill again. Reset before saving so the save
            // captures the post-reset state.
            let mut skills = world.player_skills();
            skills.reset_daily_caps();
            world.set_player_skills(skills);
            // Phase D lifecycle ticks. Saplings mature; future cards
            // add FallenLeaves spawn, mast drops, mushroom expiry.
            world.promote_saplings_on_dawn();
            // FOV may need a refresh if a sapling just became a tree
            // (the new TreeTrunk blocks sight). Cheap on the dirty
            // path.
            world.recompute_fov();
            save_game(
                &save_dir,
                &mut meta,
                &world,
                &mut prev_meta_header,
                &mut prev_run_header,
            );
        }
        last_dawn_idx = now_dawn_idx;

        // Camera in world coords — player-centered follow-cam. The viewport
        // top-left in world coords is (player - half_viewport); the player
        // always appears at the center cell of the viewport.
        let player = world.player_pos();
        let pwx = player.x as i64;
        let pwy = player.y as i64;
        let cam_x: i64 = pwx - WORLD_W as i64 / 2;
        let cam_y: i64 = pwy - WORLD_H as i64 / 2;

        // Detect camera shift for the scroll-blit path below.
        let dx_cells = (cam_x - cam_x_prev) as i32;
        let dy_cells = (cam_y - cam_y_prev) as i32;
        let scrolled = dx_cells != 0 || dy_cells != 0;
        if scrolled {
            shift_framebuffer(&mut framebuf, dx_cells, dy_cells);
            shift_prev_cells(&mut prev_cells, WORLD_W, WORLD_H, dx_cells, dy_cells);
        }
        cam_x_prev = cam_x;
        cam_y_prev = cam_y;
        let needs = world.player_needs();
        let (clock_h, clock_m) = world.clock_hm();
        let day = world.day_count();
        let is_night = world.is_night();
        let tint = brightness_at(world.clock_seconds);
        let player_skills = world.player_skills();
        let calendar_day = world.calendar_day;
        let player_body = world.player_body();
        let player_stamina = world.player_stamina();
        let mut ui_cells = build_ui_cells(
            &palette,
            needs,
            day,
            clock_h,
            clock_m,
            is_night,
            player_skills,
            calendar_day,
            player_body,
            player_stamina,
        );
        if target_cursor.is_none() {
            draw_message_line(&mut ui_cells, &world, &palette);
        }
        draw_here_line(&mut ui_cells, &world, &palette);
        if let Some(ref tc) = target_cursor {
            draw_target_cursor(&mut ui_cells, &world, tc, cam_x, cam_y, &palette);
        }
        if let Some(active) = world.active_action.as_ref() {
            draw_multi_turn_banner(&mut ui_cells, active, &palette);
        }
        if let Some(selected) = command_menu {
            draw_command_menu(&mut ui_cells, &world, selected, &palette);
        }
        if let Some(state) = info_menu.as_ref() {
            draw_info_menu(&mut ui_cells, &world, state, &palette);
        }
        if let Some(cursor) = glyph_palette {
            draw_glyph_palette(&mut ui_cells, cursor, &palette);
        }
        if radial_open {
            draw_radial_menu(&mut ui_cells, &world, &palette);
        }
        if let Some(ref ft) = world.fast_travel {
            draw_fast_travel_banner(&mut ui_cells, ft, &palette);
        }
        if let Some(ref mode) = overmap_mode {
            draw_overmap(&mut ui_cells, &world, mode, &palette, overmap_frame_count);
        }
        if let Some(selected) = pause_menu {
            draw_pause_menu(&mut ui_cells, selected, &palette);
        }
        if let Some(cause) = dead {
            draw_death_screen(&mut ui_cells, cause, &palette);
        }
        overmap_frame_count = overmap_frame_count.wrapping_add(1);

        let draw_start = Instant::now();
        let mut changed_cells = 0;
        let mut dirty_min_x = WORLD_W as i32;
        let mut dirty_min_y = WORLD_H as i32;
        let mut dirty_max_x = 0;
        let mut dirty_max_y = 0;
        let season = world.season();
        for vy in 0..WORLD_H as i32 {
            for vx in 0..WORLD_W as i32 {
                let wx = cam_x + vx as i64;
                let wy = cam_y + vy as i64;
                // Base terrain glyph via TerrainDef (one source of truth
                // for glyph + fg + bg per kind; see world.rs).
                let terrain = world.tile_at(wx, wy);
                let terrain_def = terrain.def();
                let mut glyph = terrain_def.glyph;
                // Per-cell color gradient for walkable terrain: small
                // hash-driven RGB offset on fg+bg so the floor reads as
                // organic texture rather than a flat region. Unwalkable
                // terrain (trees, water) stays flat-saturated so it
                // pierces the floor as a visual landmark.
                let apply_gradient =
                    matches!(terrain, TerrainKind::Grass | TerrainKind::BareDirt | TerrainKind::SandShore);
                let base_fg = terrain_def.fg(season);
                let base_bg = terrain_def.bg(season);
                let (fg_arr, bg_arr) = if apply_gradient {
                    floor_with_gradient(base_fg, base_bg, wx as i32, wy as i32, world.seed)
                } else {
                    (base_fg, base_bg)
                };
                let mut fg = Color::RGB(fg_arr[0], fg_arr[1], fg_arr[2]);
                // Ground-cover lerps: stored variants paint over the
                // seasonal terrain bg. Snow is render-time-only based on
                // (Winter && terrain.is_outdoor()) and lerps on top of
                // any stored ground_cover, so a winter LeafLitter cell
                // still whitens while the brown underneath survives
                // until spring.
                let stored_cover = world
                    .cell_at(wx, wy)
                    .map(|c| c.ground_cover)
                    .unwrap_or(GroundCover::None);
                let bg_arr = match stored_cover {
                    GroundCover::None => bg_arr,
                    GroundCover::LeafLitter => lerp_rgb(bg_arr, [60, 45, 25], 0.25),
                };
                // Autumn FallenLeaves: render-time-only effect on
                // outdoor grass cells adjacent to a deciduous tree.
                // Lerps on TOP of any stored cover (LeafLitter under
                // FallenLeaves looks like a deeper organic mat).
                let bg_arr = if matches!(season, calendar::Season::Autumn)
                    && terrain.is_outdoor()
                    && has_deciduous_neighbor(&world, wx, wy)
                {
                    lerp_rgb(bg_arr, [140, 70, 30], 0.35)
                } else {
                    bg_arr
                };
                let bg_arr = if matches!(season, calendar::Season::Winter) && terrain.is_outdoor() {
                    lerp_rgb(bg_arr, [230, 235, 245], 0.60)
                } else {
                    bg_arr
                };
                let bg = Color::RGB(bg_arr[0], bg_arr[1], bg_arr[2]);
                // Sparse grass tufts: hash-driven so ~25% of grass
                // cells show the 0x9C tuft sprite; the rest render as
                // blank background. Deterministic across reloads via
                // world.seed.
                if terrain == TerrainKind::Grass
                    && !grass_dot_visible(wx as i32, wy as i32, world.seed)
                {
                    glyph = b' ';
                }
                // Tree rendering: glyph + tint by species + season. The
                // per-cell `cell.tree_species` (set by chunkgen) picks
                // the species glyph; the per-species canopy_fg table
                // picks the seasonal tint. Cells with no species (only
                // hit if a save predates Phase C) fall back to the
                // per-cell hash + species-agnostic TREE_VARIANT_GLYPHS
                // catalog for visual variety.
                if terrain == TerrainKind::TreeTrunk {
                    let species = world.cell_at(wx, wy).and_then(|c| c.tree_species);
                    match species {
                        Some(sp) => {
                            glyph = sp.canopy_glyph();
                            let t = sp.canopy_fg(season);
                            fg = Color::RGB(t[0], t[1], t[2]);
                        }
                        None => {
                            let gi = tree_variant_index(wx as i32, wy as i32, world.seed);
                            glyph = TREE_VARIANT_GLYPHS[gi % TREE_VARIANT_GLYPHS.len()];
                        }
                    }
                }
                let cell_state = world.cell_at(wx, wy);
                let visible = cell_state.map(|c| c.visible).unwrap_or(false);
                let explored = cell_state.map(|c| c.explored).unwrap_or(false);
                let light_intensity = cell_state.map(|c| c.light_intensity).unwrap_or(0);

                // Items + player only render when the cell is currently
                // visible. Memory of explored-but-unseen cells shows
                // terrain only. Decoration overlay sits BETWEEN
                // terrain and items: priority is `item > decoration >
                // terrain`. A fern with a stone dropped on it still
                // reads as a stone.
                if visible {
                    let decoration = cell_state
                        .map(|c| c.decoration)
                        .unwrap_or(flora::Decoration::None);
                    if !matches!(decoration, flora::Decoration::None) {
                        glyph = decoration.glyph();
                        let [r, gn, b] = decoration.fg(season);
                        fg = Color::RGB(r, gn, b);
                    }
                    if let Some(top) = cell_state.and_then(|c| c.items.last()) {
                        // Lit fires override the kind's default glyph so
                        // a lit-firewood reads as fire (orange '*') rather
                        // than a wood pile ('=' brown).
                        let (g, [r, gn, b]) = match top.metadata {
                            items::ItemMetadata::Lit { .. } => (b'*', [230, 140, 60]),
                            _ => top.kind.glyph_color(),
                        };
                        glyph = g;
                        // Blendable items mix their fg ~45% toward the
                        // (per-cell-gradiented) terrain fg, so twigs /
                        // grass / moss / mud read as part of the floor
                        // texture. Stones / firewood / herbs / tools
                        // stay full-saturation and pierce the floor.
                        let is_blendable =
                            !matches!(top.metadata, items::ItemMetadata::Lit { .. })
                                && top.kind.def().blends_with_terrain;
                        fg = if is_blendable {
                            blend_to_terrain([r, gn, b], fg_arr, 0.45)
                        } else {
                            Color::RGB(r, gn, b)
                        };
                    }
                    // Non-player entities (bandits etc.) render above
                    // ground items but below the player @ — so a bandit
                    // standing on a dropped spear shows the bandit, but
                    // if the player and a bandit ever overlap (death
                    // tile) the @ wins.
                    if let Some((g, ec)) = world::entity_glyph_at(&world, wx as i32, wy as i32) {
                        glyph = g;
                        fg = Color::RGB(ec[0], ec[1], ec[2]);
                    }
                    if wx == pwx && wy == pwy {
                        glyph = b'@';
                        fg = palette.player_fg;
                    }
                }

                // Three visibility levels modulate brightness:
                //   visible:   full color + day/night tint
                //   explored:  fixed dim (25%) regardless of clock
                //   neither:   black
                let cell_brightness = if visible {
                    tint
                } else if explored {
                    0.25
                } else {
                    0.0
                };
                // Light-source contribution at night: cells inside a
                // lit disc get BOTH a brightness boost (so a fire-lit
                // cell isn't dim) AND a warm yellow blend, each scaled
                // by per-cell intensity. Intensity falls off linearly
                // with distance to the source, so the disc gradients
                // from bright-warm at the source to dark-cold at the
                // edge instead of being a uniform patch.
                //
                // The boost adds to brightness BEFORE the scalar dim
                // multiply (otherwise night's 0.4 floor would crush
                // the warm color back to grey).
                const LIGHT_TINT: [u8; 3] = [255, 200, 100];
                const LIGHT_BRIGHTNESS_BOOST_MAX: f32 = 0.6;
                const LIGHT_TINT_MIX_MAX: f32 = 0.5;
                let light = if is_night && visible {
                    light_intensity as f32 / 255.0
                } else {
                    0.0
                };
                let effective_brightness =
                    (cell_brightness + light * LIGHT_BRIGHTNESS_BOOST_MAX).min(1.0);
                let mut fg = tint_color(fg, effective_brightness);
                let mut bg = tint_color(bg, effective_brightness);
                if light > 0.0 {
                    let mix = light * LIGHT_TINT_MIX_MAX;
                    fg = blend_to_terrain(LIGHT_TINT, [fg.r, fg.g, fg.b], mix);
                    bg = blend_to_terrain(LIGHT_TINT, [bg.r, bg.g, bg.b], mix);
                }
                let mut cell = Cell { glyph, fg, bg };
                let i = (vy as u32 * WORLD_W + vx as u32) as usize;
                if let Some(ui_cell) = ui_cells[i] {
                    cell = ui_cell;
                }
                if prev_cells[i] != Some(cell) {
                    draw_glyph(&mut framebuf, &mut atlas, vx, vy, cell.glyph, cell.fg, cell.bg);
                    prev_cells[i] = Some(cell);
                    changed_cells += 1;
                    dirty_min_x = dirty_min_x.min(vx);
                    dirty_min_y = dirty_min_y.min(vy);
                    dirty_max_x = dirty_max_x.max(vx);
                    dirty_max_y = dirty_max_y.max(vy);
                }
            }
        }
        timing_accum.draw += draw_start.elapsed();
        timing_accum.changed_cells += changed_cells;

        let upload_start = Instant::now();
        if scrolled {
            // Scroll frame: the CPU surface contents shifted in-place via
            // shift_framebuffer, but the streaming texture's contents
            // haven't moved. Re-upload the whole framebuffer so the
            // texture matches. Edge-strip blits and any in-place cell
            // changes are already baked into `framebuf` by the compose
            // loop above.
            let pitch = framebuf.pitch() as usize;
            let pixels = framebuf.without_lock().expect("CPU surface");
            present_tex.update(None, pixels, pitch)?;
        } else if changed_cells > 0 {
            let dirty = Rect::new(
                dirty_min_x * CELL_SIZE as i32,
                dirty_min_y * CELL_SIZE as i32,
                (dirty_max_x - dirty_min_x + 1) as u32 * CELL_SIZE,
                (dirty_max_y - dirty_min_y + 1) as u32 * CELL_SIZE,
            );
            let pitch = framebuf.pitch() as usize;
            let offset = dirty.y() as usize * pitch + dirty.x() as usize * 4;
            let pixels = framebuf.without_lock().expect("CPU surface");
            present_tex.update(Some(dirty), &pixels[offset..], pitch)?;
        }
        timing_accum.upload += upload_start.elapsed();

        let copy_start = Instant::now();
        canvas.set_draw_color(palette.letterbox);
        canvas.clear();
        canvas.copy(&present_tex, None, None)?;
        timing_accum.copy += copy_start.elapsed();

        let present_start = Instant::now();
        canvas.present();
        timing_accum.present += present_start.elapsed();

        let elapsed = frame_start.elapsed();
        timing_accum.sleep += pace_frame(frame_start, elapsed);
        timing_accum.frames += 1;

        fps_count += 1;
        if fps_window.elapsed() >= TIMING_LOG_INTERVAL {
            log_verbose!("fps: {}", fps_count);
            log_verbose!("{}", timing_accum.summary());
            fps_count = 0;
            fps_window = Instant::now();
            timing_accum = FrameTiming::default();
        }
    }

    log_info!("shutdown: exiting Survival main loop");
    let _ = std::io::stderr().flush();
    Ok(())
}

/// Wall-clock-derived world seed used when no run save exists. SplitMix64
/// mixer over `SystemTime` nanos so adjacent boots produce well-spread
/// seeds (the raw nanos field changes slowly in the high bits).
fn fresh_world_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(save::DEFAULT_WORLD_SEED);
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Convert in-memory body-part HP into the save struct.
fn world_bp_to_save(bp: &world::BodyParts) -> save::BodyPartsSave {
    let c = |p: world::BodyPartHp| save::BodyPartSaveCell { hp: p.hp, max: p.max };
    save::BodyPartsSave {
        head: c(bp.head),
        torso: c(bp.torso),
        l_arm: c(bp.l_arm),
        r_arm: c(bp.r_arm),
        l_leg: c(bp.l_leg),
        r_leg: c(bp.r_leg),
    }
}

/// Convert in-memory `Equipment` into save bytes. Empty slots become
/// the empty string (forward-compat default).
fn world_equipment_to_save(eq: &world::Equipment) -> save::EquipmentSave {
    let s = |k: Option<items::ItemKind>| k.map(|x| x.save_key().to_string()).unwrap_or_default();
    save::EquipmentSave {
        main_hand: s(eq.main_hand),
        off_hand: s(eq.off_hand),
        head: s(eq.head),
        torso: s(eq.torso),
        l_arm: s(eq.l_arm),
        r_arm: s(eq.r_arm),
        l_leg: s(eq.l_leg),
        r_leg: s(eq.r_leg),
    }
}

fn save_equipment_to_world(eq: &save::EquipmentSave) -> world::Equipment {
    let p = |s: &str| items::ItemKind::from_save_key(s);
    world::Equipment {
        main_hand: p(&eq.main_hand),
        off_hand: p(&eq.off_hand),
        head: p(&eq.head),
        torso: p(&eq.torso),
        l_arm: p(&eq.l_arm),
        r_arm: p(&eq.r_arm),
        l_leg: p(&eq.l_leg),
        r_leg: p(&eq.r_leg),
    }
}

/// Reverse of `world_bp_to_save`. Defaults a zero-max cell to the
/// canonical starting maxes so a partial save (e.g. only torso written)
/// still loads into a sane body.
fn save_bp_to_world(bp: &save::BodyPartsSave) -> world::BodyParts {
    let starting = world::BodyParts::starting_human();
    let cell = |saved: save::BodyPartSaveCell, fallback: world::BodyPartHp| {
        if saved.max <= 0 {
            fallback
        } else {
            world::BodyPartHp { hp: saved.hp, max: saved.max }
        }
    };
    world::BodyParts {
        head: cell(bp.head, starting.head),
        torso: cell(bp.torso, starting.torso),
        l_arm: cell(bp.l_arm, starting.l_arm),
        r_arm: cell(bp.r_arm, starting.r_arm),
        l_leg: cell(bp.l_leg, starting.l_leg),
        r_leg: cell(bp.r_leg, starting.r_leg),
    }
}

fn save_game(
    save_dir: &std::path::Path,
    meta: &mut MetaSave,
    world: &World,
    prev_meta_header: &mut SaveHeader,
    prev_run_header: &mut Option<SaveHeader>,
) {
    let new_meta_header = SaveHeader::fresh(Some(prev_meta_header));
    let mut next_meta = meta.clone();
    next_meta.header = new_meta_header.clone();
    if let Err(e) = save::save_atomic(&save_dir.join(META_FILE), &next_meta) {
        log_info!("meta save failed: {}", e);
    } else {
        *meta = next_meta;
        *prev_meta_header = new_meta_header;
        log_debug!("meta saved (counter={})", prev_meta_header.save_counter);
    }

    let pos = world.player_pos();
    let new_run_header = SaveHeader::fresh(prev_run_header.as_ref());
    let mut run = RunSave::empty(new_run_header.clone());
    run.player_x = pos.x;
    run.player_y = pos.y;
    run.pack = world.player_pack().to_save();
    run.cell_items = world
        .snapshot_cell_items()
        .into_iter()
        .map(|(x, y, items)| CellItemsSave {
            x,
            y,
            items: items.iter().map(|i| i.to_save()).collect(),
        })
        .collect();
    run.clock_seconds = world.clock_seconds;
    run.calendar_day = world.calendar_day;
    let n = world.player_needs();
    run.needs = NeedsSave {
        thirst: n.thirst,
        hunger: n.hunger,
        sleep: n.sleep,
        warmth: n.warmth,
        thirst_acc_secs: n.thirst_acc_secs,
        hunger_acc_secs: n.hunger_acc_secs,
        sleep_acc_secs: n.sleep_acc_secs,
        warmth_acc_secs: n.warmth_acc_secs,
    };
    run.explored_cells = world.snapshot_explored();
    let player_skills = world.player_skills();
    let to_save = |s: Skill| SkillSave { value: s.value, daily_xp: s.daily_xp };
    let prof = &player_skills.proficiencies;
    run.skills = SkillsSave {
        fire_making: to_save(player_skills.fire_making),
        foraging: to_save(player_skills.foraging),
        melee: to_save(player_skills.melee),
        ranged: to_save(player_skills.ranged),
        dodge: to_save(player_skills.dodge),
        block: to_save(player_skills.block),
        light_armor: to_save(player_skills.light_armor),
        medium_armor: to_save(player_skills.medium_armor),
        heavy_armor: to_save(player_skills.heavy_armor),
        proficiencies: save::ProficienciesSave {
            knife: to_save(prof.knife),
            sword: to_save(prof.sword),
            falchion: to_save(prof.falchion),
            axe: to_save(prof.axe),
            mace_cudgel: to_save(prof.mace_cudgel),
            quarterstaff: to_save(prof.quarterstaff),
            spear_lance: to_save(prof.spear_lance),
            gisarme_bill: to_save(prof.gisarme_bill),
            unarmed: to_save(prof.unarmed),
            bow: to_save(prof.bow),
            crossbow: to_save(prof.crossbow),
        },
    };
    run.rng_state = world.rng.state;
    run.seed = world.seed;
    run.speed = world.player_speed();
    run.terrain_mutations = world
        .snapshot_terrain_mutations()
        .into_iter()
        .map(|(x, y, k)| TerrainMutationSave {
            x,
            y,
            kind: k.save_key().to_string(),
        })
        .collect();
    run.tree_species_mutations = world
        .snapshot_tree_species_mutations()
        .into_iter()
        .map(|(x, y, sp)| TreeSpeciesMutationSave {
            x,
            y,
            species_key: sp.map(|s| s.save_key().to_string()).unwrap_or_default(),
        })
        .collect();
    run.decoration_mutations = world
        .snapshot_decoration_mutations()
        .into_iter()
        .map(|(x, y, d)| DecorationMutationSave {
            x,
            y,
            decoration: d,
        })
        .collect();
    let player_body = world.player_body();
    run.player_body_parts = Some(world_bp_to_save(&player_body));
    let player_eq = world.player_equipment();
    run.player_equipment = Some(world_equipment_to_save(&player_eq));
    run.player_stamina = world.player_stamina_full().map(|s| StaminaSave {
        cur: s.cur,
        max: s.max,
        recent_combat_secs: s.recent_combat_secs,
    });
    // Leave the legacy single-pool field empty; phase 2 + later writes
    // route through body_parts. A v3 player_health field still loads
    // cleanly via serde but is never written.
    run.player_health = None;
    run.hostiles = world
        .snapshot_hostiles()
        .into_iter()
        .map(|snap| save::HostileSave {
            x: snap.pos.x,
            y: snap.pos.y,
            // hp + max_hp stay populated for any older binary that
            // wants to read the file — surface the torso pool as the
            // closest single-pool analog.
            hp: snap.body.torso.hp,
            max_hp: snap.body.torso.max,
            wielded_kind: snap.main_hand.map(|k| k.save_key().to_string()).unwrap_or_default(),
            flavor: snap.flavor.to_string(),
            body_parts: Some(world_bp_to_save(&snap.body)),
            off_hand_kind: snap.off_hand.map(|k| k.save_key().to_string()).unwrap_or_default(),
            worn_kinds: snap.worn_kinds.iter().map(|k| k.save_key().to_string()).collect(),
            stamina: snap.stamina.map(|s| StaminaSave {
                cur: s.cur,
                max: s.max,
                recent_combat_secs: s.recent_combat_secs,
            }),
        })
        .collect();
    run.active_action = world.active_action.as_ref().map(|active| ActiveActionSave {
        steps: active
            .steps
            .iter()
            .map(|s| ActionStepSave {
                id: s.id.save_key().to_string(),
                elapsed_secs: s.elapsed_secs,
                target_secs: s.target_secs,
            })
            .collect(),
        view_mode: match active.view_mode {
            ViewMode::ProgressBar => "progress_bar",
            ViewMode::TimeSkip => "time_skip",
        }
        .to_string(),
    });
    if let Err(e) = save::save_atomic(&save_dir.join(RUN_FILE), &run) {
        log_info!("run save failed: {}", e);
    } else {
        *prev_run_header = Some(new_run_header);
        log_debug!(
            "run saved at ({}, {}) — pack {}g, {} non-empty cells",
            run.player_x,
            run.player_y,
            run.pack
                .contents
                .iter()
                .map(|i| (i.weight_g_each as u64) * (i.count as u64))
                .sum::<u64>(),
            run.cell_items.len()
        );
    }
}

/// CP437 byte glyphs used in the HUD; can't be embedded in Rust string
/// literals because the source is UTF-8 and `put_text` writes raw bytes.
const HUD_GLYPH_THIRST: u8 = 0x14; // custom mug sprite
const HUD_GLYPH_HUNGER: u8 = 0xE0; // custom chicken-leg sprite (matches Ration)
const HUD_GLYPH_SLEEP: u8 = 0xE9; // custom bed sprite
const HUD_GLYPH_WARMTH: u8 = 0x0F; // ☼ sun / fire

fn build_ui_cells(
    palette: &Palette,
    needs: Needs,
    day: u64,
    clock_h: u8,
    clock_m: u8,
    is_night: bool,
    skills: Skills,
    calendar_day: u32,
    body: world::BodyParts,
    stamina: (i16, i16),
) -> Vec<Option<Cell>> {
    let mut cells = vec![None; (WORLD_W * WORLD_H) as usize];

    // Row 1 left: "Day N HH:MM day|night".
    let suffix = if is_night { "night" } else { "day" };
    let left = format!("Day {} {:02}:{:02} {}", day, clock_h, clock_m, suffix);
    put_text(&mut cells, 1, 1, &left, palette.hud_fg, palette.hud_bg);

    // Row 2 right: calendar date + season label, e.g. "21 Mar Spring".
    // Right-aligned so it sits opposite the Fire Making skill readout
    // on Row 2 left. Updated only when the calendar advances; the
    // dirty-cell diff path skips redraws on unchanged frames.
    let (_year, month, dom) = calendar::date_of(calendar_day);
    let season = calendar::season_of(calendar_day);
    let date_str = format!("{} {} {}", dom, month.short_label(), season.label());
    let dx = WORLD_W as i32 - date_str.len() as i32 - 1;
    put_text(&mut cells, dx, 2, &date_str, palette.hud_fg, palette.hud_bg);

    // Row 1 right: four CP437 need meters, each with its own symbol
    // fg so the atlas-colored sprites (mug, chicken leg, sun) tint
    // toward their natural hue. Critical state (any need < 25)
    // overrides both symbol and digit fgs to need_critical_fg so the
    // warning reads at a glance.
    let critical = needs
        .thirst
        .min(needs.hunger)
        .min(needs.sleep)
        .min(needs.warmth)
        < 25;
    let digit_fg = if critical {
        palette.need_critical_fg
    } else {
        palette.hud_fg
    };
    // Per-symbol tints (overridden by critical_fg when critical).
    let pick = |normal: Color| if critical { palette.need_critical_fg } else { normal };
    let meters: [(u8, Color, u8); 4] = [
        (HUD_GLYPH_THIRST, pick(Color::RGB(180, 140, 90)), needs.thirst),
        (HUD_GLYPH_HUNGER, pick(Color::RGB(220, 180, 110)), needs.hunger),
        // Near-white fg lets the atlas's intrinsic colors come
        // through if the bed sprite is pre-painted (e.g. brown frame
        // + red blanket). If the atlas bed is grayscale instead, we
        // need to implement luminance-banded tinting.
        (HUD_GLYPH_SLEEP, pick(Color::RGB(240, 240, 240)), needs.sleep),
        (HUD_GLYPH_WARMTH, pick(Color::RGB(240, 195, 80)), needs.warmth),
    ];
    // Pre-compute total width so we right-align.
    let total_w: usize = meters
        .iter()
        .map(|(_, _, v)| 1 + v.to_string().len())
        .sum::<usize>()
        + (meters.len() - 1); // single-space gap between meters
    let mut x = WORLD_W as i32 - total_w as i32 - 1;
    for (i, (glyph, sym_fg, value)) in meters.iter().enumerate() {
        put_cell(
            &mut cells,
            x,
            1,
            Cell {
                glyph: *glyph,
                fg: *sym_fg,
                bg: palette.hud_bg,
            },
        );
        x += 1;
        let s = value.to_string();
        put_text(&mut cells, x, 1, &s, digit_fg, palette.hud_bg);
        x += s.len() as i32;
        if i + 1 < meters.len() {
            x += 1; // space between meters
        }
    }

    // Row 2 left: vital-HP readout. With per-body-part HP, the single
    // number that answers "am I about to die?" is the worst of the two
    // vitals — head and torso. Limbs can cripple but won't kill. Show
    // head + torso explicitly so the player sees both, color the line
    // by the worst-percent of the two.
    let head_pct = body.head.hp.max(0) as i32 * 100 / body.head.max.max(1) as i32;
    let torso_pct = body.torso.hp.max(0) as i32 * 100 / body.torso.max.max(1) as i32;
    let worst = head_pct.min(torso_pct);
    let hp_fg = if worst <= 25 {
        palette.need_critical_fg
    } else if worst <= 50 {
        Color::RGB(230, 200, 90)
    } else {
        palette.hud_fg
    };
    let hp_line = format!("HP H{} T{}", body.head.hp.max(0), body.torso.hp.max(0));
    put_text(&mut cells, 1, 2, &hp_line, hp_fg, palette.hud_bg);

    // Stamina readout right after HP. Mirror of HP coloring: dims to
    // critical at low stamina. Only shown when stamina is meaningful
    // (max > 0).
    let (stam_cur, stam_max) = stamina;
    if stam_max > 0 {
        let stam_pct = stam_cur.max(0) as i32 * 100 / stam_max as i32;
        // Green → off-white → yellow → red as the pool depletes, per
        // the Stamina card. Zero gets the critical-need color so the
        // depleted state reads at a glance.
        let stam_fg = if stam_cur <= 0 {
            palette.need_critical_fg
        } else if stam_pct <= 15 {
            Color::RGB(220, 80, 80)
        } else if stam_pct <= 35 {
            Color::RGB(230, 200, 90)
        } else if stam_pct >= 85 {
            Color::RGB(120, 200, 110)
        } else {
            palette.hud_fg
        };
        let stam_line = format!(" SP {}", stam_cur.max(0));
        let after_hp_x = 1 + hp_line.len() as i32;
        put_text(&mut cells, after_hp_x, 2, &stam_line, stam_fg, palette.hud_bg);
    }

    // Crippled-status badge after stamina. Reads functionally rather
    // than anatomically — what the player can't do matters more than
    // which limb. Phase 2 rules: any arm crippled → can't wield (any
    // wielded weapon drops); any leg crippled → effective speed halved.
    let arm_out = body.l_arm.is_crippled() || body.r_arm.is_crippled();
    let leg_out = body.l_leg.is_crippled() || body.r_leg.is_crippled();
    if arm_out || leg_out {
        let after_x = 1 + hp_line.len() as i32 + 1 + format!(" SP {}", stam_cur.max(0)).len() as i32 + 1;
        let tag = match (arm_out, leg_out) {
            (true, true) => "[no weapon / lame]",
            (true, false) => "[no weapon]",
            (false, true) => "[lame]",
            (false, false) => "",
        };
        put_text(&mut cells, after_x, 2, tag, palette.need_critical_fg, palette.hud_bg);
    }

    // Per-skill readouts live in the Select info menu's Skills tab —
    // base HUD reserves row 2 for vitals (HP + crippled status) so
    // combat-relevant info reads cleanly at a glance. `skills` stays
    // in the signature for future right-of-HP indicators.
    let _ = skills;

    cells
}

/// Per-cell ±8 RGB offset on a walkable terrain's `(fg, bg)` based on a
/// hash of `(x, y, world.seed)`. The result is the terrain's base
/// color jittered slightly per cell so the floor reads as gradient
/// texture instead of a flat region. Three independent hashes for R,
/// G, B keep the variation organic rather than monochromatic.
fn floor_with_gradient(
    base_fg: [u8; 3],
    base_bg: [u8; 3],
    x: i32,
    y: i32,
    seed: u64,
) -> ([u8; 3], [u8; 3]) {
    let base = (x as i64)
        .wrapping_mul(73_856_093)
        .wrapping_add((y as i64).wrapping_mul(19_349_663))
        .wrapping_add(seed as i64);
    // Three uncorrelated mixers per channel.
    let mr = base.wrapping_mul(2_654_435_761_i64) as i32;
    let mg = base.wrapping_mul(40_503_i64) ^ 0x9E37_79B9;
    let mb = base.wrapping_mul(2_246_822_519_i64) as i32;
    // Offset range: ±8 per channel. The mod-17 maps to 0..=16; we
    // subtract 8 to center on zero.
    let dr = ((mr.rem_euclid(17)) - 8) as i16;
    let dg = ((mg as i32).rem_euclid(17) - 8) as i16;
    let db = ((mb.rem_euclid(17)) - 8) as i16;
    let fg = [
        (base_fg[0] as i16 + dr).clamp(0, 255) as u8,
        (base_fg[1] as i16 + dg).clamp(0, 255) as u8,
        (base_fg[2] as i16 + db).clamp(0, 255) as u8,
    ];
    let bg = [
        (base_bg[0] as i16 + dr / 2).clamp(0, 255) as u8,
        (base_bg[1] as i16 + dg / 2).clamp(0, 255) as u8,
        (base_bg[2] as i16 + db / 2).clamp(0, 255) as u8,
    ];
    (fg, bg)
}

/// True if any of the 8 neighboring cells at `(wx, wy)` holds a
/// TreeTrunk with a deciduous species. Drives the autumn FallenLeaves
/// render-time overlay. Walks `World::cell_at` so it works across
/// chunk seams when the ring is loaded.
fn has_deciduous_neighbor(world: &World, wx: i64, wy: i64) -> bool {
    for dy in -1..=1_i64 {
        for dx in -1..=1_i64 {
            if dx == 0 && dy == 0 {
                continue;
            }
            if let Some(cell) = world.cell_at(wx + dx, wy + dy) {
                if cell.terrain == TerrainKind::TreeTrunk {
                    if let Some(sp) = cell.tree_species {
                        if sp.is_deciduous() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Linear RGB blend from `a` toward `b` by `t` in [0.0, 1.0]. Used by
/// the render path for ground-cover and Snow bg lerps. Surface-level
/// color math runs fine on mmiyoo SDL2 (the broken bits are texture
/// color mod, not surface composition).
fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

/// Linearly mix an item color toward a terrain fg. `mix` is the
/// fraction of the item color retained (0.0 = pure terrain, 1.0 = pure
/// item). Used for `blends_with_terrain` items so organic detritus
/// merges into the floor texture instead of clashing.
fn blend_to_terrain(item_rgb: [u8; 3], terrain_fg: [u8; 3], mix: f32) -> Color {
    let m = mix.clamp(0.0, 1.0);
    let one = 1.0 - m;
    let r = (item_rgb[0] as f32 * m + terrain_fg[0] as f32 * one) as u8;
    let g = (item_rgb[1] as f32 * m + terrain_fg[1] as f32 * one) as u8;
    let b = (item_rgb[2] as f32 * m + terrain_fg[2] as f32 * one) as u8;
    Color::RGB(r, g, b)
}

/// Index into `TREE_VARIANT_GLYPHS` for a given cell. Deterministic
/// per `(x, y, world.seed)` so the same cell always shows the same
/// tree silhouette. Differs from the grass-dot hash via different
/// mixer constants so neighboring cells don't visually correlate.
fn tree_variant_index(x: i32, y: i32, seed: u64) -> usize {
    let h = (x as i64)
        .wrapping_mul(467_213)
        .wrapping_add((y as i64).wrapping_mul(2_654_435_761))
        .wrapping_add(seed as i64);
    let mixed = (h as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (mixed >> 28) as usize
}

/// Returns true for ~25% of grass cells, deterministically per
/// `(x, y, world.seed)`. Used by the render loop to render a sparse
/// pattern of tufts across grass rather than a wall of glyphs.
fn grass_dot_visible(x: i32, y: i32, seed: u64) -> bool {
    // Mixing constants: Knuth's multiplicative hash + two large primes
    // for spatial decorrelation, then a final golden-ratio shuffle so
    // adjacent cells don't show banding.
    let h = (x as i64)
        .wrapping_mul(73_856_093)
        .wrapping_add((y as i64).wrapping_mul(19_349_663))
        .wrapping_add(seed as i64);
    let mixed = (h as u64).wrapping_mul(2_654_435_761);
    (mixed >> 24) % 100 < 25
}

/// Bottom-left "what's underfoot" line. Reads the player's current
/// cell's `items` and prints a comma-separated list (with stack counts
/// and `(pitched)` markers) on the last row. Renders nothing when the
/// cell is empty so empty grass doesn't get visual chrome.
///
/// Width budget: starts at col 1, ends before col 39. Truncates with
/// `...` if the join overflows.
/// Surface the two most recent log entries on consecutive rows above
/// the here-line. Two rows are enough that a single combat exchange
/// (player swing + bandit reply) is fully visible without the second
/// message swallowing the first. Older entries scroll off; a full
/// scroll-back panel is a follow-up.
fn draw_message_line(cells: &mut [Option<Cell>], world: &World, palette: &Palette) {
    let count = world.message_log.len();
    if count == 0 {
        return;
    }
    let max = (WORLD_W as usize).saturating_sub(2);
    let truncate = |s: &str| {
        if s.len() <= max {
            s.to_string()
        } else {
            let mut t = s.to_string();
            t.truncate(max.saturating_sub(3));
            t.push_str("...");
            t
        }
    };
    // Bottom row (just above the here-line) is the *newest* entry; the
    // row above it is the previous entry (dimmed so the eye glides to
    // the newest first). Reads top→bottom as "earlier, then now."
    let newest = world.message_log.back().expect("count > 0");
    let newest_row = WORLD_H as i32 - 2;
    put_text(cells, 1, newest_row, &truncate(newest), palette.hud_fg, palette.hud_bg);
    if count >= 2 {
        let prev = &world.message_log[count - 2];
        let prev_row = WORLD_H as i32 - 3;
        put_text(cells, 1, prev_row, &truncate(prev), palette.panel_dim_fg, palette.hud_bg);
    }
}

/// Paint the ranged-targeting cursor overlay: yellow '+' on the cursor
/// cell and a HUD line above the here-line showing range / to-hit% /
/// target info. The cursor sits on top of whatever's in the cell;
/// we don't blank the underlying glyph so the player still sees the
/// bandit through the crosshair.
fn draw_target_cursor(
    cells: &mut [Option<Cell>],
    world: &World,
    cursor: &TargetCursor,
    cam_x: i64,
    cam_y: i64,
    palette: &Palette,
) {
    let vx = (cursor.pos.x as i64 - cam_x) as i32;
    let vy = (cursor.pos.y as i64 - cam_y) as i32;
    if vx >= 0 && vy >= 0 && (vx as u32) < WORLD_W && (vy as u32) < WORLD_H {
        let cursor_fg = Color::RGB(240, 220, 60);
        // Underlying cell's bg is preserved by reading prev_cells via
        // put_cell-on-top — for simplicity, paint a '+' over whatever
        // bg is at that index.
        let idx = (vy as u32 * WORLD_W + vx as u32) as usize;
        let bg = cells[idx].map(|c| c.bg).unwrap_or(palette.hud_bg);
        cells[idx] = Some(Cell { glyph: b'+', fg: cursor_fg, bg });
    }
    // HUD line above the here-line: "Aim: chest  hit 65%  rng 4/10".
    let player_pos = world.player_pos();
    let dx = cursor.pos.x - player_pos.x;
    let dy = cursor.pos.y - player_pos.y;
    let range = dx.abs().max(dy.abs()) as u8;
    let target = world.hostile_at(cursor.pos.x, cursor.pos.y);
    let los = world.player_has_los_to(cursor.pos);
    let row = WORLD_H as i32 - 2;
    let line = match target {
        Some(t) => {
            let pct = world.estimate_ranged_hit_pct(t);
            if range > cursor.max_range {
                format!("Aim: out of range  rng {}/{}", range, cursor.max_range)
            } else if !los {
                format!("Aim: no LoS  rng {}/{}", range, cursor.max_range)
            } else {
                format!("Aim: target  hit {}%  rng {}/{}", pct, range, cursor.max_range)
            }
        }
        None => format!("Aim: empty  rng {}/{}", range, cursor.max_range),
    };
    let max = (WORLD_W as usize).saturating_sub(2);
    let mut s = line;
    if s.len() > max {
        s.truncate(max.saturating_sub(3));
        s.push_str("...");
    }
    put_text(cells, 1, row, &s, palette.hud_fg, palette.hud_bg);
}

fn draw_here_line(cells: &mut [Option<Cell>], world: &World, palette: &Palette) {
    let row = WORLD_H as i32 - 1;
    // Godmode badge — render flush-right so it doesn't collide with
    // the here-line item label that anchors at column 1.
    if world.godmode {
        let tag = "[GOD]";
        let x = WORLD_W as i32 - tag.len() as i32 - 1;
        put_text(
            cells,
            x,
            row,
            tag,
            palette.need_critical_fg,
            palette.hud_bg,
        );
    }
    let pos = world.player_pos();
    let Some(cell) = world.cell_at(pos.x as i64, pos.y as i64) else {
        return;
    };
    if cell.items.is_empty() {
        return;
    }
    let parts: Vec<String> = cell.items.iter().map(|i| i.display_label()).collect();
    let mut joined = parts.join(", ");
    let max = (WORLD_W as usize).saturating_sub(2);
    if joined.len() > max {
        joined.truncate(max.saturating_sub(3));
        joined.push_str("...");
    }
    put_text(cells, 1, row, &joined, palette.hud_fg, palette.hud_bg);
}

/// Positioning + interior anchors for any UI panel (pause menu, command
/// menu, multi-turn banner). Two construction forms cover slice-1
/// needs; add more if a future panel doesn't fit centered-or-anchored.
struct PanelLayout {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl PanelLayout {
    fn anchored(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    fn centered(w: i32, h: i32) -> Self {
        Self {
            x: ((WORLD_W as i32) - w) / 2,
            y: ((WORLD_H as i32) - h) / 2,
            w,
            h,
        }
    }
    fn centered_x_at(y: i32, w: i32, h: i32) -> Self {
        Self {
            x: ((WORLD_W as i32) - w) / 2,
            y,
            w,
            h,
        }
    }
    fn inner_x(&self) -> i32 {
        self.x + 2
    }
    fn inner_right(&self) -> i32 {
        self.x + self.w - 2
    }
    fn title_y(&self) -> i32 {
        self.y + 1
    }
    /// First row of body content. Convention: leave one blank row
    /// between title and body for visual breathing room.
    fn first_row_y(&self) -> i32 {
        self.y + 3
    }
    fn footer_y(&self) -> i32 {
        self.y + self.h - 2
    }
}

/// Draw a bordered window with a title (top) and footer (bottom). Body
/// content is the caller's responsibility — use `draw_menu_row` for the
/// canonical cursor + label + right-aligned-status row pattern, or
/// call `put_text` / `put_cell` directly for one-off shapes like the
/// multi-turn progress bar.
fn draw_panel_frame(
    cells: &mut [Option<Cell>],
    layout: &PanelLayout,
    title: &str,
    footer: &str,
    palette: &Palette,
) {
    draw_panel(
        cells,
        layout.x,
        layout.y,
        layout.w,
        layout.h,
        palette.panel_fg,
        palette.panel_bg,
    );
    put_text(
        cells,
        layout.inner_x(),
        layout.title_y(),
        title,
        palette.panel_title_fg,
        palette.panel_bg,
    );
    put_text(
        cells,
        layout.inner_x(),
        layout.footer_y(),
        footer,
        palette.panel_dim_fg,
        palette.panel_bg,
    );
}

/// Canonical menu row: cursor (>/blank) + label + optional right-aligned
/// status. The right-side text is truncated to fit the remaining inner
/// width with `…` so a long reason can't overflow the box.
fn draw_menu_row(
    cells: &mut [Option<Cell>],
    layout: &PanelLayout,
    row_y: i32,
    is_selected: bool,
    label: &str,
    label_fg: Color,
    right_status: Option<(&str, Color)>,
    palette: &Palette,
) {
    let cursor = if is_selected { b'>' } else { b' ' };
    put_cell(
        cells,
        layout.inner_x(),
        row_y,
        Cell {
            glyph: cursor,
            fg: palette.panel_title_fg,
            bg: palette.panel_bg,
        },
    );
    put_text(
        cells,
        layout.inner_x() + 2,
        row_y,
        label,
        label_fg,
        palette.panel_bg,
    );
    if let Some((status, status_fg)) = right_status {
        // Available width = inner width minus cursor (1) + space (1) +
        // label + separator (2). Floor at 4 so very long labels still
        // leave a stub for the status.
        let max_status_len = (layout.w as i32 - 4 - label.len() as i32 - 2).max(4) as usize;
        let truncated: String = status.chars().take(max_status_len).collect();
        let status_x = layout.inner_right() - truncated.len() as i32;
        put_text(cells, status_x, row_y, &truncated, status_fg, palette.panel_bg);
    }
}

/// Dev overlay: render every CP437 byte (0x00–0xFF) in a 16x16 grid so
/// we can audit what's actually in our custom atlas. The cursor byte
/// is inverted (bg <-> fg) and shown in the header. Dpad navigates
/// (wraps); B/X closes.
fn draw_glyph_palette(cells: &mut [Option<Cell>], cursor: u8, palette: &Palette) {
    let layout = PanelLayout::centered(36, 24);
    let cur_row = cursor >> 4;
    let cur_col = cursor & 0x0F;
    let title = format!(
        "CP437 0x{:02X} (row {:X}, col {:X})",
        cursor, cur_row, cur_col
    );
    draw_panel_frame(
        cells,
        &layout,
        &title,
        "dpad: navigate   B: close",
        palette,
    );

    // The grid sits at first_row_y, taking 16 rows x 16 columns. We
    // also draw thin row/column labels in hex above and beside the
    // grid so the player can read coords without counting.
    let grid_x = layout.inner_x() + 2; // leave 2 cols for row labels
    let grid_y = layout.first_row_y() + 1; // row above is column header

    // Column header row.
    for col in 0..16u8 {
        put_text(
            cells,
            grid_x + col as i32,
            grid_y - 1,
            &format!("{:X}", col),
            palette.panel_dim_fg,
            palette.panel_bg,
        );
    }

    // Row labels + cells.
    for row in 0..16u8 {
        put_text(
            cells,
            layout.inner_x(),
            grid_y + row as i32,
            &format!("{:X}", row),
            palette.panel_dim_fg,
            palette.panel_bg,
        );
        for col in 0..16u8 {
            let byte = (row << 4) | col;
            let is_cursor = byte == cursor;
            // Render the glyph at its real byte index. Inversion on
            // the cursor cell (bg as fg, fg as bg) so it stands out.
            let (fg, bg) = if is_cursor {
                (palette.panel_bg, palette.panel_fg)
            } else {
                (palette.panel_fg, palette.panel_bg)
            };
            put_cell(
                cells,
                grid_x + col as i32,
                grid_y + row as i32,
                Cell { glyph: byte, fg, bg },
            );
        }
    }
}

// ---- Info hub (Select-button tabbed overlay) -------------------------

fn info_tab_row_count(world: &World, tab: InfoTab) -> usize {
    match tab {
        // Inventory shows every equip slot (8) on top, then every pack
        // item. A on a slot row unequips; A on a pack row equips.
        InfoTab::Inventory => world::EquipSlot::ALL.len() + world.player_pack().contents.len(),
        InfoTab::Crafting => crafting::RECIPES.len(),
        // Fire Making, Foraging, Melee, Ranged, Dodge.
        InfoTab::Skills => 5,
    }
}

/// Map an inventory-tab row index to the action it should trigger when
/// A is pressed. `None` means the row is informational only.
enum InventoryRow {
    EquipSlot(world::EquipSlot),
    PackItem(items::ItemKind),
}

fn inventory_row_at(world: &World, selected: usize) -> Option<InventoryRow> {
    let slot_count = world::EquipSlot::ALL.len();
    if selected < slot_count {
        return Some(InventoryRow::EquipSlot(world::EquipSlot::ALL[selected]));
    }
    let pack_idx = selected - slot_count;
    world
        .player_pack()
        .contents
        .get(pack_idx)
        .map(|i| InventoryRow::PackItem(i.kind))
}

fn draw_info_menu(
    cells: &mut [Option<Cell>],
    world: &World,
    state: &InfoMenuState,
    palette: &Palette,
) {
    let layout = PanelLayout::centered(36, 22);
    let footer = "L/R: tabs   B/Select: close";
    // Title row is rendered manually below so we can highlight the
    // current tab.
    draw_panel_frame(cells, &layout, "", footer, palette);

    // Tab strip on the title row: "Inventory | Skills" with the active
    // tab in panel-fg + others dimmed.
    let mut x = layout.inner_x();
    for (i, &tab) in INFO_TABS.iter().enumerate() {
        let active = tab == state.tab;
        let fg = if active {
            palette.panel_title_fg
        } else {
            palette.panel_dim_fg
        };
        let label = tab.label();
        put_text(cells, x, layout.title_y(), label, fg, palette.panel_bg);
        x += label.len() as i32;
        if i + 1 < INFO_TABS.len() {
            put_text(cells, x, layout.title_y(), " | ", palette.panel_dim_fg, palette.panel_bg);
            x += 3;
        }
    }

    // Body branches on the active tab.
    match state.tab {
        InfoTab::Inventory => draw_info_inventory(cells, &layout, world, state.selected, palette),
        InfoTab::Crafting => draw_info_crafting(cells, &layout, world, state.selected, palette),
        InfoTab::Skills => draw_info_skills(cells, &layout, world, state.selected, palette),
    }
}

/// Crafting tab. Lists the slice-1 recipe catalog with green/red
/// glyphs per requirement; the same `action::evaluate` dispatcher used
/// by the command menu decides availability so the two stay in sync.
fn draw_info_crafting(
    cells: &mut [Option<Cell>],
    layout: &PanelLayout,
    world: &World,
    selected: usize,
    palette: &Palette,
) {
    let max_rows = (layout.footer_y() - layout.first_row_y() - 1).max(1) as usize;
    let visible = crafting::RECIPES.iter().take(max_rows);
    for (i, recipe) in visible.enumerate() {
        let row_y = layout.first_row_y() + i as i32;
        let is_selected = i == selected;
        let avail = action::evaluate(world, recipe.action);
        let (right, dim) = match avail {
            action::Availability::Available { cost_game_seconds } => {
                (format!("ok {}s", cost_game_seconds), false)
            }
            action::Availability::Unavailable { reason } => (reason.to_string(), true),
        };
        let fg = if dim {
            palette.panel_dim_fg
        } else {
            palette.panel_fg
        };
        draw_menu_row(
            cells,
            layout,
            row_y,
            is_selected,
            recipe.name,
            fg,
            Some((&right, palette.panel_dim_fg)),
            palette,
        );
    }

    // Footer-adjacent hint. Selecting a row with A queues the recipe.
    let hint = if let Some(recipe) = crafting::RECIPES.get(selected) {
        format!("A: craft  ({})", recipe.name)
    } else {
        String::new()
    };
    put_text(
        cells,
        layout.inner_x(),
        layout.footer_y() - 1,
        &hint,
        palette.hud_fg,
        palette.panel_bg,
    );
}

fn draw_info_inventory(
    cells: &mut [Option<Cell>],
    layout: &PanelLayout,
    world: &World,
    selected: usize,
    palette: &Palette,
) {
    let pack = world.player_pack();
    let equipment = world.player_equipment();

    let max_rows = (layout.footer_y() - layout.first_row_y() - 1).max(1) as usize;
    let mut row_idx = 0usize;

    // Equipment slots first: one row per slot, labelled "[slot] item" or
    // "[slot] —". Selecting one and pressing A unequips the slot.
    for &slot in &world::EquipSlot::ALL {
        if row_idx >= max_rows {
            break;
        }
        let row_y = layout.first_row_y() + row_idx as i32;
        let is_selected = row_idx == selected;
        let label = match equipment.get(slot) {
            Some(kind) => format!("[{}] {}", slot.label(), kind.name()),
            None => format!("[{}] —", slot.label()),
        };
        draw_menu_row(
            cells,
            layout,
            row_y,
            is_selected,
            &label,
            palette.panel_dim_fg,
            None,
            palette,
        );
        row_idx += 1;
    }

    // Pack rows below. Skip the empty hint if we have equipment lines
    // above — the player still sees the slot list when the pack is empty.
    if pack.contents.is_empty() && row_idx < max_rows {
        let row_y = layout.first_row_y() + row_idx as i32;
        put_text(
            cells,
            layout.inner_x(),
            row_y,
            "(pack empty)",
            palette.panel_dim_fg,
            palette.panel_bg,
        );
    } else {
        for item in pack.contents.iter() {
            if row_idx >= max_rows {
                break;
            }
            let row_y = layout.first_row_y() + row_idx as i32;
            let is_selected = row_idx == selected;
            let label = item.display_label();
            let weight = fmt_weight(item.total_weight_g());
            draw_menu_row(
                cells,
                layout,
                row_y,
                is_selected,
                &label,
                palette.panel_fg,
                Some((&weight, palette.panel_dim_fg)),
                palette,
            );
            row_idx += 1;
        }
    }

    // Pack-total summary on the row just above the footer.
    let total = fmt_weight(pack.total_weight_g());
    let cap = fmt_weight(pack.capacity_g);
    let summary = format!("Pack {} / {}", total, cap);
    put_text(
        cells,
        layout.inner_x(),
        layout.footer_y() - 1,
        &summary,
        palette.hud_fg,
        palette.panel_bg,
    );
}

fn draw_info_skills(
    cells: &mut [Option<Cell>],
    layout: &PanelLayout,
    world: &World,
    selected: usize,
    palette: &Palette,
) {
    let skills = world.player_skills();
    let rows: [(SkillKind, &skill::Skill); 5] = [
        (SkillKind::FireMaking, skills.get(SkillKind::FireMaking)),
        (SkillKind::Foraging, skills.get(SkillKind::Foraging)),
        (SkillKind::Melee, skills.get(SkillKind::Melee)),
        (SkillKind::Ranged, skills.get(SkillKind::Ranged)),
        (SkillKind::Dodge, skills.get(SkillKind::Dodge)),
    ];

    for (i, (kind, s)) in rows.into_iter().enumerate() {
        let row_y = layout.first_row_y() + i as i32;
        let is_selected = i == selected;
        let label = kind.display_name();
        // Single row per skill: value % + daily XP banked toward next
        // level. Drops the multi-line sub-row pattern so all five fit
        // in the 22-cell panel without scrolling.
        let value = format!("{}% (+{}xp)", s.value, s.daily_xp);
        draw_menu_row(
            cells,
            layout,
            row_y,
            is_selected,
            label,
            palette.panel_fg,
            Some((&value, palette.panel_dim_fg)),
            palette,
        );
    }
}

fn fmt_weight(g: u32) -> String {
    if g >= 1000 {
        format!("{:.1} kg", g as f32 / 1000.0)
    } else {
        format!("{} g", g)
    }
}

/// Phase 15 hold-Y radial. Compact 25x5 panel with the 4 RADIAL_BINDINGS
/// arranged cardinally; unavailable verbs greyed via panel_dim_fg so
/// the player sees at a glance which they can fire right now.
fn draw_radial_menu(cells: &mut [Option<Cell>], world: &World, palette: &Palette) {
    let layout = PanelLayout::centered(25, 5);
    draw_panel_frame(cells, &layout, "radial", "release Y", palette);

    let inner_left = layout.inner_x();
    let inner_right = layout.inner_right();
    let inner_top = layout.first_row_y();
    let mid_row = inner_top + 1;
    let bot_row = inner_top + 2;
    let inner_w = inner_right - inner_left + 1;

    let pick_fg = |id: action::ActionId| -> Color {
        match action::evaluate(world, id) {
            action::Availability::Available { .. } => palette.panel_fg,
            action::Availability::Unavailable { .. } => palette.panel_dim_fg,
        }
    };

    for &(dir, id, name) in &RADIAL_BINDINGS {
        let fg = pick_fg(id);
        match dir {
            RadialDir::Up => {
                // Top row, center.
                let x = inner_left + (inner_w - name.len() as i32) / 2;
                put_text(cells, x, inner_top, name, fg, palette.panel_bg);
            }
            RadialDir::Down => {
                let x = inner_left + (inner_w - name.len() as i32) / 2;
                put_text(cells, x, bot_row, name, fg, palette.panel_bg);
            }
            RadialDir::Left => {
                put_text(cells, inner_left, mid_row, name, fg, palette.panel_bg);
            }
            RadialDir::Right => {
                let x = inner_right - name.len() as i32 + 1;
                put_text(cells, x, mid_row, name, fg, palette.panel_bg);
            }
        }
    }
}

fn draw_death_screen(cells: &mut [Option<Cell>], cause: DeathCause, palette: &Palette) {
    let layout = PanelLayout::centered(32, 7);
    draw_panel_frame(
        cells,
        &layout,
        "* DEAD *",
        "A: new run    Start: quit",
        palette,
    );
    let row_y = layout.first_row_y();
    draw_menu_row(
        cells,
        &layout,
        row_y,
        true,
        cause.epitaph(),
        palette.need_critical_fg,
        None,
        palette,
    );
}

fn draw_pause_menu(cells: &mut [Option<Cell>], selected: usize, palette: &Palette) {
    let layout = PanelLayout::centered(28, 9);
    draw_panel_frame(
        cells,
        &layout,
        "Paused",
        "A: confirm   B/Start: back",
        palette,
    );

    for (i, (action, label)) in PAUSE_OPTIONS.iter().enumerate() {
        let row_y = layout.first_row_y() + i as i32;
        let is_selected = i == selected;
        // ResetSave row is always tinted red — destructive option
        // visibility shouldn't depend on the selection cursor.
        let label_fg = match (is_selected, action) {
            (_, PauseAction::ResetSave) => palette.need_critical_fg,
            (true, _) => palette.panel_fg,
            (false, _) => palette.hud_fg,
        };
        draw_menu_row(
            cells,
            &layout,
            row_y,
            is_selected,
            label,
            label_fg,
            None,
            palette,
        );
    }
}

fn draw_multi_turn_banner(
    cells: &mut [Option<Cell>],
    active: &world::ActiveAction,
    palette: &Palette,
) {
    let Some(step) = active.current_step() else {
        return;
    };
    let total_remaining = active.total_remaining_secs();
    let mode_tag = match active.view_mode {
        ViewMode::ProgressBar => "watching",
        ViewMode::TimeSkip => "time-skip",
    };
    // Compact h=5 layout (no blank rows around the body — banner
    // intentionally doesn't dominate the screen during a 300-sec pitch):
    //   +--- pitch_tent ---+
    //   | [###......]  ... |   body row
    //   | B cancel ...     |   footer row
    //   +------------------+
    let layout = PanelLayout::centered_x_at(4, 34, 5);
    let footer = format!("B cancel    Select toggle ({})", mode_tag);
    draw_panel_frame(cells, &layout, step.id.save_key(), &footer, palette);

    let body_y = layout.y + 2;
    let bar_w: i32 = 12;
    let filled = if step.target_secs == 0 {
        bar_w
    } else {
        ((step.elapsed_secs as i64 * bar_w as i64) / step.target_secs as i64) as i32
    };
    for i in 0..bar_w {
        let glyph = if i < filled { 0xDB } else { 0xB1 }; // █ vs ▒
        put_cell(
            cells,
            layout.inner_x() + i,
            body_y,
            Cell {
                glyph,
                fg: palette.panel_fg,
                bg: palette.panel_bg,
            },
        );
    }
    let remaining_m = total_remaining / 60;
    let remaining_s = total_remaining % 60;
    let remaining = format!(" {}:{:02} remaining", remaining_m, remaining_s);
    put_text(
        cells,
        layout.inner_x() + bar_w,
        body_y,
        &remaining,
        palette.hud_fg,
        palette.panel_bg,
    );
}

fn draw_command_menu(
    cells: &mut [Option<Cell>],
    world: &World,
    selected: usize,
    palette: &Palette,
) {
    let layout = PanelLayout::anchored(2, 4, 36, 21);
    draw_panel_frame(
        cells,
        &layout,
        "Actions",
        "A: confirm   B/Y: close",
        palette,
    );

    for (i, ca) in action::ALL_ACTIONS.iter().enumerate() {
        let row_y = layout.first_row_y() + i as i32;
        let is_selected = i == selected;

        let avail = action::evaluate(world, ca.id);
        let available = matches!(avail, action::Availability::Available { .. });
        // Selected + available -> full panel fg (highlight).
        // Selected + unavailable -> critical red (you tried to confirm
        //     a verb that can't run; this color reinforces the bounce).
        // Unselected + available -> panel fg.
        // Unselected + unavailable -> dim fg (greyed-out catalog row).
        let label_fg = match (is_selected, available) {
            (true, false) => palette.need_critical_fg,
            (_, true) => palette.panel_fg,
            (false, false) => palette.panel_dim_fg,
        };

        let (status_text, status_fg) = match avail {
            action::Availability::Available { cost_game_seconds } => {
                (format!("{}s", cost_game_seconds), palette.panel_fg)
            }
            action::Availability::Unavailable { reason } => {
                (reason.to_string(), palette.panel_dim_fg)
            }
        };

        draw_menu_row(
            cells,
            &layout,
            row_y,
            is_selected,
            ca.name,
            label_fg,
            Some((&status_text, status_fg)),
            palette,
        );
    }

    // Description for the selected row, one line above the footer.
    let desc_y = layout.footer_y() - 1;
    if let Some(sel) = action::ALL_ACTIONS.get(selected) {
        let max_desc_len = (layout.w as usize).saturating_sub(4);
        let desc: String = sel.description.chars().take(max_desc_len).collect();
        put_text(
            cells,
            layout.inner_x(),
            desc_y,
            &desc,
            palette.hud_fg,
            palette.panel_bg,
        );
    }
}

fn draw_panel(
    cells: &mut [Option<Cell>],
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    fg: Color,
    bg: Color,
) {
    for py in y..(y + h) {
        for px in x..(x + w) {
            let glyph = if (px == x || px == x + w - 1) && (py == y || py == y + h - 1) {
                b'+'
            } else if py == y || py == y + h - 1 {
                b'-'
            } else if px == x || px == x + w - 1 {
                b'|'
            } else {
                b' '
            };
            put_cell(cells, px, py, Cell { glyph, fg, bg });
        }
    }
}

fn tint_color(c: Color, t: f32) -> Color {
    Color::RGBA(
        (c.r as f32 * t) as u8,
        (c.g as f32 * t) as u8,
        (c.b as f32 * t) as u8,
        c.a,
    )
}

fn put_text(cells: &mut [Option<Cell>], x: i32, y: i32, text: &str, fg: Color, bg: Color) {
    for (i, b) in text.bytes().enumerate() {
        put_cell(cells, x + i as i32, y, Cell { glyph: b, fg, bg });
    }
}

/// Full-screen Cornwall overmap mode. Cursor + biome view. `last_destination`
/// is wired here so the cursor can default back to the most recent
/// fast-travel target on re-open (resume-from-interrupt per design doc §6.3).
struct OvermapMode {
    cursor: ChunkCoord,
}

impl OvermapMode {
    fn open(world: &World, last_destination: Option<ChunkCoord>) -> Self {
        let cursor = last_destination.unwrap_or_else(|| world.player_chunk());
        Self { cursor }
    }
    fn move_cursor(&mut self, dx: i32, dy: i32) {
        self.cursor = ChunkCoord {
            cx: self.cursor.cx + dx,
            cy: self.cursor.cy + dy,
        };
    }
}

/// Average game-seconds per chunk crossing for the overmap's travel-
/// time estimate. Real cost varies by biome+road; this is the eyeballed
/// midpoint. Cheap enough to recompute per-frame.
const OVERMAP_AVG_CHUNK_SECS: u32 = 90;

fn draw_overmap(
    cells: &mut [Option<Cell>],
    world: &World,
    mode: &OvermapMode,
    palette: &Palette,
    frame_count: u64,
) {
    let player_chunk = world.player_chunk();
    let discovered = world.discovered_chunks();
    let view_w = WORLD_W as i32;
    let view_h = WORLD_H as i32 - 1; // bottom row reserved for info line
    let half_w = view_w / 2;
    let half_h = view_h / 2;

    // Player chunk renders at (half_w, half_h). Fill every cell of the
    // viewport so terrain underneath doesn't bleed through.
    for vy in 0..view_h {
        for vx in 0..view_w {
            let cc = ChunkCoord {
                cx: player_chunk.cx + (vx - half_w),
                cy: player_chunk.cy + (vy - half_h),
            };

            let (glyph, fg) = if !discovered.contains(&cc) {
                (b'?', color_dim())
            } else {
                let info = cornwall::overmap_info_at(cc);
                // Display priority at a chunk: named-site (anchor only)
                // wins over river / road / biome. Otherwise rivers
                // visually take precedence over roads (you'd notice a
                // river crossing a road, not the road under it), and
                // roads override the biome glyph.
                if let Some(site) = info.named_site {
                    if cornwall::chunk_for_anchor(site) == cc {
                        (
                            site.kind.overmap_glyph(),
                            color_from_rgb(site.kind.overmap_fg()),
                        )
                    } else {
                        chunk_glyph_color(cc, &info)
                    }
                } else {
                    chunk_glyph_color(cc, &info)
                }
            };

            // Player chunk: flashing `@` on top.
            let (glyph, fg) = if cc == player_chunk {
                let bright = (frame_count / 30) % 2 == 0;
                (
                    b'@',
                    if bright {
                        palette.player_fg
                    } else {
                        palette.panel_dim_fg
                    },
                )
            } else {
                (glyph, fg)
            };

            // Cursor overlay (replaces glyph entirely so the marker is
            // unambiguous, but keeps the underlying fg color so the
            // biome is still hinted).
            let (glyph, fg) = if cc == mode.cursor && cc != player_chunk {
                (b'+', palette.panel_title_fg)
            } else {
                (glyph, fg)
            };

            put_cell(
                cells,
                vx,
                vy,
                Cell {
                    glyph,
                    fg,
                    bg: palette.panel_bg,
                },
            );
        }
    }

    // Bottom info line: biome name · site name (if any) · travel time.
    let cursor_info = cornwall::overmap_info_at(mode.cursor);
    let biome_name = cursor_info.biome.display_name();
    let site_name = cursor_info.named_site.map(|s| s.name).unwrap_or("");
    let cheby = (mode.cursor.cx - player_chunk.cx)
        .unsigned_abs()
        .max((mode.cursor.cy - player_chunk.cy).unsigned_abs());
    let est_secs = cheby.saturating_mul(OVERMAP_AVG_CHUNK_SECS);
    let est_hours = est_secs / 3600;
    let est_mins = (est_secs % 3600) / 60;
    let line = if site_name.is_empty() {
        if cheby == 0 {
            format!("{} · (here)", biome_name)
        } else {
            format!("{} · ~{}h{:02}m", biome_name, est_hours, est_mins)
        }
    } else {
        format!(
            "{} · {} · ~{}h{:02}m",
            biome_name, site_name, est_hours, est_mins
        )
    };
    // Pad the info line to full width so terrain bleed is hidden.
    let info_y = WORLD_H as i32 - 1;
    for x in 0..WORLD_W as i32 {
        put_cell(
            cells,
            x,
            info_y,
            Cell {
                glyph: b' ',
                fg: palette.panel_fg,
                bg: palette.panel_bg,
            },
        );
    }
    put_text(cells, 1, info_y, &line, palette.panel_fg, palette.panel_bg);
    // Footer hint along the right.
    let hint = "A: travel  M/B: close";
    let hint_x = WORLD_W as i32 - hint.len() as i32 - 1;
    put_text(
        cells,
        hint_x,
        info_y,
        hint,
        palette.panel_dim_fg,
        palette.panel_bg,
    );
}

/// Pick the glyph + color for a non-site chunk on the overmap. Rivers
/// win over roads (the visual is "river crossing a road"); roads win
/// over the underlying biome. Road glyph uses CP437 box-drawing
/// connectors derived from neighboring chunks' `has_road` flags so the
/// network draws as a continuous line.
fn chunk_glyph_color(cc: ChunkCoord, info: &cornwall::OvermapInfo) -> (u8, Color) {
    if info.has_river {
        let fg = cornwall::Biome::RiverValley.overmap_fg();
        return (cornwall::Biome::RiverValley.overmap_glyph(), color_from_rgb(fg));
    }
    if info.has_road {
        return (road_connector_glyph(cc), color_from_rgb(ROAD_FG));
    }
    (info.biome.overmap_glyph(), color_from_rgb(info.biome.overmap_fg()))
}

/// Beaten-earth highway tint. Tan-yellow so the road network reads as
/// distinct from forest greens, moor grays, and water blues.
const ROAD_FG: [u8; 3] = [200, 175, 110];

/// CP437 box-drawing connector for a road chunk, picked from which of
/// the 4 cardinal neighbors are also road chunks. Now that `has_road`
/// uses segment-vs-chunk-bbox intersection (so a diagonal polyline
/// only flags chunks it actually crosses, not a 2-wide band), the
/// connector pattern is a clean staircase — `└┐` pairs over diagonal
/// segments, `─` runs over horizontal, `│` runs over vertical — with
/// no closed-loop artifacts.
fn road_connector_glyph(cc: ChunkCoord) -> u8 {
    let n = cornwall::overmap_info_at(ChunkCoord { cx: cc.cx, cy: cc.cy - 1 }).has_road;
    let s = cornwall::overmap_info_at(ChunkCoord { cx: cc.cx, cy: cc.cy + 1 }).has_road;
    let e = cornwall::overmap_info_at(ChunkCoord { cx: cc.cx + 1, cy: cc.cy }).has_road;
    let w = cornwall::overmap_info_at(ChunkCoord { cx: cc.cx - 1, cy: cc.cy }).has_road;
    match (n, s, e, w) {
        (true, true, true, true) => 0xC5,    // ┼
        (true, true, true, false) => 0xC3,   // ├
        (true, true, false, true) => 0xB4,   // ┤
        (true, false, true, true) => 0xC1,   // ┴
        (false, true, true, true) => 0xC2,   // ┬
        (true, true, false, false) => 0xB3,  // │
        (false, false, true, true) => 0xC4,  // ─
        (true, false, true, false) => 0xC0,  // └
        (true, false, false, true) => 0xD9,  // ┘
        (false, true, true, false) => 0xDA,  // ┌
        (false, true, false, true) => 0xBF,  // ┐
        // Single-direction stub or isolated cell — generic horizontal.
        _ => 0xC4,
    }
}

fn color_from_rgb(rgb: [u8; 3]) -> Color {
    Color::RGB(rgb[0], rgb[1], rgb[2])
}

fn color_dim() -> Color {
    Color::RGB(80, 75, 65)
}

/// Top-of-screen banner shown while fast-travel is active. Destination
/// label = named site if known, else chunk coord. Time remaining =
/// queue.cells.len() × per-cell game-sec advance.
fn draw_fast_travel_banner(
    cells: &mut [Option<Cell>],
    queue: &crate::fasttravel::FastTravelQueue,
    palette: &Palette,
) {
    let dest_label = cornwall::overmap_info_at(queue.destination)
        .named_site
        .map(|s| s.name)
        .map(String::from)
        .unwrap_or_else(|| format!("({}, {})", queue.destination.cx, queue.destination.cy));
    let remaining = queue.remaining_est_secs();
    let h = remaining / 3600;
    let m = (remaining % 3600) / 60;
    let msg = format!(
        "→ Travelling to {} · ~{}h{:02}m · press anything to stop",
        dest_label, h, m
    );
    // Banner row 0, full-width.
    for x in 0..WORLD_W as i32 {
        put_cell(
            cells,
            x,
            0,
            Cell {
                glyph: b' ',
                fg: palette.panel_fg,
                bg: palette.panel_bg,
            },
        );
    }
    let max_w = WORLD_W as i32 - 2;
    let truncated: String = msg.chars().take(max_w as usize).collect();
    put_text(cells, 1, 0, &truncated, palette.panel_fg, palette.panel_bg);
}

fn put_cell(cells: &mut [Option<Cell>], x: i32, y: i32, cell: Cell) {
    if x < 0 || y < 0 || x >= WORLD_W as i32 || y >= WORLD_H as i32 {
        return;
    }
    cells[(y as u32 * WORLD_W + x as u32) as usize] = Some(cell);
}

#[derive(Default)]
struct FrameTiming {
    frames: u32,
    changed_cells: u32,
    draw: Duration,
    upload: Duration,
    copy: Duration,
    present: Duration,
    sleep: Duration,
}

impl FrameTiming {
    fn summary(&self) -> String {
        let frames = self.frames.max(1);
        format!(
            "timing avg_ms draw={:.2} upload={:.2} copy={:.2} present={:.2} sleep={:.2} changed_cells={:.1}",
            avg_ms(self.draw, frames),
            avg_ms(self.upload, frames),
            avg_ms(self.copy, frames),
            avg_ms(self.present, frames),
            avg_ms(self.sleep, frames),
            self.changed_cells as f64 / frames as f64,
        )
    }
}

fn avg_ms(duration: Duration, frames: u32) -> f64 {
    duration.as_secs_f64() * 1000.0 / frames as f64
}

fn pace_frame(frame_start: Instant, elapsed: Duration) -> Duration {
    if elapsed >= TARGET_FRAME {
        return Duration::ZERO;
    }

    let sleep_start = Instant::now();
    let remaining = TARGET_FRAME - elapsed;
    if remaining > SLEEP_GUARD {
        std::thread::sleep(remaining - SLEEP_GUARD);
    }

    while frame_start.elapsed() < TARGET_FRAME {
        std::hint::spin_loop();
    }

    sleep_start.elapsed()
}

/// In-place 2D shift of a CPU `Surface`'s pixels by `(-dx_cells*CELL_SIZE,
/// -dy_cells*CELL_SIZE)` pixels — when the camera moves by `(+dx, +dy)`
/// cells, the *content* on screen moves by `(-dx, -dy)`. Pixels that
/// scroll off the edge are discarded; pixels exposed at the new edge are
/// left untouched (the compose loop will repaint them via `prev_cells`
/// being `None` there).
///
/// Uses `slice::copy_within` so overlapping regions are memmove-safe.
/// Row iteration direction picks correctly for either vertical scroll
/// direction.
fn shift_framebuffer(framebuf: &mut Surface, dx_cells: i32, dy_cells: i32) {
    if dx_cells == 0 && dy_cells == 0 {
        return;
    }
    let w_px = framebuf.width() as i32;
    let h_px = framebuf.height() as i32;
    let cell = CELL_SIZE as i32;
    // Content shift is the negative of camera shift.
    let content_dx = -dx_cells * cell;
    let content_dy = -dy_cells * cell;
    if content_dx.abs() >= w_px || content_dy.abs() >= h_px {
        // Whole framebuffer scrolled off; nothing to preserve.
        return;
    }
    let pitch = framebuf.pitch() as usize;
    framebuf.with_lock_mut(|pixels| {
        // Region of pixels that survives the shift.
        let src_x = (-content_dx).max(0) as usize;
        let src_y = (-content_dy).max(0) as usize;
        let dst_x = content_dx.max(0) as usize;
        let dst_y = content_dy.max(0) as usize;
        let copy_w = (w_px - content_dx.abs()) as usize;
        let copy_h = (h_px - content_dy.abs()) as usize;
        let bytes_per_row = copy_w * 4;
        let row_iter: Box<dyn Iterator<Item = usize>> = if dst_y > src_y {
            // Content moves down; iterate bottom-up to avoid clobber.
            Box::new((0..copy_h).rev())
        } else {
            Box::new(0..copy_h)
        };
        for i in row_iter {
            let sy = src_y + i;
            let dy = dst_y + i;
            let src_off = sy * pitch + src_x * 4;
            let dst_off = dy * pitch + dst_x * 4;
            pixels.copy_within(src_off..src_off + bytes_per_row, dst_off);
        }
    });
}

/// Mirror of `shift_framebuffer` for the `prev_cells` viewport-indexed
/// cache. A scrolled cell's identity matches what's already painted at
/// the new viewport coord; cells whose source was off-viewport are reset
/// to `None` so the compose loop repaints them.
fn shift_prev_cells(prev: &mut [Option<Cell>], w: u32, h: u32, dx_cells: i32, dy_cells: i32) {
    if dx_cells == 0 && dy_cells == 0 {
        return;
    }
    let w_i = w as i32;
    let h_i = h as i32;
    if dx_cells.abs() >= w_i || dy_cells.abs() >= h_i {
        for slot in prev.iter_mut() {
            *slot = None;
        }
        return;
    }
    // For each new (vx, vy) the source is (vx + dx, vy + dy) in the
    // *old* viewport. Anything outside [0, w) × [0, h) is a freshly
    // exposed edge cell and must be `None`.
    let old: Vec<Option<Cell>> = prev.to_vec();
    for new_vy in 0..h_i {
        for new_vx in 0..w_i {
            let dst_i = (new_vy as u32 * w + new_vx as u32) as usize;
            let src_vx = new_vx + dx_cells;
            let src_vy = new_vy + dy_cells;
            if src_vx < 0 || src_vx >= w_i || src_vy < 0 || src_vy >= h_i {
                prev[dst_i] = None;
            } else {
                let src_i = (src_vy as u32 * w + src_vx as u32) as usize;
                prev[dst_i] = old[src_i];
            }
        }
    }
}

struct Palette {
    letterbox: Color,
    player_fg: Color,
    hud_fg: Color,
    hud_bg: Color,
    need_critical_fg: Color,
    panel_fg: Color,
    panel_bg: Color,
    panel_dim_fg: Color,
    panel_title_fg: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            letterbox: Color::RGB(8, 6, 4),
            player_fg: Color::RGB(240, 232, 200),
            hud_fg: Color::RGB(190, 205, 160),
            hud_bg: Color::RGB(20, 17, 13),
            need_critical_fg: Color::RGB(220, 110, 90),
            panel_fg: Color::RGB(218, 205, 170),
            panel_bg: Color::RGB(28, 22, 17),
            panel_dim_fg: Color::RGB(110, 100, 80),
            panel_title_fg: Color::RGB(230, 200, 120),
        }
    }
}
