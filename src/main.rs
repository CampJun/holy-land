mod input;
mod platform;
mod render;
mod save;
mod world;

use sdl2::event::Event;
use sdl2::pixels::Color;

use input::{Action, Input};
use render::{draw_glyph, load_atlas, CELL_SIZE};
use save::{MetaSave, RunSave, SaveHeader};
use world::{Position, Tile, World};

const WORLD_W: u32 = 40;
const WORLD_H: u32 = 30;
const ATLAS_PNG: &[u8] = include_bytes!("../assets/cp437_16x16.png");
const META_FILE: &str = "meta.cbor";
const RUN_FILE: &str = "run.cbor";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let save_dir = platform::save_dir();
    eprintln!("save dir: {}", save_dir.display());

    let meta = match save::load_meta(&save_dir.join(META_FILE)) {
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
        .window("Holy Land", logical_w, logical_h)
        .position_centered()
        .resizable()
        .build()?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()?;
    canvas.set_logical_size(logical_w, logical_h)?;

    let texture_creator = canvas.texture_creator();
    let mut atlas = load_atlas(&texture_creator, ATLAS_PNG)?;

    let mut events = sdl.event_pump()?;
    let mut input = Input::new();
    let mut world = World::new(WORLD_W, WORLD_H);

    if let Ok(run) = save::load_run(&save_dir.join(RUN_FILE)) {
        eprintln!(
            "loaded run save (player at {},{})",
            run.player_x, run.player_y
        );
        world.set_player_pos(Position {
            x: run.player_x,
            y: run.player_y,
        });
    }

    let palette = Palette::default();
    let mut prev_meta_header = meta.header.clone();
    let mut prev_run_header: Option<SaveHeader> = None;

    'main: loop {
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
                Action::Select => {
                    let new_meta_header = SaveHeader::fresh(Some(&prev_meta_header));
                    let mut next_meta = MetaSave::empty(new_meta_header.clone());
                    next_meta.xp = meta.xp;
                    next_meta.demon_currency = meta.demon_currency;
                    next_meta.deity_affinity = meta.deity_affinity.clone();
                    next_meta.unlocks = meta.unlocks.clone();
                    if let Err(e) = save::save_atomic(&save_dir.join(META_FILE), &next_meta) {
                        eprintln!("meta save failed: {}", e);
                    } else {
                        prev_meta_header = new_meta_header;
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
                        prev_run_header = Some(new_run_header);
                        eprintln!(
                            "run saved at ({}, {})",
                            run.player_x, run.player_y
                        );
                    }
                }
                _ => {}
            }
        }

        canvas.set_draw_color(palette.letterbox);
        canvas.clear();

        for y in 0..world.height as i32 {
            for x in 0..world.width as i32 {
                let (g, fg, bg) = match world.tile(x, y) {
                    Tile::Floor => (b'.', palette.floor_fg, palette.floor_bg),
                    Tile::Wall => (b'#', palette.wall_fg, palette.wall_bg),
                };
                draw_glyph(&mut canvas, &mut atlas, x, y, g, fg, bg);
            }
        }

        let pos = world.player_pos();
        draw_glyph(
            &mut canvas,
            &mut atlas,
            pos.x,
            pos.y,
            b'@',
            palette.player_fg,
            palette.floor_bg,
        );

        canvas.present();
    }

    Ok(())
}

struct Palette {
    letterbox: Color,
    player_fg: Color,
    floor_fg: Color,
    floor_bg: Color,
    wall_fg: Color,
    wall_bg: Color,
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
        }
    }
}
