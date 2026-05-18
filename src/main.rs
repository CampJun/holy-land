mod action;
#[cfg(not(target_arch = "arm"))]
mod debug_console;
mod fov;
mod input;
mod items;
mod logging;
mod needs;
mod platform;
mod render;
mod save;
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
use save::{CellItemsSave, MetaSave, NeedsSave, RunSave, SaveHeader};
use world::{brightness_at, dawns_elapsed, Position, TerrainKind, World};

const WORLD_W: u32 = 40;
const WORLD_H: u32 = 30;
const ATLAS_PNG: &[u8] = include_bytes!("../assets/cp437_16x16.png");
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
    let mut atlas = load_atlas(ATLAS_PNG)?;
    let mut framebuf = Surface::new(logical_w, logical_h, PixelFormatEnum::ARGB8888)?;
    let mut present_tex = texture_creator
        .create_texture_streaming(PixelFormatEnum::ARGB8888, logical_w, logical_h)?;

    let mut events = sdl.event_pump()?;
    let mut input = Input::new();
    let mut world = World::new(WORLD_W, WORLD_H);
    let mut prev_meta_header = meta.header.clone();
    let mut prev_run_header: Option<SaveHeader> = None;

    if let Ok(run) = save::load_run(&save_dir.join(RUN_FILE)) {
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
        // Recompute FOV after restoring position so the visible set is
        // correct for the loaded clock + player coord. (World::new already
        // did a recompute, but the loaded position may differ.)
        world.recompute_fov();
    }

    // Track dawn crossings for auto-save-on-dawn. Init from the (possibly
    // loaded) clock so a loaded save mid-day doesn't immediately re-save.
    let mut last_dawn_idx = dawns_elapsed(world.clock_seconds);

    // Tap-Y command menu state. None = closed; Some(i) = open, with row i
    // selected. Phase 15 adds the hold-Y radial overlay alongside this.
    let mut command_menu: Option<usize> = None;

    #[cfg(not(target_arch = "arm"))]
    let debug = debug_console::DebugConsole::spawn();

    let palette = Palette::default();

    // B-style per-cell diff renderer. `prev_cells` mirrors what we last painted
    // into `framebuf`; each frame we recompute the visible cells and only blit
    // the ones that differ. None entries force a paint on the first frame.
    let viewport_cells = (WORLD_W * WORLD_H) as usize;
    let mut prev_cells: Vec<Option<Cell>> = vec![None; viewport_cells];
    let _ = framebuf.fill_rect(None, palette.letterbox);

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

        for input_action in input.drain() {
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
                                let outcome = action::execute(&mut world, id);
                                match outcome {
                                    action::ExecuteOutcome::Done(msg) => {
                                        log_info!("[menu] {}", msg)
                                    }
                                    action::ExecuteOutcome::NotImplemented => {
                                        log_info!(
                                            "[menu] {} not yet implemented",
                                            action::ALL_ACTIONS[selected].name
                                        );
                                    }
                                }
                                command_menu = None;
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
                    Action::B | Action::Y => {
                        command_menu = None;
                    }
                    Action::Start => break 'main,
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
                    let picked = world.try_pickup_all_at_player();
                    if picked > 0 {
                        log_debug!("picked up {} stack(s)", picked);
                    }
                }
                Action::Y => {
                    command_menu = Some(0);
                }
                Action::Start => break 'main,
                Action::Select => save_game(
                    &save_dir,
                    &mut meta,
                    &world,
                    &mut prev_meta_header,
                    &mut prev_run_header,
                ),
                _ => {}
            }
        }

        #[cfg(not(target_arch = "arm"))]
        debug.drain(|cmd| debug_console::apply_debug_command(&mut world, cmd));

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
            save_game(
                &save_dir,
                &mut meta,
                &world,
                &mut prev_meta_header,
                &mut prev_run_header,
            );
        }
        last_dawn_idx = now_dawn_idx;

        // Camera in world coords. While the world fits the viewport we anchor
        // at (0, 0); when the world grows beyond the viewport, switch this to
        //   let cam_x = player.x as i64 - WORLD_W as i64 / 2;
        //   let cam_y = player.y as i64 - WORLD_H as i64 / 2;
        // and the renderer below stays unchanged.
        let cam_x: i64 = 0;
        let cam_y: i64 = 0;
        let player = world.player_pos();
        let pwx = player.x as i64;
        let pwy = player.y as i64;
        let needs = world.player_needs();
        let (clock_h, clock_m) = world.clock_hm();
        let day = world.day_count();
        let is_night = world.is_night();
        let tint = brightness_at(world.clock_seconds);
        let mut ui_cells = build_ui_cells(&palette, needs, day, clock_h, clock_m, is_night);
        if let Some(selected) = command_menu {
            draw_command_menu(&mut ui_cells, &world, selected, &palette);
        }

        let draw_start = Instant::now();
        let mut changed_cells = 0;
        let mut dirty_min_x = WORLD_W as i32;
        let mut dirty_min_y = WORLD_H as i32;
        let mut dirty_max_x = 0;
        let mut dirty_max_y = 0;
        for vy in 0..WORLD_H as i32 {
            for vx in 0..WORLD_W as i32 {
                let wx = cam_x + vx as i64;
                let wy = cam_y + vy as i64;
                // Base terrain glyph.
                let (mut glyph, mut fg, bg) = match world.tile_at(wx, wy) {
                    TerrainKind::Floor => (b'.', palette.floor_fg, palette.floor_bg),
                    TerrainKind::Wall => (b'#', palette.wall_fg, palette.wall_bg),
                };
                let cell_state = world.cell_at(wx, wy);
                let visible = cell_state.map(|c| c.visible).unwrap_or(false);
                let explored = cell_state.map(|c| c.explored).unwrap_or(false);

                // Items + player only render when the cell is currently
                // visible. Memory of explored-but-unseen cells shows
                // terrain only.
                if visible {
                    if let Some(top) = cell_state.and_then(|c| c.items.last()) {
                        let (g, [r, gn, b]) = top.kind.glyph_color();
                        glyph = g;
                        fg = Color::RGB(r, gn, b);
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
                let fg = tint_color(fg, cell_brightness);
                let bg = tint_color(bg, cell_brightness);
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
        if changed_cells > 0 {
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
const HUD_GLYPH_THIRST: u8 = 0xF7; // ≈ wavy water
const HUD_GLYPH_HUNGER: u8 = b'%'; // matches the ration ground-item glyph
const HUD_GLYPH_SLEEP: u8 = b'z'; // classic Z's
const HUD_GLYPH_WARMTH: u8 = 0x0F; // ☼ sun / fire

fn build_ui_cells(
    palette: &Palette,
    needs: Needs,
    day: u64,
    clock_h: u8,
    clock_m: u8,
    is_night: bool,
) -> Vec<Option<Cell>> {
    let mut cells = vec![None; (WORLD_W * WORLD_H) as usize];

    // Left side: "Day N HH:MM day|night".
    let suffix = if is_night { "night" } else { "day" };
    let left = format!("Day {} {:02}:{:02} {}", day, clock_h, clock_m, suffix);
    put_text(&mut cells, 1, 1, &left, palette.hud_fg, palette.hud_bg);

    // Right side: four CP437 need meters, right-aligned. Compose to a
    // Vec<u8> first so the layout shifts cleanly as warmth flips between
    // 100 (3 digits) and < 100 (2 digits).
    let mut bytes: Vec<u8> = Vec::with_capacity(20);
    push_meter(&mut bytes, HUD_GLYPH_THIRST, needs.thirst);
    bytes.push(b' ');
    push_meter(&mut bytes, HUD_GLYPH_HUNGER, needs.hunger);
    bytes.push(b' ');
    push_meter(&mut bytes, HUD_GLYPH_SLEEP, needs.sleep);
    bytes.push(b' ');
    push_meter(&mut bytes, HUD_GLYPH_WARMTH, needs.warmth);
    let critical = needs
        .thirst
        .min(needs.hunger)
        .min(needs.sleep)
        .min(needs.warmth)
        < 25;
    let fg = if critical {
        palette.need_critical_fg
    } else {
        palette.hud_fg
    };
    let x = WORLD_W as i32 - bytes.len() as i32 - 1;
    put_bytes(&mut cells, x, 1, &bytes, fg, palette.hud_bg);

    cells
}

fn push_meter(out: &mut Vec<u8>, glyph: u8, value: u8) {
    out.push(glyph);
    out.extend_from_slice(value.to_string().as_bytes());
}

fn draw_command_menu(
    cells: &mut [Option<Cell>],
    world: &World,
    selected: usize,
    palette: &Palette,
) {
    let x = 2;
    let y = 4;
    let w = 36;
    let h = 21;
    draw_panel(cells, x, y, w, h, palette.panel_fg, palette.panel_bg);
    put_text(
        cells,
        x + 2,
        y + 1,
        "Actions",
        palette.panel_title_fg,
        palette.panel_bg,
    );

    let inner_right = x + w - 2;

    for (i, ca) in action::ALL_ACTIONS.iter().enumerate() {
        let row_y = y + 3 + i as i32;
        let is_selected = i == selected;

        // Cursor + name. Available actions render with the panel
        // foreground; unavailable ones with the dim hud color so the
        // greyed-out state reads at a glance.
        let avail = action::evaluate(world, ca.id);
        let available = matches!(avail, action::Availability::Available { .. });
        let name_fg = match (is_selected, available) {
            (true, true) => palette.panel_fg,
            (true, false) => palette.need_critical_fg,
            (false, true) => palette.panel_fg,
            (false, false) => palette.panel_dim_fg,
        };
        let cursor = if is_selected { b'>' } else { b' ' };
        put_cell(
            cells,
            x + 2,
            row_y,
            Cell {
                glyph: cursor,
                fg: palette.panel_title_fg,
                bg: palette.panel_bg,
            },
        );
        put_text(cells, x + 4, row_y, ca.name, name_fg, palette.panel_bg);

        // Right-aligned status: cost like "3s" or the unavailable reason.
        let (status, status_fg) = match avail {
            action::Availability::Available { cost_game_seconds } => {
                (format!("{}s", cost_game_seconds), palette.panel_fg)
            }
            action::Availability::Unavailable { reason } => {
                (reason.to_string(), palette.panel_dim_fg)
            }
        };
        // Truncate status to fit the inner width.
        let max_status_len = (w - 4 - ca.name.len() as i32 - 2).max(4) as usize;
        let status: String = status.chars().take(max_status_len).collect();
        let status_x = inner_right - status.len() as i32;
        put_text(cells, status_x, row_y, &status, status_fg, palette.panel_bg);
    }

    // Description line for the selected row (truncated to inner width).
    let desc_y = y + h - 3;
    if let Some(sel) = action::ALL_ACTIONS.get(selected) {
        let max_desc_len = (w - 4) as usize;
        let desc: String = sel.description.chars().take(max_desc_len).collect();
        put_text(cells, x + 2, desc_y, &desc, palette.hud_fg, palette.panel_bg);
    }

    // Footer hint.
    put_text(
        cells,
        x + 2,
        y + h - 2,
        "A: confirm   B/Y: close",
        palette.panel_dim_fg,
        palette.panel_bg,
    );
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

fn put_bytes(cells: &mut [Option<Cell>], x: i32, y: i32, bytes: &[u8], fg: Color, bg: Color) {
    for (i, &b) in bytes.iter().enumerate() {
        put_cell(cells, x + i as i32, y, Cell { glyph: b, fg, bg });
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

struct Palette {
    letterbox: Color,
    player_fg: Color,
    floor_fg: Color,
    floor_bg: Color,
    wall_fg: Color,
    wall_bg: Color,
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
            floor_fg: Color::RGB(70, 60, 45),
            floor_bg: Color::RGB(20, 17, 13),
            wall_fg: Color::RGB(140, 110, 75),
            wall_bg: Color::RGB(35, 28, 20),
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
