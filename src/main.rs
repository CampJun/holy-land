mod input;
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
use render::{draw_glyph, load_atlas, CELL_SIZE};
use save::{MetaSave, RunSave, SaveHeader};
use world::{Position, Tile, World};

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
    eprintln!("save dir: {}", save_dir.display());

    let mut meta = match save::load_meta(&save_dir.join(META_FILE)) {
        Ok(m) => {
            eprintln!(
                "loaded meta save (counter={}, device={})",
                m.header.save_counter, m.header.device_id
            );
            m
        }
        Err(e) => {
            eprintln!("no meta save loaded ({}); starting fresh", e);
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
        eprintln!(
            "renderer: {} (flags={:#x}) max_texture={}x{}",
            info.name, info.flags, info.max_texture_width, info.max_texture_height
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
        eprintln!(
            "loaded run save (player at {},{})",
            run.player_x, run.player_y
        );
        prev_run_header = Some(run.header.clone());
        world.set_player_pos(Position {
            x: run.player_x,
            y: run.player_y,
        });
    }

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

        for action in input.drain() {
            match action {
                Action::Up => world.try_move_player(0, -1),
                Action::Down => world.try_move_player(0, 1),
                Action::Left => world.try_move_player(-1, 0),
                Action::Right => world.try_move_player(1, 0),
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
        let ui_cells = build_ui_cells(&palette);

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
                let (mut glyph, mut fg, bg) = match world.tile_at(wx, wy) {
                    Tile::Floor => (b'.', palette.floor_fg, palette.floor_bg),
                    Tile::Wall => (b'#', palette.wall_fg, palette.wall_bg),
                };
                if wx == pwx && wy == pwy {
                    glyph = b'@';
                    fg = palette.player_fg;
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
            eprintln!("fps: {}", fps_count);
            eprintln!("{}", timing_accum.summary());
            fps_count = 0;
            fps_window = Instant::now();
            timing_accum = FrameTiming::default();
        }
    }

    eprintln!("shutdown: exiting Survival main loop");
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
        eprintln!("meta save failed: {}", e);
    } else {
        *meta = next_meta;
        *prev_meta_header = new_meta_header;
        eprintln!("meta saved (counter={})", prev_meta_header.save_counter);
    }

    let pos = world.player_pos();
    let new_run_header = SaveHeader::fresh(prev_run_header.as_ref());
    let mut run = RunSave::empty(new_run_header.clone());
    run.player_x = pos.x;
    run.player_y = pos.y;
    if let Err(e) = save::save_atomic(&save_dir.join(RUN_FILE), &run) {
        eprintln!("run save failed: {}", e);
    } else {
        *prev_run_header = Some(new_run_header);
        eprintln!("run saved at ({}, {})", run.player_x, run.player_y);
    }
}

fn build_ui_cells(palette: &Palette) -> Vec<Option<Cell>> {
    let mut cells = vec![None; (WORLD_W * WORLD_H) as usize];
    put_text(&mut cells, 1, 1, "Survival", palette.hud_fg, palette.hud_bg);
    cells
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
        }
    }
}
