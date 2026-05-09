use std::path::PathBuf;

// Stub for Session 2 (Retroid Pocket 5 / Android handhelds).
// Real impl will use SDL_AndroidGetInternalStoragePath and bundle assets via
// the APK asset manager. For now, return a placeholder so the cfg-gated build
// compiles symmetrically.
pub fn save_dir() -> PathBuf {
    PathBuf::from("/data/local/holyland")
}
