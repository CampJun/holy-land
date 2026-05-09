use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

pub const CELL_SIZE: u32 = 16;
const ATLAS_COLS: u32 = 16;

pub fn load_atlas<'a>(
    creator: &'a TextureCreator<WindowContext>,
    png_bytes: &[u8],
) -> Result<Texture<'a>, String> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (w, h) = (img.width(), img.height());
    let mut data = img.into_raw();

    for chunk in data.chunks_exact_mut(4) {
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        let is_magenta = r > 240 && g < 16 && b > 240;
        if is_magenta {
            chunk[3] = 0;
        } else {
            chunk[0] = 255;
            chunk[1] = 255;
            chunk[2] = 255;
        }
    }

    let mut texture = creator
        .create_texture_static(PixelFormatEnum::RGBA32, w, h)
        .map_err(|e| e.to_string())?;
    texture
        .update(None, &data, (w * 4) as usize)
        .map_err(|e| e.to_string())?;
    texture.set_blend_mode(BlendMode::Blend);
    Ok(texture)
}

pub fn draw_glyph(
    canvas: &mut Canvas<Window>,
    atlas: &mut Texture,
    cx: i32,
    cy: i32,
    glyph: u8,
    fg: Color,
    bg: Color,
) {
    let px = cx * CELL_SIZE as i32;
    let py = cy * CELL_SIZE as i32;
    let dst = Rect::new(px, py, CELL_SIZE, CELL_SIZE);

    canvas.set_draw_color(bg);
    let _ = canvas.fill_rect(dst);

    atlas.set_color_mod(fg.r, fg.g, fg.b);
    atlas.set_alpha_mod(fg.a);

    let src_x = (glyph as u32 % ATLAS_COLS) * CELL_SIZE;
    let src_y = (glyph as u32 / ATLAS_COLS) * CELL_SIZE;
    let src = Rect::new(src_x as i32, src_y as i32, CELL_SIZE, CELL_SIZE);
    let _ = canvas.copy(atlas, src, dst);
}
