//! Shared library crate for Holy Land. The game binary (`main.rs`) and the
//! desktop-only `prefab-editor` binary (`src/bin/prefab_editor.rs`) both build
//! on these modules. Modules reference each other via `crate::…`, which now
//! resolves to this library; the `#[macro_export]` logging macros are exported
//! at the library crate root (use `holyland::log_info!` etc. from a binary).

pub mod action;
pub mod atlases;
pub mod buildings;
pub mod calendar;
pub mod chunkgen;
pub mod city;
pub mod combat;
pub mod cornwall;
pub mod crafting;
#[cfg(not(target_arch = "arm"))]
pub mod debug_console;
pub mod fasttravel;
pub mod flora;
pub mod fov;
pub mod input;
pub mod items;
pub mod logging;
pub mod needs;
pub mod objects;
pub mod platform;
pub mod render;
pub mod save;
pub mod skill;
pub mod sprite_tags;
pub mod sprites;
pub mod world;
