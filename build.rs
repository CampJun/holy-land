//! Auto-discovers building prefabs so the engine doesn't need a hand-kept
//! `include_str!` list. At build time this globs `assets/buildings/*.ron`,
//! sorts them (stable cross-host order, like the in-engine pools expect), and
//! generates `PREFAB_SOURCES` into OUT_DIR. `buildings.rs` includes the file.
//!
//! The sprite/atlas assets stay embedded via `include_bytes!` in their own
//! modules; this only covers the prefab RONs, which the editor tool creates at
//! runtime — dropping a new `.ron` here makes it embed on the next build.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let dir = Path::new("assets/buildings");
    // Rerun when a prefab is added/removed (dir mtime) or edited (file mtime).
    println!("cargo:rerun-if-changed=assets/buildings");

    let mut entries: Vec<(String, String)> = Vec::new();
    if let Ok(read) = fs::read_dir(dir) {
        for e in read.flatten() {
            let path = e.path();
            if path.extension().map(|x| x == "ron").unwrap_or(false) {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .expect("prefab filename is valid UTF-8")
                    .to_string();
                let abs = fs::canonicalize(&path)
                    .expect("canonicalize prefab path")
                    .to_string_lossy()
                    .into_owned();
                println!("cargo:rerun-if-changed={}", path.display());
                entries.push((stem, abs));
            }
        }
    }
    entries.sort();

    let mut code = String::from("pub static PREFAB_SOURCES: &[(&str, &str)] = &[\n");
    for (stem, abs) in entries {
        // `abs` is an absolute path, so include_str! resolves regardless of the
        // generated file's own location under OUT_DIR.
        code.push_str(&format!("    ({stem:?}, include_str!({abs:?})),\n"));
    }
    code.push_str("];\n");

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("prefab_sources.rs");
    fs::write(&out, code).expect("write generated prefab_sources.rs");
}
