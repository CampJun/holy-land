// CDDA-style building prefabs. A `Prefab` is a small CP437 grid + a
// char→TerrainKind palette that stamps one building into a city's
// house slot. Variants are tagged (trade, tier) and weight-rolled per
// slot so that towns get historically flavoured silhouettes — e.g. a
// Smythen-Street armurer's house is StoneWall + open shopfront, while
// a village farrier is WoodWall with a road-facing gap.
//
// Catalog is loaded lazily once, like `city::cities()`. Prefab assets
// are embedded via `include_str!` so the cross-compiled Miyoo build
// keeps the same single-binary deploy.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::objects::Object;
use crate::sprite_tags::{catalog as tag_catalog, Role};
use crate::sprites::Sprite;
use crate::world::TerrainKind;

/// Settlement size. Drives which prefab pool a tag draws from — an
/// Urban Exeter blacksmith stamps a different prefab than a Village
/// farrier even though both share the `"smithy"` tag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
pub enum CityTier {
    #[default]
    Urban,
    #[allow(dead_code)] // reserved for Bodmin / Tavistock RONs
    Town,
    #[allow(dead_code)] // reserved for hamlet-tier sites
    Village,
    #[allow(dead_code)] // reserved for un-named clusters
    Hamlet,
}

#[derive(Debug, Deserialize)]
pub struct Prefab {
    pub tag: String,
    pub tier: CityTier,
    #[allow(dead_code)] // surfaces in debug overlay later
    pub variant: String,
    pub weight: u32,
    pub size: (u16, u16),
    pub palette: HashMap<char, TerrainKind>,
    /// Tag-based terrain palette: char -> sprite-tag name (see `sprite_tags`).
    /// Resolved to a passability carrier (`Floor`/`Solid`) + a per-cell sprite.
    /// Takes precedence over `palette` for the same char; lets a prefab paint
    /// any sheet cell as wall/floor. Additive — legacy `palette` still works.
    #[serde(default)]
    pub tag_palette: HashMap<char, String>,
    pub grid: Vec<String>,
    /// Optional 2.5D roof facade layer, same dimensions as `grid`. Each
    /// non-space char maps via `roof_palette` to a `Sheet::Structures`
    /// `(col, row)` cell — roof slope on the upper rows, wall + window +
    /// door facade on the bottom row. Empty = no roof (renders as flat
    /// walls, like every prefab before roofs existed).
    #[serde(default)]
    pub roof: Vec<String>,
    /// Char → `Sheet::Structures` `(col, row)` for the `roof` layer.
    #[serde(default)]
    pub roof_palette: HashMap<char, (u8, u8)>,
    /// Optional interior-object layer, same dimensions as `grid`. Each
    /// non-space char maps via `objects_palette` to a sprite-tag name whose
    /// (non-terrain) role becomes the cell's `objects::Object`.
    #[serde(default)]
    pub objects: Vec<String>,
    /// Char → sprite-tag name for the `objects` layer.
    #[serde(default)]
    pub objects_palette: HashMap<char, String>,
}

impl Prefab {
    pub fn width(&self) -> u16 {
        self.size.0
    }
    pub fn height(&self) -> u16 {
        self.size.1
    }

    /// Resolve the prefab cell at `(x, y)` to a TerrainKind. Returns
    /// `None` for the space character — meaning "don't stamp this
    /// cell, let whatever was here before show through" (useful for
    /// non-rectangular footprints).
    pub fn cell_at(&self, x: u16, y: u16) -> Option<TerrainKind> {
        self.resolve_cell(x, y).map(|(kind, _)| kind)
    }

    /// The sprite-tag name a cell references (tag_palette), if any. Used by the
    /// editor to round-trip the terrain layer as tags.
    pub fn tag_palette_name(&self, x: u16, y: u16) -> Option<&str> {
        let row = self.grid.get(y as usize)?;
        let ch = row.chars().nth(x as usize)?;
        self.tag_palette.get(&ch).map(|s| s.as_str())
    }

    /// The object-tag name a cell references (objects layer), if any. Editor
    /// round-trip for the object layer.
    pub fn objects_palette_name(&self, x: u16, y: u16) -> Option<&str> {
        let row = self.objects.get(y as usize)?;
        let ch = row.chars().nth(x as usize)?;
        self.objects_palette.get(&ch).map(|s| s.as_str())
    }

    /// Resolve a grid cell to its passability carrier `TerrainKind` plus an
    /// optional per-cell sprite. A `tag_palette` char looks up the sprite-tag
    /// catalog: a `Terrain` tag maps to `Floor` (passable) or `Solid` (blocked)
    /// and carries the tag's sprite; a non-terrain tag falls back to `Solid`.
    /// Otherwise the legacy `palette` char gives a `TerrainKind` with no sprite.
    /// `None` = space or an unknown tag (don't stamp).
    pub fn resolve_cell(&self, x: u16, y: u16) -> Option<(TerrainKind, Option<Sprite>)> {
        let row = self.grid.get(y as usize)?;
        let ch = row.chars().nth(x as usize)?;
        if ch == ' ' {
            return None;
        }
        if let Some(tag_name) = self.tag_palette.get(&ch) {
            let tag = tag_catalog().by_name(tag_name)?; // unknown tag -> don't stamp
            let kind = match tag.role {
                Role::Terrain { passable: true, .. } => TerrainKind::Floor,
                _ => TerrainKind::Solid,
            };
            return Some((kind, tag.sprite()));
        }
        self.palette.get(&ch).map(|t| (*t, None))
    }

    /// Resolve the roof facade cell at `(x, y)` to a `Sheet::Structures`
    /// `(col, row)`. Returns `None` for the space char or when the prefab
    /// has no roof layer — meaning "draw nothing on top, this cell shows
    /// its terrain as before".
    pub fn roof_at(&self, x: u16, y: u16) -> Option<(u8, u8)> {
        let row = self.roof.get(y as usize)?;
        let ch = row.chars().nth(x as usize)?;
        if ch == ' ' {
            return None;
        }
        self.roof_palette.get(&ch).copied()
    }

    /// Resolve the interior object at `(x, y)` from the `objects` layer: the
    /// grid char -> tag name -> `Object` (from the tag's non-terrain role).
    /// `None` for space, no objects layer, an unknown tag, or a terrain tag.
    pub fn object_at(&self, x: u16, y: u16) -> Option<Object> {
        let row = self.objects.get(y as usize)?;
        let ch = row.chars().nth(x as usize)?;
        if ch == ' ' {
            return None;
        }
        let name = self.objects_palette.get(&ch)?;
        Object::from_tag(tag_catalog().by_name(name)?)
    }
}

/// All bundled prefabs, indexed by their (tag, tier) bucket. Indirected
/// through `OnceLock` so the parse + validation runs once and panics
/// at boot if any bundled RON is malformed.
pub struct BuildingCatalog {
    pools: HashMap<(String, CityTier), Vec<Prefab>>,
}

// `PREFAB_SOURCES: &[(stem, ron_text)]` is generated by `build.rs`, which globs
// every `assets/buildings/*.ron` (sorted) and embeds it via `include_str!`. Drop
// a new prefab file there (e.g. from the prefab-editor) and it loads on the next
// build — no hand-kept list. Still a single embedded binary for the Miyoo.
include!(concat!(env!("OUT_DIR"), "/prefab_sources.rs"));

static CATALOG: OnceLock<BuildingCatalog> = OnceLock::new();

pub fn catalog() -> &'static BuildingCatalog {
    CATALOG.get_or_init(BuildingCatalog::load)
}

impl BuildingCatalog {
    pub fn load() -> Self {
        let mut pools: HashMap<(String, CityTier), Vec<Prefab>> = HashMap::new();
        for (name, src) in PREFAB_SOURCES {
            let prefab: Prefab = ron::from_str(src)
                .unwrap_or_else(|e| panic!("prefab RON parse failed for {name}: {e}"));
            validate_prefab(name, &prefab);
            pools
                .entry((prefab.tag.clone(), prefab.tier))
                .or_default()
                .push(prefab);
        }
        // Sort each pool by `variant` so the roulette iterates the
        // pool in the same order on every host — important for the
        // Miyoo/desktop cross-determinism that `hash3` already gives
        // us on the slot-decision side.
        for pool in pools.values_mut() {
            pool.sort_by(|a, b| a.variant.cmp(&b.variant));
        }
        Self { pools }
    }

    /// Weighted-roulette pick from the `(tag, tier)` pool. Returns
    /// `None` when the pool is empty or every prefab has weight 0.
    pub fn pick_variant(&self, tag: &str, tier: CityTier, hash: u64) -> Option<&Prefab> {
        let pool = self.pools.get(&(tag.to_string(), tier))?;
        if pool.is_empty() {
            return None;
        }
        let total: u32 = pool.iter().map(|p| p.weight).sum();
        if total == 0 {
            return None;
        }
        let mut roll = (hash % total as u64) as u32;
        for p in pool {
            if roll < p.weight {
                return Some(p);
            }
            roll -= p.weight;
        }
        // Numerically unreachable given the modulo, but keep the
        // graceful tail rather than `unreachable!()` so a future
        // overflow-shaped bug degrades to "first variant" instead
        // of panicking the world-stamp.
        pool.first()
    }

    #[cfg(test)]
    pub fn pool(&self, tag: &str, tier: CityTier) -> Option<&[Prefab]> {
        self.pools.get(&(tag.to_string(), tier)).map(|v| v.as_slice())
    }
}

fn validate_prefab(name: &str, p: &Prefab) {
    assert_eq!(
        p.grid.len(),
        p.size.1 as usize,
        "prefab {name}: grid has {} rows but size.1 is {}",
        p.grid.len(),
        p.size.1,
    );
    for (row_i, row) in p.grid.iter().enumerate() {
        assert_eq!(
            row.chars().count(),
            p.size.0 as usize,
            "prefab {name} row {row_i}: width {} != size.0 {}",
            row.chars().count(),
            p.size.0,
        );
        for (col_j, ch) in row.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            assert!(
                p.palette.contains_key(&ch) || p.tag_palette.contains_key(&ch),
                "prefab {name} cell ({col_j}, {row_i}): char {ch:?} missing from palette/tag_palette",
            );
        }
    }
    // Roof layer is optional, but when present must match the footprint
    // dimensions and have every non-space char covered by roof_palette.
    if !p.roof.is_empty() {
        assert_eq!(
            p.roof.len(),
            p.size.1 as usize,
            "prefab {name}: roof has {} rows but size.1 is {}",
            p.roof.len(),
            p.size.1,
        );
        for (row_i, row) in p.roof.iter().enumerate() {
            assert_eq!(
                row.chars().count(),
                p.size.0 as usize,
                "prefab {name} roof row {row_i}: width {} != size.0 {}",
                row.chars().count(),
                p.size.0,
            );
            for (col_j, ch) in row.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                assert!(
                    p.roof_palette.contains_key(&ch),
                    "prefab {name} roof cell ({col_j}, {row_i}): char {ch:?} missing from roof_palette",
                );
            }
        }
    }
    // Object layer: same shape rules; every non-space char in objects_palette.
    if !p.objects.is_empty() {
        assert_eq!(
            p.objects.len(),
            p.size.1 as usize,
            "prefab {name}: objects has {} rows but size.1 is {}",
            p.objects.len(),
            p.size.1,
        );
        for (row_i, row) in p.objects.iter().enumerate() {
            assert_eq!(
                row.chars().count(),
                p.size.0 as usize,
                "prefab {name} objects row {row_i}: width {} != size.0 {}",
                row.chars().count(),
                p.size.0,
            );
            for (col_j, ch) in row.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                assert!(
                    p.objects_palette.contains_key(&ch),
                    "prefab {name} objects cell ({col_j}, {row_i}): char {ch:?} missing from objects_palette",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_every_bundled_prefab() {
        let cat = catalog();
        // Every ship-target file should appear in *some* pool.
        let total: usize = cat.pools.values().map(|v| v.len()).sum();
        assert_eq!(
            total,
            PREFAB_SOURCES.len(),
            "expected {} prefabs to load, got {}",
            PREFAB_SOURCES.len(),
            total,
        );
    }

    #[test]
    fn roof_layer_parses_validates_and_resolves() {
        // Tests the roof MECHANISM on a synthetic prefab (not a bundled file,
        // which is now editable via the prefab-editor): a roof grid + palette
        // must parse, pass validation, and resolve chars through roof_palette
        // to Sheet::Structures (col, row); space = no roof.
        let src = r####"Prefab(
            tag: "house", tier: Urban, variant: "t", weight: 1, size: (3, 2),
            palette: { '#': WoodWall, '.': Floor },
            grid: [ "#.#", "###" ],
            roof_palette: { 'A': (5, 1), 'D': (0, 42) },
            roof: [ "A A", "ADA" ],
        )"####;
        let p: Prefab = ron::from_str(src).expect("synthetic roofed prefab parses");
        validate_prefab("synthetic", &p); // panics if the roof layer is malformed
        assert_eq!(p.roof_at(0, 0), Some((5, 1)));
        assert_eq!(p.roof_at(1, 0), None, "space char = no roof");
        assert_eq!(p.roof_at(1, 1), Some((0, 42)));
    }

    #[test]
    fn tag_palette_resolves_sprite_and_passability() {
        // A tag_palette prefab resolves each cell to a passability carrier
        // (Solid for a wall tag, Floor for a floor tag) PLUS the tag's sprite.
        let src = r####"Prefab(
            tag: "house", tier: Urban, variant: "t", weight: 1, size: (2, 1),
            palette: {},
            tag_palette: { '#': "stone_wall", '.': "cobble_floor" },
            grid: [ "#." ],
        )"####;
        let p: Prefab = ron::from_str(src).expect("tag_palette prefab parses");
        validate_prefab("synthetic", &p);
        let (wk, ws) = p.resolve_cell(0, 0).expect("wall cell");
        assert_eq!(wk, TerrainKind::Solid);
        assert!(!wk.def().walkable, "wall tag stamps a blocked cell");
        assert!(ws.is_some(), "tag carries a per-cell sprite");
        let (fk, fs) = p.resolve_cell(1, 0).expect("floor cell");
        assert_eq!(fk, TerrainKind::Floor);
        assert!(fk.def().walkable, "floor tag stamps a walkable cell");
        assert!(fs.is_some());
    }

    #[test]
    fn objects_layer_resolves_to_objects() {
        // The objects layer resolves grid chars -> tag names -> Objects with
        // the tag's role kind (Light emits light; Decoration does not).
        let src = r####"Prefab(
            tag: "house", tier: Urban, variant: "t", weight: 1, size: (2, 1),
            palette: {}, tag_palette: { '.': "cobble_floor" }, grid: [ ".." ],
            objects_palette: { 'B': "brazier", 'R': "rug" }, objects: [ "BR" ],
        )"####;
        let p: Prefab = ron::from_str(src).expect("prefab with objects parses");
        validate_prefab("synthetic", &p);
        let b = p.object_at(0, 0).expect("brazier object");
        assert_eq!(b.kind, crate::objects::ObjectKind::Light);
        assert!(b.emits_light());
        let r = p.object_at(1, 0).expect("rug object");
        assert_eq!(r.kind, crate::objects::ObjectKind::Decoration);
        assert!(!r.emits_light());
    }

    #[test]
    fn roofless_prefab_has_no_roof_cells() {
        // A prefab without a roof layer (additive feature) resolves to None
        // everywhere so it renders as flat walls, as before.
        let src = r####"Prefab(
            tag: "house", tier: Urban, variant: "t", weight: 1, size: (2, 2),
            palette: { '#': WoodWall }, grid: [ "##", "##" ],
        )"####;
        let p: Prefab = ron::from_str(src).expect("roofless prefab parses");
        assert!(p.roof.is_empty());
        assert_eq!(p.roof_at(0, 0), None);
    }

    #[test]
    fn catalog_indexes_urban_smithy_pool() {
        let cat = catalog();
        let pool = cat
            .pool("smithy", CityTier::Urban)
            .expect("Urban smithy pool exists");
        assert!(
            pool.iter().any(|p| p.variant == "armorer"),
            "Urban smithy pool must contain armorer"
        );
        assert!(
            pool.iter().any(|p| p.variant == "weaponsmith"),
            "Urban smithy pool must contain weaponsmith"
        );
    }

    #[test]
    fn catalog_indexes_village_smithy_pool() {
        let cat = catalog();
        let pool = cat
            .pool("smithy", CityTier::Village)
            .expect("Village smithy pool exists");
        assert!(
            pool.iter().any(|p| p.variant == "farrier"),
            "Village smithy pool must contain farrier"
        );
    }

    #[test]
    fn pick_variant_is_deterministic() {
        let cat = catalog();
        for h in [0u64, 1, 42, 0xDEADBEEF, 0xFFFF_FFFF_FFFF_FFFF] {
            let a = cat.pick_variant("smithy", CityTier::Urban, h).map(|p| p.variant.clone());
            let b = cat.pick_variant("smithy", CityTier::Urban, h).map(|p| p.variant.clone());
            assert_eq!(a, b, "pick_variant(hash={h:#x}) is not deterministic");
        }
    }

    #[test]
    fn pick_variant_misses_unknown_tag() {
        let cat = catalog();
        assert!(cat.pick_variant("zoo", CityTier::Urban, 0).is_none());
    }
}
