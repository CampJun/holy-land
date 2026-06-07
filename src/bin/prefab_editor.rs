//! Mouse-driven desktop editor for building prefabs (`assets/buildings/*.ron`).
//!
//! Two layers per cell, rendered through the GAME'S OWN `render::draw_sprite`
//! for pixel-identical previews:
//!   - TERRAIN: a sprite-tag (see `sprite_tags`) — any sheet cell carrying a
//!     passability flag (wall = blocked, floor = passable). Painted from the
//!     swatch row of catalog terrain tags.
//!   - ROOF: a `Structures` facade slice drawn on top (the 2.5D look).
//!
//! LABEL mode lets you tag any sheet cell (name + Wall/Floor) into
//! `assets/sprite_tags.ron`, which both the editor and game read.
//!
//! Launch with no args for a picker list; or a stem to open; or `new <stem> <w> <h>`.

use std::path::PathBuf;

use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::surface::Surface;
use sdl2::video::{Window, WindowContext};

use holyland::buildings::{CityTier, Prefab};
use holyland::render::{draw_sprite, draw_sprite_layered, load_atlas};
use holyland::sprite_tags::{Role, Tag};
use holyland::sprites::{Sheet, Sprite, SpriteSheets};
use holyland::world::TerrainKind;

// ---- layout ---------------------------------------------------------------
const WIN_W: u32 = 1280;
const WIN_H: u32 = 800;
const TOOLBAR_H: i32 = 56;

const PAL_X: i32 = 8;
const PAL_Y: i32 = TOOLBAR_H + 28;
const PAL_ZOOM: i32 = 16;
const PAL_VW: i32 = 30;
const PAL_VH: i32 = 38;
const PAL_W: i32 = PAL_VW * PAL_ZOOM;
const PAL_H: i32 = PAL_VH * PAL_ZOOM;

const GRID_X: i32 = PAL_X + PAL_W + 24;
const GRID_Y: i32 = TOOLBAR_H + 28;
const GRID_CELL: i32 = 40;

const GLYPH_W: i32 = 11;
const GLYPH_H: i32 = 18;

const WHITE: Color = Color::RGB(255, 255, 255);
const SEL: Color = Color::RGB(250, 220, 70);
const FRAME: Color = Color::RGB(80, 80, 90);

// ---- model ----------------------------------------------------------------
struct Model {
    stem: String,
    tag: String,
    tier: CityTier,
    variant: String,
    weight: u32,
    w: u16,
    h: u16,
    terrain: Vec<Option<String>>, // sprite-tag name (None = empty cell)
    roof: Vec<Option<(u8, u8)>>,  // Structures (col,row) (None = no roof)
    objects: Vec<Option<String>>, // object-tag name (None = no object)
}

impl Model {
    fn blank(stem: String, w: u16, h: u16) -> Self {
        let n = w as usize * h as usize;
        Model {
            variant: stem.clone(),
            stem,
            tag: "house".into(),
            tier: CityTier::Urban,
            weight: 3,
            w,
            h,
            terrain: vec![None; n],
            roof: vec![None; n],
            objects: vec![None; n],
        }
    }

    fn from_prefab(stem: String, p: &Prefab) -> Self {
        let (w, h) = p.size;
        let mut m = Model::blank(stem, w, h);
        m.tag = p.tag.clone();
        m.tier = p.tier;
        m.variant = p.variant.clone();
        m.weight = p.weight;
        for y in 0..h {
            for x in 0..w {
                let i = m.idx(x, y);
                // Prefer the tag palette; fall back to mapping a legacy
                // TerrainKind to a seed tag so old prefabs open sensibly.
                m.terrain[i] = p
                    .tag_palette_name(x, y)
                    .map(str::to_string)
                    .or_else(|| p.cell_at(x, y).and_then(legacy_tag));
                m.roof[i] = p.roof_at(x, y);
                m.objects[i] = p.objects_palette_name(x, y).map(str::to_string);
            }
        }
        m
    }

    fn idx(&self, x: u16, y: u16) -> usize {
        y as usize * self.w as usize + x as usize
    }

    fn resize(&mut self, nw: u16, nh: u16) {
        let nw = nw.clamp(1, 40);
        let nh = nh.clamp(1, 40);
        let mut nt = vec![None; nw as usize * nh as usize];
        let mut nr = vec![None; nw as usize * nh as usize];
        let mut no = vec![None; nw as usize * nh as usize];
        for y in 0..nh.min(self.h) {
            for x in 0..nw.min(self.w) {
                let src = self.idx(x, y);
                let dst = y as usize * nw as usize + x as usize;
                nt[dst] = self.terrain[src].clone();
                nr[dst] = self.roof[src];
                no[dst] = self.objects[src].clone();
            }
        }
        self.w = nw;
        self.h = nh;
        self.terrain = nt;
        self.roof = nr;
        self.objects = no;
    }

    /// Emit RON: a tag-name terrain palette + grid, plus the roof layer.
    fn to_ron(&self) -> String {
        const POOL: &[u8] = b"#.=o,:~+*ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut tpal: Vec<(char, String)> = Vec::new();
        for name in self.terrain.iter().flatten() {
            if !tpal.iter().any(|(_, n)| n == name) {
                let ch = POOL[tpal.len().min(POOL.len() - 1)] as char;
                tpal.push((ch, name.clone()));
            }
        }
        let mut rpal: Vec<(char, (u8, u8))> = Vec::new();
        for r in self.roof.iter().flatten() {
            if !rpal.iter().any(|(_, v)| v == r) {
                let ch = POOL[rpal.len().min(POOL.len() - 1)] as char;
                rpal.push((ch, *r));
            }
        }

        let mut s = String::new();
        s.push_str(&format!(
            "// Edited with the prefab-editor tool.\nPrefab(\n    tag: {:?},\n    tier: {},\n    variant: {:?},\n    weight: {},\n    size: ({}, {}),\n    palette: {{}},\n",
            self.tag, tier_name(self.tier), self.variant, self.weight, self.w, self.h
        ));
        s.push_str("    tag_palette: {\n");
        for (ch, name) in &tpal {
            s.push_str(&format!("        {:?}: {:?},\n", ch, name));
        }
        s.push_str("    },\n    grid: [\n");
        for y in 0..self.h {
            let mut row = String::new();
            for x in 0..self.w {
                let c = self.terrain[self.idx(x, y)]
                    .as_ref()
                    .map(|n| tpal.iter().find(|(_, nm)| nm == n).unwrap().0)
                    .unwrap_or(' ');
                row.push(c);
            }
            s.push_str(&format!("        {:?},\n", row));
        }
        s.push_str("    ],\n");
        if !rpal.is_empty() {
            s.push_str("    roof_palette: {\n");
            for (ch, (c, r)) in &rpal {
                s.push_str(&format!("        {:?}: ({}, {}),\n", ch, c, r));
            }
            s.push_str("    },\n    roof: [\n");
            for y in 0..self.h {
                let mut row = String::new();
                for x in 0..self.w {
                    let c = self.roof[self.idx(x, y)]
                        .map(|v| rpal.iter().find(|(_, w)| *w == v).unwrap().0)
                        .unwrap_or(' ');
                    row.push(c);
                }
                s.push_str(&format!("        {:?},\n", row));
            }
            s.push_str("    ],\n");
        }
        // Object layer.
        let mut opal: Vec<(char, String)> = Vec::new();
        for name in self.objects.iter().flatten() {
            if !opal.iter().any(|(_, n)| n == name) {
                let ch = POOL[opal.len().min(POOL.len() - 1)] as char;
                opal.push((ch, name.clone()));
            }
        }
        if !opal.is_empty() {
            s.push_str("    objects_palette: {\n");
            for (ch, name) in &opal {
                s.push_str(&format!("        {:?}: {:?},\n", ch, name));
            }
            s.push_str("    },\n    objects: [\n");
            for y in 0..self.h {
                let mut row = String::new();
                for x in 0..self.w {
                    let c = self.objects[self.idx(x, y)]
                        .as_ref()
                        .map(|n| opal.iter().find(|(_, nm)| nm == n).unwrap().0)
                        .unwrap_or(' ');
                    row.push(c);
                }
                s.push_str(&format!("        {:?},\n", row));
            }
            s.push_str("    ],\n");
        }
        s.push_str(")\n");
        s
    }
}

/// Best-effort map from a legacy `TerrainKind` to a seed catalog tag name, so
/// pre-tag prefabs still open in the editor.
fn legacy_tag(t: TerrainKind) -> Option<String> {
    let n = match t {
        TerrainKind::WoodWall => "wood_wall",
        TerrainKind::StoneWall | TerrainKind::Solid | TerrainKind::Wall => "stone_wall",
        TerrainKind::Floor | TerrainKind::CobbleRoad => "cobble_floor",
        TerrainKind::BareDirt => "dirt_floor",
        TerrainKind::Grass => "grass_floor",
        _ => return None,
    };
    Some(n.to_string())
}

fn tier_name(t: CityTier) -> &'static str {
    match t {
        CityTier::Urban => "Urban",
        CityTier::Town => "Town",
        CityTier::Village => "Village",
        CityTier::Hamlet => "Hamlet",
    }
}

fn cycle_tier(t: CityTier) -> CityTier {
    match t {
        CityTier::Urban => CityTier::Town,
        CityTier::Town => CityTier::Village,
        CityTier::Village => CityTier::Hamlet,
        CityTier::Hamlet => CityTier::Urban,
    }
}

fn buildings_dir() -> PathBuf {
    PathBuf::from("assets/buildings")
}
fn catalog_path() -> PathBuf {
    PathBuf::from("assets/sprite_tags.ron")
}

fn load_tags() -> Vec<Tag> {
    std::fs::read_to_string(catalog_path())
        .ok()
        .and_then(|s| ron::from_str::<Vec<Tag>>(&s).ok())
        .unwrap_or_default()
}

/// Write the catalog back (the editor owns the live copy; the game reloads it
/// on its next build via the embedded include_str!).
fn write_tags(tags: &[Tag]) -> std::io::Result<()> {
    let cfg = ron::ser::PrettyConfig::new().compact_arrays(false);
    let body = ron::ser::to_string_pretty(&tags.to_vec(), cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let text = format!("// Sprite-tag catalog. Managed by the prefab-editor Label mode.\n{body}\n");
    let path = catalog_path();
    let tmp = path.with_extension("ron.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)
}

fn list_prefabs() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if let Ok(read) = std::fs::read_dir(buildings_dir()) {
        for e in read.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "ron").unwrap_or(false) {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    v.push(stem.to_string());
                }
            }
        }
    }
    v.sort();
    v
}

fn load_model(stem: &str) -> Result<Model, String> {
    let path = buildings_dir().join(format!("{stem}.ron"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let p: Prefab = ron::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    Ok(Model::from_prefab(stem.to_string(), &p))
}

fn save_ron(stem: &str, contents: &str) -> std::io::Result<PathBuf> {
    let path = buildings_dir().join(format!("{stem}.ron"));
    let tmp = path.with_extension("ron.tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

fn keycode_to_char(k: Keycode) -> Option<char> {
    let n = k.name();
    if n.len() == 1 {
        let c = n.chars().next().unwrap();
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_lowercase());
        }
    }
    matches!(k, Keycode::Underscore | Keycode::Minus).then_some('_')
}

// ---- editor state ---------------------------------------------------------
#[derive(Clone, Copy, PartialEq)]
enum Layer {
    Terrain,
    Roof,
    Object,
}

impl Layer {
    fn next(self) -> Layer {
        match self {
            Layer::Terrain => Layer::Roof,
            Layer::Roof => Layer::Object,
            Layer::Object => Layer::Terrain,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Layer::Terrain => "TERRAIN",
            Layer::Roof => "ROOF",
            Layer::Object => "OBJECT",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Browse,
    NewName,
    Edit,
}

struct State {
    active: Layer,
    terrain_brush: Option<String>,
    object_brush: Option<String>,
    roof_brush: Option<(u8, u8)>,
    reveal: bool,
    sheet: usize, // index into Sheet::ALL for the palette
    scroll_col: i32,
    scroll_row: i32,
    tags: Vec<Tag>,
    labeling: bool,
    label_target: Option<(String, u8, u8)>, // (sheet name, col, row)
    label_name: String,
    status: String,
}

impl State {
    fn terrain_tags(&self) -> Vec<&Tag> {
        self.tags.iter().filter(|t| t.is_terrain()).collect()
    }
    fn object_tags(&self) -> Vec<&Tag> {
        self.tags.iter().filter(|t| !t.is_terrain()).collect()
    }
    /// The swatch tags for the active layer (terrain tags, or object tags).
    fn swatch_tags(&self) -> Vec<&Tag> {
        match self.active {
            Layer::Object => self.object_tags(),
            _ => self.terrain_tags(),
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let (mut mode, mut model): (Mode, Option<Model>) = match args.get(1).map(|s| s.as_str()) {
        Some("new") => {
            let stem = args.get(2).cloned().unwrap_or_else(|| "untitled".into());
            let w: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(6);
            let h: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(6);
            (Mode::Edit, Some(Model::blank(stem, w, h)))
        }
        Some(stem) => (Mode::Edit, Some(load_model(stem)?)),
        None => (Mode::Browse, None),
    };

    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "0");
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let window = video
        .window("prefab-editor", WIN_W, WIN_H)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas: Canvas<Window> = window.into_canvas().accelerated().build().map_err(|e| e.to_string())?;
    let tc = canvas.texture_creator();
    let mut events = sdl.event_pump()?;

    let mut sheets = SpriteSheets::new();
    let mut font = {
        let surf = load_atlas(include_bytes!("../../assets/cp437_16x16.png"))?;
        tc.create_texture_from_surface(&surf).map_err(|e| e.to_string())?
    };

    let mut st = State {
        active: Layer::Terrain,
        terrain_brush: None,
        object_brush: None,
        roof_brush: Some((6, 2)),
        reveal: false,
        sheet: Sheet::ALL.iter().position(|s| *s == Sheet::Structures).unwrap(),
        scroll_col: 0,
        scroll_row: 0,
        tags: load_tags(),
        labeling: false,
        label_target: None,
        label_name: String::new(),
        status: "pick a prefab or [+ new]".into(),
    };
    st.terrain_brush = st.terrain_tags().first().map(|t| t.name.clone());
    st.object_brush = st.object_tags().first().map(|t| t.name.clone());
    let mut files = list_prefabs();
    let mut namebuf = String::new();
    let mut mouse = (0i32, 0i32);

    'main: loop {
        for ev in events.poll_iter() {
            if let Event::MouseMotion { x, y, mousestate, .. } = ev {
                mouse = (x, y);
                if mode == Mode::Edit && !st.labeling {
                    if mousestate.left() {
                        paint_left(x, y, &st, model.as_mut().unwrap());
                    } else if mousestate.right() {
                        erase(x, y, st.active, model.as_mut().unwrap());
                    }
                }
            }
            match mode {
                Mode::Browse => match ev {
                    Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'main,
                    Event::MouseButtonDown { x, y, .. } => {
                        if let Some(row) = browse_row_at(x, y, files.len()) {
                            if row == 0 {
                                namebuf.clear();
                                mode = Mode::NewName;
                            } else if let Ok(m) = load_model(&files[row - 1]).map_err(|e| st.status = e) {
                                st.status = format!("editing {}", m.stem);
                                model = Some(m);
                                mode = Mode::Edit;
                            }
                        }
                    }
                    _ => {}
                },
                Mode::NewName => {
                    if let Event::KeyDown { keycode: Some(k), .. } = ev {
                        match k {
                            Keycode::Escape => mode = Mode::Browse,
                            Keycode::Return | Keycode::Return2 | Keycode::KpEnter => {
                                if !namebuf.is_empty() {
                                    model = Some(Model::blank(namebuf.clone(), 6, 6));
                                    st.status = format!("new prefab {namebuf} (6x6)");
                                    mode = Mode::Edit;
                                }
                            }
                            Keycode::Backspace => {
                                namebuf.pop();
                            }
                            other => {
                                if let Some(c) = keycode_to_char(other) {
                                    namebuf.push(c);
                                }
                            }
                        }
                    }
                }
                Mode::Edit => {
                    if let Event::Quit { .. } = ev {
                        break 'main;
                    }
                    let m = model.as_mut().unwrap();
                    match ev {
                        Event::KeyDown { keycode: Some(k), keymod, .. } => {
                            // While typing a label name, letters go to the name.
                            if st.labeling && st.label_target.is_some() {
                                match k {
                                    Keycode::Escape => { st.label_target = None; }
                                    Keycode::Backspace => { st.label_name.pop(); }
                                    other => if let Some(c) = keycode_to_char(other) { st.label_name.push(c); }
                                }
                                continue;
                            }
                            let shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
                            let ctrl = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);
                            match k {
                                Keycode::Escape => break 'main,
                                Keycode::L => { st.labeling = !st.labeling; st.label_target = None;
                                    st.status = if st.labeling { "LABEL: pick a sheet cell".into() } else { "label off".into() }; }
                                Keycode::O => {
                                    files = list_prefabs();
                                    st.status = "pick a prefab or [+ new]".into();
                                    mode = Mode::Browse;
                                }
                                Keycode::Tab => st.active = st.active.next(),
                                Keycode::R => st.reveal = !st.reveal,
                                Keycode::S if ctrl => {
                                    st.status = match save_ron(&m.stem, &m.to_ron()) {
                                        Ok(p) => format!("saved {} (rebuild game to embed)", p.display()),
                                        Err(e) => format!("SAVE FAILED: {e}"),
                                    };
                                }
                                Keycode::T => { m.tier = cycle_tier(m.tier); st.status = format!("tier -> {}", tier_name(m.tier)); }
                                Keycode::G => { m.tag = if m.tag == "house" { "smithy".into() } else { "house".into() }; st.status = format!("tag -> {}", m.tag); }
                                Keycode::Comma => st.sheet = (st.sheet + Sheet::COUNT - 1) % Sheet::COUNT,
                                Keycode::Period => st.sheet = (st.sheet + 1) % Sheet::COUNT,
                                Keycode::LeftBracket => m.weight = m.weight.saturating_sub(1),
                                Keycode::RightBracket => m.weight += 1,
                                Keycode::Right if shift => m.resize(m.w + 1, m.h),
                                Keycode::Left if shift => m.resize(m.w.saturating_sub(1), m.h),
                                Keycode::Down if shift => m.resize(m.w, m.h + 1),
                                Keycode::Up if shift => m.resize(m.w, m.h.saturating_sub(1)),
                                _ => {}
                            }
                        }
                        Event::MouseWheel { x, y, .. } => {
                            let (cols, rows) = Sheet::ALL[st.sheet].grid();
                            st.scroll_row = (st.scroll_row - y).clamp(0, (rows as i32 - PAL_VH).max(0));
                            st.scroll_col = (st.scroll_col + x).clamp(0, (cols as i32 - PAL_VW).max(0));
                        }
                        Event::MouseButtonDown { mouse_btn, x, y, .. } => {
                            handle_edit_click(mouse_btn, x, y, m, &mut st, &mut mode, &mut files);
                        }
                        _ => {}
                    }
                }
            }
        }

        match mode {
            Mode::Browse => render_browse(&mut canvas, &mut font, &files, mouse, &st.status)?,
            Mode::NewName => render_newname(&mut canvas, &mut font, &namebuf)?,
            Mode::Edit => render_edit(&mut canvas, &tc, &mut sheets, &mut font, model.as_ref().unwrap(), &st, mouse)?,
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    Ok(())
}

// ---- painting -------------------------------------------------------------
fn cell_at_xy(x: i32, y: i32, m: &Model) -> Option<(u16, u16)> {
    let (gx, gy) = (x - GRID_X, y - GRID_Y);
    if gx < 0 || gy < 0 {
        return None;
    }
    let (cx, cy) = ((gx / GRID_CELL) as u16, (gy / GRID_CELL) as u16);
    (cx < m.w && cy < m.h).then_some((cx, cy))
}

fn paint_left(x: i32, y: i32, st: &State, m: &mut Model) {
    if let Some((cx, cy)) = cell_at_xy(x, y, m) {
        let i = m.idx(cx, cy);
        match st.active {
            Layer::Terrain => m.terrain[i] = st.terrain_brush.clone(),
            Layer::Roof => m.roof[i] = st.roof_brush,
            Layer::Object => m.objects[i] = st.object_brush.clone(),
        }
    }
}

fn erase(x: i32, y: i32, active: Layer, m: &mut Model) {
    if let Some((cx, cy)) = cell_at_xy(x, y, m) {
        let i = m.idx(cx, cy);
        match active {
            Layer::Terrain => m.terrain[i] = None,
            Layer::Roof => m.roof[i] = None,
            Layer::Object => m.objects[i] = None,
        }
    }
}

fn handle_edit_click(btn: MouseButton, x: i32, y: i32, m: &mut Model, st: &mut State, mode: &mut Mode, files: &mut Vec<String>) {
    // Label role buttons (only while a label target is selected).
    if st.labeling {
        if let Some((sheet, col, row)) = st.label_target.clone() {
            let roles = [
                ("Wall", Role::Terrain { passable: false, blocks_sight: true }),
                ("Floor", Role::Terrain { passable: true, blocks_sight: false }),
                ("Light", Role::Light),
                ("Decor", Role::Decoration),
                ("Statn", Role::Station { skill: "metallurgy".into() }),
            ];
            for (i, (lbl, role)) in roles.into_iter().enumerate() {
                let r = label_role_button(i as i32);
                if rect_hit(x, y, r.x(), r.y(), r.width() as i32, r.height() as i32) && !st.label_name.is_empty() {
                    let is_terrain = matches!(role, Role::Terrain { .. });
                    st.tags.retain(|t| t.name != st.label_name);
                    st.tags.push(Tag { name: st.label_name.clone(), sheet, col, row, role });
                    st.status = match write_tags(&st.tags) {
                        Ok(_) => format!("labeled {:?} as {}", st.label_name, lbl),
                        Err(e) => format!("catalog write failed: {e}"),
                    };
                    if is_terrain {
                        st.terrain_brush = Some(st.label_name.clone());
                        st.active = Layer::Terrain;
                    } else {
                        st.object_brush = Some(st.label_name.clone());
                        st.active = Layer::Object;
                    }
                    st.labeling = false;
                    st.label_target = None;
                    return;
                }
            }
        }
    }
    // Swatches (terrain or object tags, per active layer).
    let swatches: Vec<String> = st.swatch_tags().iter().map(|t| t.name.clone()).collect();
    for (i, name) in swatches.iter().enumerate() {
        let sx = 8 + i as i32 * 44;
        if rect_hit(x, y, sx, 8, 40, 40) {
            if st.active == Layer::Object {
                st.object_brush = Some(name.clone());
            } else {
                st.terrain_brush = Some(name.clone());
                st.active = Layer::Terrain;
            }
            st.labeling = false;
            st.status = format!("brush: {name}");
            return;
        }
    }
    // Erase swatch (clears the active layer's brush).
    let ex = 8 + swatches.len() as i32 * 44;
    if rect_hit(x, y, ex, 8, 40, 40) {
        if st.active == Layer::Object {
            st.object_brush = None;
        } else {
            st.terrain_brush = None;
            st.active = Layer::Terrain;
        }
        st.status = "brush: (erase)".into();
        return;
    }
    // Resize buttons + sheet switch + open.
    for (r, dw, dh) in resize_buttons() {
        if rect_hit(x, y, r.x(), r.y(), r.width() as i32, r.height() as i32) {
            m.resize((m.w as i32 + dw).max(1) as u16, (m.h as i32 + dh).max(1) as u16);
            st.status = format!("size {}x{}", m.w, m.h);
            return;
        }
    }
    let (sl, sr) = sheet_buttons();
    if rect_hit(x, y, sl.x(), sl.y(), sl.width() as i32, sl.height() as i32) { st.sheet = (st.sheet + Sheet::COUNT - 1) % Sheet::COUNT; st.scroll_col = 0; st.scroll_row = 0; return; }
    if rect_hit(x, y, sr.x(), sr.y(), sr.width() as i32, sr.height() as i32) { st.sheet = (st.sheet + 1) % Sheet::COUNT; st.scroll_col = 0; st.scroll_row = 0; return; }
    let ob = open_button();
    if rect_hit(x, y, ob.x(), ob.y(), ob.width() as i32, ob.height() as i32) {
        *files = list_prefabs();
        st.status = "pick a prefab or [+ new]".into();
        *mode = Mode::Browse;
        return;
    }
    // Palette cell pick.
    if rect_hit(x, y, PAL_X, PAL_Y, PAL_W, PAL_H) {
        let (cols, rows) = Sheet::ALL[st.sheet].grid();
        let col = st.scroll_col + (x - PAL_X) / PAL_ZOOM;
        let row = st.scroll_row + (y - PAL_Y) / PAL_ZOOM;
        if col < 0 || col >= cols as i32 || row < 0 || row >= rows as i32 {
            return;
        }
        if st.labeling {
            st.label_target = Some((Sheet::ALL[st.sheet].name().to_string(), col as u8, row as u8));
            st.label_name.clear();
        } else if st.active == Layer::Roof && Sheet::ALL[st.sheet] == Sheet::Structures {
            st.roof_brush = Some((col as u8, row as u8));
            st.status = format!("roof brush: ({col}, {row})");
        } else if st.active == Layer::Roof {
            st.status = "roof tiles come from the Structures sheet (switch sheet)".into();
        }
        return;
    }
    // Grid paint.
    if btn == MouseButton::Right {
        erase(x, y, st.active, m);
    } else {
        paint_left(x, y, st, m);
    }
}

fn rect_hit(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

// ---- toolbar button rects -------------------------------------------------
fn resize_buttons() -> [(Rect, i32, i32); 4] {
    let x0 = 560;
    let mk = |i: i32| Rect::new(x0 + i * 30, 8, 26, 22);
    [(mk(0), -1, 0), (mk(1), 1, 0), (mk(2), 0, -1), (mk(3), 0, 1)]
}
fn sheet_buttons() -> (Rect, Rect) {
    (Rect::new(8, 34, 22, 20), Rect::new(220, 34, 22, 20))
}
fn open_button() -> Rect {
    Rect::new(560 + 4 * 30 + 12, 8, 64, 22)
}
fn label_role_button(i: i32) -> Rect {
    Rect::new(GRID_X + 200 + i * 78, GRID_Y - 26, 70, 22)
}

// ---- browse / new-name ----------------------------------------------------
const BROWSE_X: i32 = 48;
const BROWSE_Y0: i32 = 120;
const BROWSE_ROW_H: i32 = 30;
const BROWSE_W: i32 = 420;

fn browse_row_at(x: i32, y: i32, n_files: usize) -> Option<usize> {
    if x < BROWSE_X || x > BROWSE_X + BROWSE_W || y < BROWSE_Y0 {
        return None;
    }
    let row = ((y - BROWSE_Y0) / BROWSE_ROW_H) as usize;
    (row <= n_files).then_some(row)
}

fn render_browse(canvas: &mut Canvas<Window>, font: &mut Texture, files: &[String], mouse: (i32, i32), status: &str) -> Result<(), String> {
    canvas.set_draw_color(Color::RGB(24, 24, 30));
    canvas.clear();
    draw_text(canvas, font, BROWSE_X, 64, "Open a prefab", Color::RGB(230, 230, 240));
    draw_text(canvas, font, BROWSE_X, 88, status, Color::RGB(150, 150, 160));
    let hover = browse_row_at(mouse.0, mouse.1, files.len());
    for row in 0..files.len() + 1 {
        let y = BROWSE_Y0 + row as i32 * BROWSE_ROW_H;
        let hot = hover == Some(row);
        if hot {
            fill(canvas, BROWSE_X - 6, y - 4, BROWSE_W, BROWSE_ROW_H, Color::RGB(44, 44, 56));
        }
        let (label, color) = if row == 0 {
            ("[+ new prefab]".to_string(), Color::RGB(150, 230, 150))
        } else {
            (files[row - 1].clone(), Color::RGB(210, 210, 220))
        };
        draw_text(canvas, font, BROWSE_X, y, &label, if hot { WHITE } else { color });
    }
    canvas.present();
    Ok(())
}

fn render_newname(canvas: &mut Canvas<Window>, font: &mut Texture, namebuf: &str) -> Result<(), String> {
    canvas.set_draw_color(Color::RGB(24, 24, 30));
    canvas.clear();
    draw_text(canvas, font, BROWSE_X, 120, "New prefab name:", Color::RGB(230, 230, 240));
    fill(canvas, BROWSE_X, 150, 500, 28, Color::RGB(40, 40, 52));
    draw_text(canvas, font, BROWSE_X + 6, 154, &format!("{namebuf}_"), WHITE);
    draw_text(canvas, font, BROWSE_X, 196, "letters / digits / _   Enter = create 6x6   Esc = cancel", Color::RGB(150, 150, 160));
    canvas.present();
    Ok(())
}

// ---- edit screen ----------------------------------------------------------
fn render_edit(canvas: &mut Canvas<Window>, tc: &TextureCreator<WindowContext>, sheets: &mut SpriteSheets, font: &mut Texture, model: &Model, st: &State, mouse: (i32, i32)) -> Result<(), String> {
    canvas.set_draw_color(Color::RGB(24, 24, 30));
    canvas.clear();

    // Swatch row for the active layer (terrain or object tags), real sprites.
    let active_brush = match st.active {
        Layer::Object => st.object_brush.clone(),
        _ => st.terrain_brush.clone(),
    };
    let tt: Vec<(String, Option<Sprite>)> = st.swatch_tags().iter().map(|t| (t.name.clone(), t.sprite())).collect();
    let mut sw = Surface::new((tt.len().max(1) as u32) * 16, 16, PixelFormatEnum::ARGB8888)?;
    sw.fill_rect(None, Color::RGB(30, 30, 38))?;
    for (i, (_, sp)) in tt.iter().enumerate() {
        if let Some(sp) = sp {
            let surf = sheets.get(sp.sheet)?;
            draw_sprite(&mut sw, surf, i as i32, 0, sp.src_rect(), WHITE, Color::RGB(30, 30, 38));
        }
    }
    let sw_tex = tc.create_texture_from_surface(&sw).map_err(|e| e.to_string())?;
    for (i, (name, _)) in tt.iter().enumerate() {
        let sx = 8 + i as i32 * 44;
        canvas.copy(&sw_tex, Some(Rect::new(i as i32 * 16, 0, 16, 16)), Some(Rect::new(sx, 8, 40, 40)))?;
        let sel = active_brush.as_deref() == Some(name);
        outline(canvas, sx, 8, 40, 40, if sel { SEL } else { FRAME });
    }
    let ex = 8 + tt.len() as i32 * 44;
    fill(canvas, ex, 8, 40, 40, Color::RGB(40, 40, 48));
    outline(canvas, ex, 8, 40, 40, if active_brush.is_none() { SEL } else { FRAME });
    draw_text(canvas, font, ex + 12, 16, "X", WHITE);

    // Resize + open buttons.
    for ((r, _, _), lbl) in resize_buttons().iter().zip(["W-", "W+", "H-", "H+"]) {
        button(canvas, font, *r, lbl, rect_hit(mouse.0, mouse.1, r.x(), r.y(), r.width() as i32, r.height() as i32));
    }
    button(canvas, font, open_button(), "[open]", false);

    // Sheet selector (second toolbar row).
    let (sl, sr) = sheet_buttons();
    button(canvas, font, sl, "<", false);
    button(canvas, font, sr, ">", false);
    draw_text(canvas, font, 36, 36, Sheet::ALL[st.sheet].name(), Color::RGB(200, 200, 210));

    // Info line (top-right).
    let layer = st.active.label();
    let brush = match st.active {
        Layer::Roof => st.roof_brush.map(|(c, r)| format!("roof({c},{r})")).unwrap_or_else(|| "roof(-)".into()),
        Layer::Object => st.object_brush.clone().unwrap_or_else(|| "(erase)".into()),
        Layer::Terrain => st.terrain_brush.clone().unwrap_or_else(|| "(erase)".into()),
    };
    draw_text(canvas, font, 700, 8, &format!("[{layer}] {brush}  reveal:{}", if st.reveal { "ON" } else { "off" }), Color::RGB(210, 210, 220));
    draw_text(canvas, font, 700, 30, &format!("{}  {}x{}  {}/{}", model.stem, model.w, model.h, tier_name(model.tier), model.tag), Color::RGB(160, 160, 170));

    // Palette (current sheet).
    let sheet = Sheet::ALL[st.sheet];
    let pal_tex = { let surf = sheets.get(sheet)?; tc.create_texture_from_surface(&*surf).map_err(|e| e.to_string())? };
    let pal_src = Rect::new(st.scroll_col * 8, st.scroll_row * 8, (PAL_VW * 8) as u32, (PAL_VH * 8) as u32);
    canvas.copy(&pal_tex, Some(pal_src), Some(Rect::new(PAL_X, PAL_Y, PAL_W as u32, PAL_H as u32)))?;
    outline(canvas, PAL_X - 1, PAL_Y - 1, PAL_W + 2, PAL_H + 2, FRAME);
    // Label target highlight / panel.
    if st.labeling {
        if let Some((_, c, r)) = &st.label_target {
            let sx = PAL_X + (*c as i32 - st.scroll_col) * PAL_ZOOM;
            let sy = PAL_Y + (*r as i32 - st.scroll_row) * PAL_ZOOM;
            if rect_hit(sx, sy, PAL_X, PAL_Y, PAL_W, PAL_H) {
                outline(canvas, sx, sy, PAL_ZOOM, PAL_ZOOM, Color::RGB(120, 230, 120));
            }
            draw_text(canvas, font, GRID_X, GRID_Y - 24, &format!("name: {}_", st.label_name), WHITE);
            for (i, lbl) in ["[Wall]", "[Floor]", "[Light]", "[Decor]", "[Statn]"].into_iter().enumerate() {
                button(canvas, font, label_role_button(i as i32), lbl, false);
            }
        } else {
            draw_text(canvas, font, GRID_X, GRID_Y - 24, "LABEL: click a sheet cell, type a name, pick a role", Color::RGB(150, 230, 150));
        }
    } else if st.active == Layer::Roof {
        if let Some((c, r)) = st.roof_brush {
            let sx = PAL_X + (c as i32 - st.scroll_col) * PAL_ZOOM;
            let sy = PAL_Y + (r as i32 - st.scroll_row) * PAL_ZOOM;
            if sheet == Sheet::Structures && rect_hit(sx, sy, PAL_X, PAL_Y, PAL_W, PAL_H) {
                outline(canvas, sx, sy, PAL_ZOOM, PAL_ZOOM, SEL);
            }
        }
    }

    // Prefab preview via REAL render::draw_sprite.
    let mut preview = Surface::new(model.w as u32 * 16, model.h as u32 * 16, PixelFormatEnum::ARGB8888)?;
    preview.fill_rect(None, Color::RGB(70, 100, 55))?;
    let tag_sprite = |name: &str| st.tags.iter().find(|t| t.name == name).and_then(|t| t.sprite());
    let bg = Color::RGB(70, 100, 55);
    for y in 0..model.h {
        for x in 0..model.w {
            let i = model.idx(x, y);
            let (cx, cy) = (x as i32, y as i32);
            if !st.reveal && model.roof[i].is_some() {
                let (c, r) = model.roof[i].unwrap();
                let sp = Sprite::at(Sheet::Structures, c, r);
                let surf = sheets.get(Sheet::Structures)?;
                draw_sprite(&mut preview, surf, cx, cy, sp.src_rect(), WHITE, bg);
                continue;
            }
            // Base terrain + (when present) the interior object layered on top —
            // object hidden under a shown roof, exactly like in-game.
            let base = model.terrain[i].as_deref().and_then(tag_sprite);
            let obj = model.objects[i].as_deref().and_then(tag_sprite);
            match (base, obj) {
                (Some(b), Some(o)) => {
                    let (bs, os) = sheets.get_two(b.sheet, o.sheet)?;
                    draw_sprite_layered(&mut preview, bs, b.src_rect(), os, o.src_rect(), cx, cy, WHITE, bg);
                }
                (Some(sp), None) | (None, Some(sp)) => {
                    let surf = sheets.get(sp.sheet)?;
                    draw_sprite(&mut preview, surf, cx, cy, sp.src_rect(), WHITE, bg);
                }
                (None, None) => {}
            }
        }
    }
    let preview_tex = tc.create_texture_from_surface(&preview).map_err(|e| e.to_string())?;
    let (gw, gh) = (model.w as i32 * GRID_CELL, model.h as i32 * GRID_CELL);
    canvas.copy(&preview_tex, None, Some(Rect::new(GRID_X, GRID_Y, gw as u32, gh as u32)))?;
    canvas.set_draw_color(Color::RGBA(0, 0, 0, 90));
    for cx in 0..=model.w as i32 { canvas.draw_line((GRID_X + cx * GRID_CELL, GRID_Y), (GRID_X + cx * GRID_CELL, GRID_Y + gh)).ok(); }
    for cy in 0..=model.h as i32 { canvas.draw_line((GRID_X, GRID_Y + cy * GRID_CELL), (GRID_X + gw, GRID_Y + cy * GRID_CELL)).ok(); }
    outline(canvas, GRID_X - 1, GRID_Y - 1, gw + 2, gh + 2, FRAME);

    // Bottom bars.
    draw_text(canvas, font, 8, WIN_H as i32 - 46, &st.status, Color::RGB(210, 210, 220));
    draw_text_sm(canvas, font, 8, WIN_H as i32 - 22,
        "L-drag=paint  R=erase  swatch=brush  Tab=layer(Terrain/Roof/Object)  L=label  < >=sheet  R=reveal  W/H=size  Ctrl+S=save  O=list  Esc=quit",
        Color::RGB(150, 150, 165));
    canvas.present();
    Ok(())
}

// ---- draw helpers ---------------------------------------------------------
fn button(canvas: &mut Canvas<Window>, font: &mut Texture, r: Rect, lbl: &str, hot: bool) {
    fill(canvas, r.x(), r.y(), r.width() as i32, r.height() as i32, Color::RGB(46, 46, 58));
    outline(canvas, r.x(), r.y(), r.width() as i32, r.height() as i32, if hot { SEL } else { FRAME });
    draw_text(canvas, font, r.x() + 3, r.y() + 2, lbl, WHITE);
}

fn fill(canvas: &mut Canvas<Window>, x: i32, y: i32, w: i32, h: i32, c: Color) {
    canvas.set_draw_color(c);
    canvas.fill_rect(Rect::new(x, y, w as u32, h as u32)).ok();
}

fn outline(canvas: &mut Canvas<Window>, x: i32, y: i32, w: i32, h: i32, c: Color) {
    canvas.set_draw_color(c);
    canvas.draw_rect(Rect::new(x, y, w as u32, h as u32)).ok();
}

fn glyphs(canvas: &mut Canvas<Window>, font: &mut Texture, x: i32, y: i32, s: &str, color: Color, gw: i32, gh: i32) {
    font.set_color_mod(color.r, color.g, color.b);
    for (i, b) in s.bytes().enumerate() {
        let src = Rect::new((b as i32 % 16) * 16, (b as i32 / 16) * 16, 16, 16);
        let dst = Rect::new(x + i as i32 * gw, y, gw as u32, gh as u32);
        canvas.copy(font, Some(src), Some(dst)).ok();
    }
}
fn draw_text(canvas: &mut Canvas<Window>, font: &mut Texture, x: i32, y: i32, s: &str, color: Color) {
    glyphs(canvas, font, x, y, s, color, GLYPH_W, GLYPH_H);
}
fn draw_text_sm(canvas: &mut Canvas<Window>, font: &mut Texture, x: i32, y: i32, s: &str, color: Color) {
    glyphs(canvas, font, x, y, s, color, 8, 14);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(stem: &str) -> Model {
        let text = std::fs::read_to_string(format!("assets/buildings/{stem}.ron")).unwrap();
        let p: Prefab = ron::from_str(&text).unwrap();
        Model::from_prefab(stem.to_string(), &p)
    }

    #[test]
    fn legacy_prefab_opens_as_tags_and_round_trips() {
        // Loading a legacy (TerrainKind-palette) prefab maps cells to seed
        // tags; re-emitting yields a tag_palette prefab that parses back the
        // same footprint and roof.
        let m = load("house_urban_workshop");
        let emitted = m.to_ron();
        assert!(emitted.contains("tag_palette"));
        let p2: Prefab = ron::from_str(&emitted)
            .unwrap_or_else(|e| panic!("emitted RON did not parse: {e}\n{emitted}"));
        let m2 = Model::from_prefab("x".into(), &p2);
        assert_eq!((m.w, m.h), (m2.w, m2.h));
        assert_eq!(m.terrain, m2.terrain);
        assert_eq!(m.roof, m2.roof);
    }

    #[test]
    fn resize_preserves_overlap() {
        let mut m = Model::blank("r".into(), 3, 3);
        let i = m.idx(1, 1);
        m.roof[i] = Some((6, 2));
        m.terrain[i] = Some("stone_wall".into());
        m.resize(5, 5);
        assert_eq!(m.roof[m.idx(1, 1)], Some((6, 2)));
        assert_eq!(m.terrain[m.idx(1, 1)].as_deref(), Some("stone_wall"));
        m.resize(2, 2);
        assert_eq!((m.w, m.h), (2, 2));
    }
}
