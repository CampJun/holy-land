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

    // Magenta is the chromakey: those pixels become transparent. Every
    // other pixel's RGB is PRESERVED so the atlas's color and grayscale
    // detail survive the load. The blit in `draw_glyph` color-mod's the
    // result by fg, so:
    //   white-on-magenta sprite x colored fg -> tinted silhouette
    //     (unchanged behavior for standard CP437 chars)
    //   colored sprite x white fg            -> atlas colors preserved
    //   grayscale sprite x colored fg        -> shaded tint
    for chunk in data.chunks_exact_mut(4) {
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        let is_magenta = r > 240 && g < 16 && b > 240;
        if is_magenta {
            chunk[3] = 0;
        }
        // else: keep RGB + alpha exactly as the artist intended.
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
