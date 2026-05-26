// Tree species + interactive undergrowth vocabulary. The state machine
// behind PlantState transitions, dawn-tick scheduling, and mast drops
// land in Phase D. Phase E replaces uniform random species placement
// with noise-weighted distributions.

use serde::{Deserialize, Serialize};

use crate::calendar::Season;
use crate::items::ItemKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeSpecies {
    Oak,
    Hazel,
    Holly,
    Ash,
    Rowan,
    /// Beech anchors the `BeechCombe` biome (east Devon hedgerow vales).
    /// No mast item yet — `mast_item` returns None until a `Beechnut`
    /// `ItemKind` is added in a future foraging card.
    Beech,
}

#[allow(dead_code)] // exposed for tests + future ManagedSpecies surfaces
pub const ALL_TREE_SPECIES: &[TreeSpecies] = &[
    TreeSpecies::Oak,
    TreeSpecies::Hazel,
    TreeSpecies::Holly,
    TreeSpecies::Ash,
    TreeSpecies::Rowan,
    TreeSpecies::Beech,
];

impl TreeSpecies {
    /// Stable string for save round-tripping. Unknown keys load as None
    /// (i.e., chunkgen will assign a species deterministically per seed).
    #[allow(dead_code)] // Phase D wires the mutation save path
    pub fn save_key(self) -> &'static str {
        match self {
            TreeSpecies::Oak => "oak",
            TreeSpecies::Hazel => "hazel",
            TreeSpecies::Holly => "holly",
            TreeSpecies::Ash => "ash",
            TreeSpecies::Rowan => "rowan",
            TreeSpecies::Beech => "beech",
        }
    }

    #[allow(dead_code)] // Phase D wires the mutation save path
    pub fn from_save_key(s: &str) -> Option<Self> {
        match s {
            "oak" => Some(TreeSpecies::Oak),
            "hazel" => Some(TreeSpecies::Hazel),
            "holly" => Some(TreeSpecies::Holly),
            "ash" => Some(TreeSpecies::Ash),
            "rowan" => Some(TreeSpecies::Rowan),
            "beech" => Some(TreeSpecies::Beech),
            _ => None,
        }
    }

    /// True for species that drop leaves in autumn. Drives the
    /// FallenLeaves ground-cover spawn in Phase D.
    #[allow(dead_code)] // wired in Phase D
    pub fn is_deciduous(self) -> bool {
        !matches!(self, TreeSpecies::Holly)
    }

    /// Per-species canopy glyph. Picked from CP437 atlas; the existing
    /// TREE_VARIANT_GLYPHS array was the species-agnostic catalog.
    pub fn canopy_glyph(self) -> u8 {
        match self {
            TreeSpecies::Oak => 0x05,    // ♣
            TreeSpecies::Hazel => 0x18,  // ↑
            TreeSpecies::Holly => 0x06,  // ♠
            TreeSpecies::Ash => 0x17,    // ↨
            TreeSpecies::Rowan => 0x18,  // ↑ (same glyph, distinguished by tint)
            TreeSpecies::Beech => 0x05,  // ♣ (same glyph as Oak, distinguished by tint)
        }
    }

    /// Canopy fg tint per (species, season). Replaces the
    /// per-cell-hashed TREE_TINT_VARIANTS lottery — each tree's tint is
    /// now its species' authored seasonal hue. Spring/Summer/Autumn/
    /// Winter palette per the species card.
    pub fn canopy_fg(self, season: Season) -> [u8; 3] {
        let row: [[u8; 3]; 4] = match self {
            TreeSpecies::Oak => [
                [100, 150, 55],
                [55, 110, 40],
                [170, 110, 40],
                [140, 130, 110], // bare bark
            ],
            TreeSpecies::Hazel => [
                [140, 170, 80],
                [100, 150, 60],
                [200, 150, 50],
                [150, 140, 110],
            ],
            TreeSpecies::Holly => [
                [40, 100, 40],
                [35, 95, 35],
                [40, 100, 40],
                [40, 100, 40], // evergreen — stays green in winter
            ],
            TreeSpecies::Ash => [
                [120, 160, 70],
                [80, 130, 55],
                [180, 140, 60],
                [155, 145, 125],
            ],
            TreeSpecies::Rowan => [
                [130, 170, 75],
                [90, 135, 55],
                [210, 90, 40], // characteristic autumn red
                [160, 150, 120],
            ],
            TreeSpecies::Beech => [
                [150, 180, 90],
                [85, 130, 55],
                [200, 130, 50], // copper autumn
                [180, 150, 110], // pale beech bark
            ],
        };
        row[season as usize]
    }

    /// Days a chopped sapling of this species takes to promote back to
    /// TreeTrunk in Phase D's regrowth loop. Per the lifecycle card.
    #[allow(dead_code)] // wired in Phase D
    pub fn sapling_days_to_mature(self) -> u32 {
        match self {
            TreeSpecies::Hazel => 30,
            TreeSpecies::Rowan => 45,
            TreeSpecies::Ash => 45,
            TreeSpecies::Holly => 60,
            TreeSpecies::Oak => 90,
            TreeSpecies::Beech => 90,
        }
    }

    /// Mast item (acorn, hazelnut, rowanberry) that drops in autumn near
    /// fruiting tree neighbors. None for Holly/Ash (no v1 mast).
    #[allow(dead_code)] // wired in Phase D
    pub fn mast_item(self) -> Option<ItemKind> {
        match self {
            TreeSpecies::Oak => Some(ItemKind::Acorn),
            TreeSpecies::Hazel => Some(ItemKind::Hazelnut),
            TreeSpecies::Rowan => Some(ItemKind::RowanBerry),
            _ => None,
        }
    }
}

/// Lifecycle state for plants that bear interactive harvest output. Full
/// state-machine transitions wire in Phase D; Phase C just stores the
/// state as a tag so existing decoration values can round-trip through
/// save without losing data when Phase D arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlantState {
    Sprout,
    #[default]
    Mature,
    Fruiting,
    Dormant,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MushroomKind {
    Edible,
    Mild,
    Toxic,
    Hallucinogenic,
}

/// Per-cell decoration overlay. Glyph priority is `item > decoration >
/// terrain` so an item dropped onto a decoration cell still draws on
/// top; the decoration shows through when the cell has no items.
/// Walkable + sight semantics OR with the underlying terrain — Gorse
/// blocks pass AND LOS, the rest pass through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Decoration {
    #[default]
    None,
    Fern {
        state: PlantState,
    },
    Moss,
    Bramble {
        state: PlantState,
    },
    Bracken {
        state: PlantState,
    },
    Gorse {
        state: PlantState,
    },
    Sapling {
        species: TreeSpecies,
        planted_day: u32,
    },
    Mushroom {
        kind: MushroomKind,
        expires_day: u32,
    },
}

impl Decoration {
    /// Does this decoration prevent walking through the cell? Gorse —
    /// the thorny shrub — is the only blocker. Saplings, mushrooms,
    /// and herbaceous undergrowth pass through.
    pub fn blocks_pass(self) -> bool {
        matches!(self, Decoration::Gorse { .. })
    }

    /// Does this decoration block line of sight? Same set as
    /// blocks_pass — Gorse is dense enough to break sightlines.
    pub fn blocks_sight(self) -> bool {
        matches!(self, Decoration::Gorse { .. })
    }

    /// CP437 glyph for the overlay. Returns 0 for `None` (caller is
    /// expected to short-circuit).
    #[allow(dead_code)] // wired with chunkgen decoration placement (Phase D)
    pub fn glyph(self) -> u8 {
        match self {
            Decoration::None => 0,
            Decoration::Fern { .. } => 0xF0,
            Decoration::Moss => 0x07,
            Decoration::Bramble { .. } => 0x9D,
            Decoration::Bracken { .. } => 0x16,
            Decoration::Gorse { .. } => 0x05,
            Decoration::Sapling { .. } => 0x18,
            Decoration::Mushroom { .. } => 0xFA,
        }
    }

    /// Foreground tint for the overlay glyph. Season-aware where the
    /// plant genuinely shifts (bracken copper in autumn, gorse yellow
    /// in spring/summer). Mushrooms render with their kind tint, not
    /// a season-driven one.
    #[allow(dead_code)] // wired with chunkgen decoration placement (Phase D)
    pub fn fg(self, season: Season) -> [u8; 3] {
        match self {
            Decoration::None => [0, 0, 0],
            Decoration::Fern { .. } => match season {
                Season::Autumn => [170, 130, 60],
                Season::Winter => [120, 110, 95],
                _ => [80, 145, 70],
            },
            Decoration::Moss => match season {
                Season::Winter => [80, 130, 130],
                _ => [70, 140, 80],
            },
            Decoration::Bramble { .. } => match season {
                Season::Spring => [110, 150, 80],
                Season::Summer => [110, 90, 130], // berry purple
                Season::Autumn => [160, 60, 70],
                Season::Winter => [130, 100, 75],
            },
            Decoration::Bracken { .. } => match season {
                Season::Summer => [90, 140, 60],
                Season::Autumn => [180, 110, 45],
                _ => [110, 95, 65],
            },
            Decoration::Gorse { .. } => match season {
                Season::Spring | Season::Summer => [220, 200, 70], // flowering
                _ => [60, 110, 50],
            },
            Decoration::Sapling { species, .. } => species.canopy_fg(season),
            Decoration::Mushroom { kind, .. } => match kind {
                MushroomKind::Edible => [200, 170, 130],
                MushroomKind::Mild => [180, 140, 90],
                MushroomKind::Toxic => [200, 80, 70],
                MushroomKind::Hallucinogenic => [180, 90, 180],
            },
        }
    }
}

/// Foragable items in season — driven by `(season)` lookup. Wires into
/// HUD "in-season" hints and NPC dialog in a future card. Listed here
/// so Phase D's lifecycle scheduler shares the table.
#[allow(dead_code)] // exposed for Phase D's UI surfacing
pub fn forage_in_season(season: Season) -> &'static [ItemKind] {
    match season {
        Season::Spring => &[ItemKind::Herb, ItemKind::FernFrond],
        Season::Summer => &[
            ItemKind::Herb,
            ItemKind::FernFrond,
            ItemKind::BrambleFruit,
            ItemKind::BrackenStraw,
        ],
        Season::Autumn => &[
            ItemKind::Hazelnut,
            ItemKind::RowanBerry,
            ItemKind::Acorn,
            ItemKind::BrambleFruit,
        ],
        Season::Winter => &[ItemKind::Moss, ItemKind::GorseFaggot],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holly_is_evergreen_others_deciduous() {
        for s in ALL_TREE_SPECIES {
            let dec = s.is_deciduous();
            let expected = !matches!(s, TreeSpecies::Holly);
            assert_eq!(dec, expected, "{:?}", s);
        }
    }

    #[test]
    fn canopy_fg_changes_with_season_for_deciduous() {
        // Oak (deciduous) should look different in autumn than summer.
        let oak = TreeSpecies::Oak;
        assert_ne!(
            oak.canopy_fg(Season::Summer),
            oak.canopy_fg(Season::Autumn),
            "Oak should color-shift Summer → Autumn"
        );
        // Holly is evergreen — no copper-autumn shift. Spring/Autumn/
        // Winter all share the same green; Summer is allowed a slight
        // dimming.
        let holly = TreeSpecies::Holly;
        assert_eq!(
            holly.canopy_fg(Season::Spring),
            holly.canopy_fg(Season::Autumn),
            "Holly Autumn must match Spring — no copper shift"
        );
        assert_eq!(
            holly.canopy_fg(Season::Spring),
            holly.canopy_fg(Season::Winter),
            "Holly Winter must match Spring — evergreen"
        );
    }

    #[test]
    fn gorse_blocks_movement_and_sight() {
        let g = Decoration::Gorse {
            state: PlantState::Mature,
        };
        assert!(g.blocks_pass());
        assert!(g.blocks_sight());
        let f = Decoration::Fern {
            state: PlantState::Mature,
        };
        assert!(!f.blocks_pass());
        assert!(!f.blocks_sight());
    }

    #[test]
    fn tree_species_save_key_round_trips() {
        for s in ALL_TREE_SPECIES {
            assert_eq!(TreeSpecies::from_save_key(s.save_key()), Some(*s));
        }
    }
}
