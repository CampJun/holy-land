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
    pub grid: Vec<String>,
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
        let row = self.grid.get(y as usize)?;
        let ch = row.chars().nth(x as usize)?;
        if ch == ' ' {
            return None;
        }
        self.palette.get(&ch).copied()
    }
}

/// All bundled prefabs, indexed by their (tag, tier) bucket. Indirected
/// through `OnceLock` so the parse + validation runs once and panics
/// at boot if any bundled RON is malformed.
pub struct BuildingCatalog {
    pools: HashMap<(String, CityTier), Vec<Prefab>>,
}

const PREFAB_SOURCES: &[(&str, &str)] = &[
    (
        "house_urban_burgage",
        include_str!("../assets/buildings/house_urban_burgage.ron"),
    ),
    (
        "house_urban_workshop",
        include_str!("../assets/buildings/house_urban_workshop.ron"),
    ),
    (
        "smithy_urban_armorer",
        include_str!("../assets/buildings/smithy_urban_armorer.ron"),
    ),
    (
        "smithy_urban_weaponsmith",
        include_str!("../assets/buildings/smithy_urban_weaponsmith.ron"),
    ),
    (
        "house_village_cottage",
        include_str!("../assets/buildings/house_village_cottage.ron"),
    ),
    (
        "smithy_village_farrier",
        include_str!("../assets/buildings/smithy_village_farrier.ron"),
    ),
];

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
                p.palette.contains_key(&ch),
                "prefab {name} cell ({col_j}, {row_i}): char {ch:?} missing from palette",
            );
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
