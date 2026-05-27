//! Atlas registry + filesystem discovery.
//!
//! Two embedded atlases (`cp437`, `aesomatica`) are guaranteed; any
//! additional 256×256 PNG dropped into `assets/` next to the binary
//! (or the repo `assets/` for desktop `cargo run`) shows up in the
//! in-game tileset picker on the next launch.

use std::path::{Path, PathBuf};

use sdl2::surface::Surface;

use crate::render::load_atlas;

const CP437_PNG: &[u8] = include_bytes!("../assets/cp437_16x16.png");
const AESOMATICA_PNG: &[u8] = include_bytes!("../assets/Aesomatica_16x16.png");

pub const DEFAULT_KEY: &str = "cp437";
const REQUIRED_DIM: u32 = 256;

pub enum AtlasSource {
    Embedded(&'static [u8]),
    File(PathBuf),
}

pub struct AtlasEntry {
    pub key: String,
    pub display_name: String,
    pub source: AtlasSource,
}

/// Build the list of usable atlases. The two embedded defaults are
/// always present. Any 256×256 PNG found in one of the search
/// directories with a stem not already claimed by an embedded entry
/// is added as a `File` source. Non-256×256 PNGs are silently skipped.
pub fn discover() -> Vec<AtlasEntry> {
    let mut entries = vec![
        AtlasEntry {
            key: "cp437".to_string(),
            display_name: "cp437".to_string(),
            source: AtlasSource::Embedded(CP437_PNG),
        },
        AtlasEntry {
            key: "aesomatica".to_string(),
            display_name: "aesomatica".to_string(),
            source: AtlasSource::Embedded(AESOMATICA_PNG),
        },
    ];

    for dir in search_dirs() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let key = atlas_key_from_stem(stem);
            if entries.iter().any(|e| e.key == key) {
                continue;
            }
            match image::image_dimensions(&path) {
                Ok((REQUIRED_DIM, REQUIRED_DIM)) => {
                    entries.push(AtlasEntry {
                        key,
                        display_name: stem.to_string(),
                        source: AtlasSource::File(path),
                    });
                }
                _ => {}
            }
        }
    }

    entries
}

pub fn load(entry: &AtlasEntry) -> Result<Surface<'static>, String> {
    match &entry.source {
        AtlasSource::Embedded(bytes) => load_atlas(bytes),
        AtlasSource::File(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
            load_atlas(&bytes)
        }
    }
}

/// Lowercase + strip a trailing `_16x16` if present, so
/// `Aesomatica_16x16.png` and `cp437_16x16.png` collide with the
/// embedded entries instead of duplicating them.
fn atlas_key_from_stem(stem: &str) -> String {
    let lower = stem.to_ascii_lowercase();
    lower.strip_suffix("_16x16").unwrap_or(&lower).to_string()
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("assets"));
        }
    }
    dirs.push(PathBuf::from("assets"));
    // De-dupe in case the binary already runs from the repo root.
    let mut seen: Vec<PathBuf> = Vec::new();
    dirs.retain(|d| {
        let canon = d.canonicalize().unwrap_or_else(|_| d.clone());
        if seen.iter().any(|s| s == &canon) {
            false
        } else {
            seen.push(canon);
            true
        }
    });
    dirs.retain(|d| Path::new(d).is_dir());
    dirs
}
