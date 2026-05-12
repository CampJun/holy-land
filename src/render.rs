use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::BlendMode;
use sdl2::surface::Surface;

pub const CELL_SIZE: u32 = 16;
const ATLAS_COLS: u32 = 16;

pub fn load_atlas(png_bytes: &[u8]) -> Result<Surface<'static>, String> {
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

    // image::to_rgba8 = byte order R,G,B,A = pixel ABGR8888 on LE.
    // Convert once to ARGB8888 to match the framebuffer/texture format
    // (mmiyoo's CreateTexture only accepts ARGB8888 / RGB565).
    let temp = Surface::from_data(&mut data, w, h, w * 4, PixelFormatEnum::ABGR8888)?;
    let mut atlas = temp.convert_format(PixelFormatEnum::ARGB8888)?;
    atlas.set_blend_mode(BlendMode::Blend)?;
    Ok(atlas)
}

pub fn draw_glyph(
    framebuf: &mut Surface,
    atlas: &mut Surface,
    cx: i32,
    cy: i32,
    glyph: u8,
    fg: Color,
    bg: Color,
) {
    let px = cx * CELL_SIZE as i32;
    let py = cy * CELL_SIZE as i32;
    let dst = Rect::new(px, py, CELL_SIZE, CELL_SIZE);

    let _ = framebuf.fill_rect(dst, bg);

    atlas.set_color_mod(Color::RGB(fg.r, fg.g, fg.b));
    atlas.set_alpha_mod(fg.a);

    let src_x = (glyph as u32 % ATLAS_COLS) * CELL_SIZE;
    let src_y = (glyph as u32 / ATLAS_COLS) * CELL_SIZE;
    let src = Rect::new(src_x as i32, src_y as i32, CELL_SIZE, CELL_SIZE);
    let _ = atlas.blit(src, framebuf, dst);
}
