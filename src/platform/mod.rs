// Platform abstraction. The rest of the codebase calls into this module —
// nothing else touches OS APIs directly. Backends are cfg-gated:
//
//   desktop.rs        — Linux/macOS/Windows desktop + Linux retro handhelds
//   linux_handheld.rs — handheld-specific overrides (placeholder, see file)
//   android.rs        — Retroid Pocket 5 and other Android handhelds
//
// Surface for Session 1 is intentionally tiny (just `save_dir`); it grows
// as save/audio/asset-loading land in later sessions.

#[cfg(not(target_os = "android"))]
#[path = "desktop.rs"]
mod imp;

#[cfg(target_os = "android")]
#[path = "android.rs"]
mod imp;

pub use imp::save_dir;
