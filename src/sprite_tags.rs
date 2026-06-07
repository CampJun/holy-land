//! Sprite-tag catalog. A "tag" labels a single sprite-sheet cell with a
//! gameplay **role** — turning raw art into a wall / floor / anvil / chest /
//! hearth. Both the game and the prefab-editor read this catalog; the editor's
//! Label mode writes it. Loaded once from the embedded `assets/sprite_tags.ron`.
//!
//! This is the unifying mechanism behind the whole tagged-tile content system:
//! terrain passability and interior-object usage are both just a tag's role.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::sprites::{Sheet, Sprite};

/// What a tagged sheet cell *is*, gameplay-wise.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum Role {
    /// A floor-plan cell: its passability flag (and whether it blocks sight).
    /// `passable: false` stamps a solid (blocked) cell; `true` a walkable floor.
    Terrain { passable: bool, blocks_sight: bool },
    /// Crafting station tied to a skill (e.g. anvil -> "metallurgy"). Phase 3.
    Station { skill: String },
    /// Searchable loot container, keyed to a loot table. Phase 4.
    Container { loot: String },
    /// Emits light (reuses the fire light path). Phase 2.
    Light,
    /// Purely visual interior dressing. Phase 2.
    Decoration,
}

/// One labeled sheet cell.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tag {
    pub name: String,
    pub sheet: String, // matches sprites::Sheet::name()
    pub col: u8,
    pub row: u8,
    pub role: Role,
}

impl Tag {
    /// The sprite this tag points at (None if the sheet name is unknown).
    pub fn sprite(&self) -> Option<Sprite> {
        Sheet::from_name(&self.sheet).map(|s| Sprite::at(s, self.col, self.row))
    }

    pub fn is_terrain(&self) -> bool {
        matches!(self.role, Role::Terrain { .. })
    }
}

const CATALOG_RON: &str = include_str!("../assets/sprite_tags.ron");

pub struct Catalog {
    tags: Vec<Tag>,
    by_name: HashMap<String, usize>,
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();

/// The bundled catalog, parsed + validated once (panics at boot if malformed).
pub fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(Catalog::load)
}

impl Catalog {
    fn load() -> Self {
        let tags: Vec<Tag> = ron::from_str(CATALOG_RON)
            .unwrap_or_else(|e| panic!("sprite_tags.ron parse failed: {e}"));
        let mut by_name = HashMap::new();
        for (i, t) in tags.iter().enumerate() {
            if by_name.insert(t.name.clone(), i).is_some() {
                panic!("sprite_tags.ron: duplicate tag name {:?}", t.name);
            }
        }
        Self { tags, by_name }
    }

    pub fn by_name(&self, name: &str) -> Option<&Tag> {
        self.by_name.get(name).map(|&i| &self.tags[i])
    }

    pub fn all(&self) -> &[Tag] {
        &self.tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_loads_and_resolves_sprites() {
        let cat = catalog(); // panics if sprite_tags.ron is malformed
        assert!(!cat.all().is_empty());
        let wall = cat.by_name("stone_wall").expect("seed stone_wall tag");
        assert!(matches!(wall.role, Role::Terrain { passable: false, .. }));
        assert!(wall.sprite().is_some(), "tag must resolve to a real sheet cell");
        let floor = cat.by_name("cobble_floor").expect("seed cobble_floor tag");
        assert!(matches!(floor.role, Role::Terrain { passable: true, .. }));
    }
}
