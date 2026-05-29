//! Mini Medieval (V3X3D) 8×8 sprite pack — addressable registry + lazy loader.
//!
//! Two asset folders ship with the pack:
//!   - `assets/Mini-Medieval-Documented-8x8/` — the *legend*: every sprite
//!     labeled, but with text + group boxes baked into the pixels and an
//!     irregular layout. NOT renderable; read it to learn what a cell is.
//!   - `assets/Mini-Medieval-8x8/`           — the *plain* uniform 8px-grid
//!     sheets we actually render from (this module embeds these).
//!
//! A `Sprite` is `{sheet, col, row}` so EVERY cell of EVERY sheet is
//! addressable — that is the "wire it all up" foundation: future systems
//! (animals, crops, ores, ships) can name art that already exists before
//! the gameplay code does. The named constants/mapping fns below cover the
//! content the game renders today; the rest stays reachable via `Sprite::at`.
//!
//! Coordinate confidence:
//!   - Terrain, trees, decorations, units, fire: read off the plain grid
//!     against the documented legend — solid.
//!   - Items: best-effort cell picks; tune by eye once the game renders
//!     (some equipment art may live on other sheets). Marked inline.
#![allow(dead_code)]

use std::collections::HashMap;

use sdl2::rect::Rect;
use sdl2::surface::Surface;

use crate::flora::{Decoration, TreeSize, TreeSpecies, ALL_TREE_SPECIES};
use crate::items::ItemKind;
use crate::render::load_atlas;
use crate::save::SpriteOverride;
use crate::world::{TerrainKind, REMAPPABLE_TERRAINS};

/// Native sprite edge in source pixels. Rendered 2× into a 16px cell.
pub const SRC: u32 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sheet {
    Overworld,
    Items,
    Walls,
    Units,
    Structures,
    Crops,
    Ores,
    Animals,
    Misc,
    Ships,
    Interface,
}

impl Sheet {
    pub const COUNT: usize = 11;

    /// Every sheet, in display order. Backs the picker's sheet cycling.
    pub const ALL: [Sheet; Self::COUNT] = [
        Sheet::Overworld,
        Sheet::Items,
        Sheet::Walls,
        Sheet::Units,
        Sheet::Structures,
        Sheet::Crops,
        Sheet::Ores,
        Sheet::Animals,
        Sheet::Misc,
        Sheet::Ships,
        Sheet::Interface,
    ];

    /// Stable lowercase key (save round-trip + picker header).
    pub fn name(self) -> &'static str {
        match self {
            Sheet::Overworld => "overworld",
            Sheet::Items => "items",
            Sheet::Walls => "walls",
            Sheet::Units => "units",
            Sheet::Structures => "structures",
            Sheet::Crops => "crops",
            Sheet::Ores => "ores",
            Sheet::Animals => "animals",
            Sheet::Misc => "misc",
            Sheet::Ships => "ships",
            Sheet::Interface => "interface",
        }
    }

    pub fn from_name(s: &str) -> Option<Sheet> {
        Sheet::ALL.into_iter().find(|sh| sh.name() == s)
    }

    /// Grid size `(cols, rows)` of the plain sheet — bounds picker
    /// navigation. Ships is 143px wide (17 full 8px cols + a 7px
    /// remainder we don't address).
    pub fn grid(self) -> (u8, u8) {
        match self {
            Sheet::Overworld => (24, 93),
            Sheet::Items => (20, 44),
            Sheet::Walls => (29, 72),
            Sheet::Units => (116, 83),
            Sheet::Structures => (60, 109),
            Sheet::Crops => (24, 29),
            Sheet::Ores => (26, 24),
            Sheet::Animals => (41, 98),
            Sheet::Misc => (55, 49),
            Sheet::Ships => (17, 29),
            Sheet::Interface => (9, 20),
        }
    }

    /// Embedded plain-sheet PNG bytes. Decoded lazily by `SpriteSheets`.
    fn bytes(self) -> &'static [u8] {
        match self {
            Sheet::Overworld => include_bytes!("../assets/Mini-Medieval-8x8/Overworld.png"),
            Sheet::Items => include_bytes!("../assets/Mini-Medieval-8x8/Items.png"),
            Sheet::Walls => include_bytes!("../assets/Mini-Medieval-8x8/Walls.png"),
            Sheet::Units => include_bytes!("../assets/Mini-Medieval-8x8/Units.png"),
            Sheet::Structures => include_bytes!("../assets/Mini-Medieval-8x8/Structures.png"),
            Sheet::Crops => include_bytes!("../assets/Mini-Medieval-8x8/Crops.png"),
            Sheet::Ores => include_bytes!("../assets/Mini-Medieval-8x8/Ores.png"),
            Sheet::Animals => include_bytes!("../assets/Mini-Medieval-8x8/Animals.png"),
            Sheet::Misc => include_bytes!("../assets/Mini-Medieval-8x8/Misc.png"),
            Sheet::Ships => include_bytes!("../assets/Mini-Medieval-8x8/Ships.png"),
            Sheet::Interface => include_bytes!("../assets/Mini-Medieval-8x8/Interface.png"),
        }
    }
}

/// A single 8×8 cell in a named sheet. `Copy` + `Eq` so it lives in the
/// per-cell diff `Cell` without extra cost.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sprite {
    pub sheet: Sheet,
    pub col: u8,
    pub row: u8,
}

impl Sprite {
    pub const fn at(sheet: Sheet, col: u8, row: u8) -> Self {
        Self { sheet, col, row }
    }

    /// Source rect into the sheet surface (native 8×8 pixels).
    pub fn src_rect(self) -> Rect {
        Rect::new(
            self.col as i32 * SRC as i32,
            self.row as i32 * SRC as i32,
            SRC,
            SRC,
        )
    }
}

/// Holds decoded sheet surfaces. Sheets decode lazily on first access so
/// the Miyoo only pays RAM for sheets the run actually draws (Units alone
/// is ~2.4 MB decoded). Mirrors how `atlas` is threaded through main.rs.
pub struct SpriteSheets {
    surfaces: [Option<Surface<'static>>; Sheet::COUNT],
}

impl SpriteSheets {
    pub fn new() -> Self {
        Self {
            surfaces: std::array::from_fn(|_| None),
        }
    }

    /// Decoded surface for `s`, decoding (and caching) on first use.
    /// Reuses `render::load_atlas` (chromakey is harmless on these alpha
    /// PNGs; converts to ARGB8888 + Blend like the CP437 atlas).
    pub fn get(&mut self, s: Sheet) -> Result<&mut Surface<'static>, String> {
        let i = s as usize;
        if self.surfaces[i].is_none() {
            self.surfaces[i] = Some(load_atlas(s.bytes())?);
        }
        Ok(self.surfaces[i].as_mut().unwrap())
    }
}

impl Default for SpriteSheets {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Registry — named cells, grouped by sheet. Coordinates are (col, row) on
// the PLAIN grid in assets/Mini-Medieval-8x8/. Use the documented sheet as
// the legend when adding more.
// ===========================================================================

/// Overworld.png (24×93): terrain, water, paths, trees, bushes.
pub mod overworld {
    use super::{Sheet, Sprite};
    const S: Sheet = Sheet::Overworld;

    // --- Ground (rows 0-6) --- (0,0) is a flat single-color fill; the
    // (1,0)+ cells carry grass-blade texture, so default to a textured one.
    pub const GRASS_FLAT: Sprite = Sprite::at(S, 0, 0);
    pub const GRASS: Sprite = Sprite::at(S, 1, 0);
    pub const GRASS_TUFTS: Sprite = Sprite::at(S, 1, 1);
    pub const GRASS_FLOWERS_YELLOW: Sprite = Sprite::at(S, 0, 2);
    pub const GRASS_FLOWERS_PINK: Sprite = Sprite::at(S, 0, 5);
    pub const GRASS_FLOWERS_RED: Sprite = Sprite::at(S, 0, 6);

    // --- Floors (rows 38-41): dirt / gravel-sand / cobble auto-tile fills ---
    pub const DIRT: Sprite = Sprite::at(S, 0, 39);
    pub const SAND: Sprite = Sprite::at(S, 9, 40);
    pub const COBBLE: Sprite = Sprite::at(S, 14, 40);

    // --- Water (basic rows 8-15, plain fill in the details block) ---
    pub const WATER: Sprite = Sprite::at(S, 0, 32);

    // --- Bushes / undergrowth (rows 80-92) ---
    pub const BUSH_GREEN: Sprite = Sprite::at(S, 1, 89);
    pub const BUSH_BLUEBERRY: Sprite = Sprite::at(S, 5, 84);
    pub const BUSH_RASPBERRY: Sprite = Sprite::at(S, 8, 84);
    pub const BUSH_YELLOW: Sprite = Sprite::at(S, 19, 84);

    // --- Saplings + logs (row 44) ---
    pub const SAPLING: Sprite = Sprite::at(S, 12, 44);

    // --- Bare/brown small tree (rows 66-74) ---
    pub const TREE_BARE: Sprite = Sprite::at(S, 0, 70);

    // --- Tree canopies (green fruit trees rows 51-63, evergreen 76-79) ---
    pub const TREE_GREEN_SMALL: Sprite = Sprite::at(S, 0, 51);
    pub const TREE_GREEN_ROUND: Sprite = Sprite::at(S, 3, 52);
    pub const TREE_GREEN_TALL: Sprite = Sprite::at(S, 8, 51);
    pub const TREE_GREEN_BIG: Sprite = Sprite::at(S, 15, 51);
    pub const TREE_EVERGREEN: Sprite = Sprite::at(S, 1, 76);
    pub const TREE_FRUIT_RED: Sprite = Sprite::at(S, 0, 57);

    // --- Multi-cell tree BLOCKS: top-left cell of each (species, size)
    // tree. Render adds the per-cell (sub_col, sub_row) offset to
    // address each 8×8 slice. Within a band the 2×2 small tree sits at
    // cols 0/6/12/18 and the 4×4 full-crown tree at cols 2/8/14/20.
    // Plain-green band = row 51 (Oak/Beech); pink-fruit band = row 56
    // (Ash/Hazel); red-berry block = (12,56)/(14,56) (Rowan); evergreen
    // band = row 76 (Holly). Verified against the sheet; tune by eye. ---
    pub const TREE_OAK_YOUNG: Sprite = Sprite::at(S, 0, 51);
    pub const TREE_OAK_MATURE: Sprite = Sprite::at(S, 2, 51);
    pub const TREE_BEECH_YOUNG: Sprite = Sprite::at(S, 6, 51);
    pub const TREE_BEECH_MATURE: Sprite = Sprite::at(S, 8, 51);
    pub const TREE_ASH_YOUNG: Sprite = Sprite::at(S, 0, 56);
    pub const TREE_ASH_MATURE: Sprite = Sprite::at(S, 2, 56);
    pub const TREE_HAZEL_YOUNG: Sprite = Sprite::at(S, 6, 56);
    pub const TREE_HAZEL_MATURE: Sprite = Sprite::at(S, 8, 56);
    pub const TREE_ROWAN_YOUNG: Sprite = Sprite::at(S, 12, 56);
    pub const TREE_ROWAN_MATURE: Sprite = Sprite::at(S, 14, 56);
    pub const TREE_HOLLY_YOUNG: Sprite = Sprite::at(S, 0, 76);
    pub const TREE_HOLLY_MATURE: Sprite = Sprite::at(S, 2, 76);
}

/// Items.png (20×44): weapons, armor, tools, food, materials.
/// Canonical art is the LEFT block (cols 0-9); cols 10-19 are navy-bg dups.
/// NOTE: item cells are best-effort — verify/tune in-game.
pub mod items {
    use super::{Sheet, Sprite};
    const S: Sheet = Sheet::Items;

    // Blades / polearms (rows 0-2, 6)
    pub const SPEAR: Sprite = Sprite::at(S, 0, 0);
    pub const LANCE: Sprite = Sprite::at(S, 1, 0);
    pub const GISARME: Sprite = Sprite::at(S, 2, 0);
    pub const SHORTSWORD: Sprite = Sprite::at(S, 1, 1);
    pub const ARMING_SWORD: Sprite = Sprite::at(S, 1, 2);
    pub const FALCHION: Sprite = Sprite::at(S, 2, 2);
    pub const KNIFE: Sprite = Sprite::at(S, 0, 6);
    pub const DAGGER: Sprite = Sprite::at(S, 3, 2);

    // Ranged (rows 3, 6)
    pub const BOW: Sprite = Sprite::at(S, 0, 3);
    pub const CROSSBOW: Sprite = Sprite::at(S, 2, 3);
    pub const ARROW: Sprite = Sprite::at(S, 7, 6);
    pub const BOLT: Sprite = Sprite::at(S, 8, 6);

    // Hafted / blunt (tools rows 7-8, staff row 4)
    pub const AXE: Sprite = Sprite::at(S, 0, 7);
    pub const PICK: Sprite = Sprite::at(S, 3, 7);
    pub const HAMMER: Sprite = Sprite::at(S, 5, 7);
    pub const STAFF: Sprite = Sprite::at(S, 0, 4);
    pub const CUDGEL: Sprite = Sprite::at(S, 5, 7);
    pub const QUARTERSTAFF: Sprite = Sprite::at(S, 1, 4);
    pub const FLINT_AND_STEEL: Sprite = Sprite::at(S, 0, 8);

    // Armor — APPROXIMATE (helmets/shields may live on other sheets)
    pub const HELM_SKULLCAP: Sprite = Sprite::at(S, 0, 9);
    pub const HELM_KETTLE: Sprite = Sprite::at(S, 1, 9);
    pub const HELM_COIF: Sprite = Sprite::at(S, 2, 9);
    pub const HELM_GREAT: Sprite = Sprite::at(S, 0, 10);
    pub const TORSO_DOUBLET: Sprite = Sprite::at(S, 0, 12);
    pub const TORSO_JERKIN: Sprite = Sprite::at(S, 1, 12);
    pub const TORSO_HAUBERK: Sprite = Sprite::at(S, 2, 12);
    pub const TORSO_PLATES: Sprite = Sprite::at(S, 0, 13);
    pub const LEGS_CHAUSSES: Sprite = Sprite::at(S, 1, 13);
    pub const SHIELD_ROUND: Sprite = Sprite::at(S, 0, 10);
    pub const SHIELD_LARGE: Sprite = Sprite::at(S, 1, 10);

    // Potions / rings
    pub const POTION: Sprite = Sprite::at(S, 0, 11);
    pub const RING: Sprite = Sprite::at(S, 0, 10);

    // Materials (wood row 35, ore row 37, leaves row 24)
    pub const WOOD_LOG: Sprite = Sprite::at(S, 0, 35);
    pub const STICK: Sprite = Sprite::at(S, 1, 35);
    pub const STONE: Sprite = Sprite::at(S, 0, 37);
    pub const ORE: Sprite = Sprite::at(S, 2, 37);
    pub const LEAF_GREEN: Sprite = Sprite::at(S, 0, 24);
    pub const LEAF_BROWN: Sprite = Sprite::at(S, 2, 24);
    pub const FEATHER: Sprite = Sprite::at(S, 0, 41);
    pub const LEATHER: Sprite = Sprite::at(S, 0, 43);
    pub const WOOL: Sprite = Sprite::at(S, 0, 39);

    // Food / forage
    pub const BREAD: Sprite = Sprite::at(S, 0, 22);
    pub const MEAT_RAW: Sprite = Sprite::at(S, 0, 31);
    pub const MEAT_COOKED: Sprite = Sprite::at(S, 4, 31);
    pub const FISH: Sprite = Sprite::at(S, 1, 31);
    pub const BERRIES: Sprite = Sprite::at(S, 0, 26);
    pub const BERRIES_RED: Sprite = Sprite::at(S, 4, 26);
    pub const NUT_HAZEL: Sprite = Sprite::at(S, 8, 26);
    pub const NUT_ACORN: Sprite = Sprite::at(S, 9, 26);
    pub const MUSHROOM: Sprite = Sprite::at(S, 0, 25);
    pub const FLOWER: Sprite = Sprite::at(S, 0, 21);
    pub const SEED: Sprite = Sprite::at(S, 0, 15);
    pub const HERB: Sprite = Sprite::at(S, 4, 21);
    pub const WATERSKIN: Sprite = Sprite::at(S, 0, 11);
    pub const BASKET: Sprite = Sprite::at(S, 1, 23);
}

/// Walls.png (29×72): building facades / palisades. The pack is oriented
/// toward authored building art rather than a clean top-down Wang set, so
/// for now we expose solid stone/wood blocks; full neighbor-mask auto-tiling
/// can be refined later by selecting edge/corner cells from rows 0-9.
pub mod walls {
    use super::{Sheet, Sprite};
    const S: Sheet = Sheet::Walls;
    pub const STONE: Sprite = Sprite::at(S, 0, 0);
    pub const WOOD: Sprite = Sprite::at(S, 16, 0);
}

/// Units.png (116×83): characters as animation strips (IDLE leftmost,
/// one character per row-band). We use the IDLE frame (col 0) of each.
pub mod units {
    use super::{Sheet, Sprite};
    const S: Sheet = Sheet::Units;
    pub const PLAYER: Sprite = Sprite::at(S, 0, 2);
    pub const BANDIT_RABBLE: Sprite = Sprite::at(S, 0, 18);
    pub const BANDIT_YEOMAN: Sprite = Sprite::at(S, 0, 4);
    pub const BANDIT_SERGEANT: Sprite = Sprite::at(S, 0, 7);
    pub const BANDIT_KNIGHT: Sprite = Sprite::at(S, 0, 6);
}

/// Misc.png (55×49): effects + odds-and-ends. Fire/flame lives here for
/// the lit-campfire overlay. Coords approximate — refine when used.
pub mod misc {
    use super::{Sheet, Sprite};
    const S: Sheet = Sheet::Misc;
    pub const FIRE: Sprite = Sprite::at(S, 0, 0);
}

// --- Future-content sheets: addressable now, named as systems land ---
// Crops.png (24×29), Ores.png (26×24), Animals.png (41×98),
// Structures.png (60×109), Ships.png (17×29), Interface.png (9×20).
// Reach any cell via Sprite::at(Sheet::Crops, col, row) etc.; see the
// documented legend for what each region holds.

// ===========================================================================
// Mapping functions — game content → Sprite. The render path calls these.
// ===========================================================================

/// Top-left cell of the (species, size) multi-cell tree block. The
/// render path adds the per-cell `(sub_col, sub_row)` offset to address
/// each 8×8 slice of the 2×2 / 4×4 tree (see the compose loop). This is
/// the authority for stamped trees; `tree_sprite` below stays the
/// single-cell representative for the remap picker + legacy fallback.
pub fn tree_base(species: TreeSpecies, size: TreeSize) -> Sprite {
    use overworld as O;
    match (species, size) {
        (TreeSpecies::Oak, TreeSize::Young) => O::TREE_OAK_YOUNG,
        (TreeSpecies::Oak, TreeSize::Mature) => O::TREE_OAK_MATURE,
        (TreeSpecies::Beech, TreeSize::Young) => O::TREE_BEECH_YOUNG,
        (TreeSpecies::Beech, TreeSize::Mature) => O::TREE_BEECH_MATURE,
        (TreeSpecies::Ash, TreeSize::Young) => O::TREE_ASH_YOUNG,
        (TreeSpecies::Ash, TreeSize::Mature) => O::TREE_ASH_MATURE,
        (TreeSpecies::Hazel, TreeSize::Young) => O::TREE_HAZEL_YOUNG,
        (TreeSpecies::Hazel, TreeSize::Mature) => O::TREE_HAZEL_MATURE,
        (TreeSpecies::Rowan, TreeSize::Young) => O::TREE_ROWAN_YOUNG,
        (TreeSpecies::Rowan, TreeSize::Mature) => O::TREE_ROWAN_MATURE,
        (TreeSpecies::Holly, TreeSize::Young) => O::TREE_HOLLY_YOUNG,
        (TreeSpecies::Holly, TreeSize::Mature) => O::TREE_HOLLY_MATURE,
    }
}

/// A reasonable canopy sprite per tree species (one static cell each).
pub fn tree_sprite(species: TreeSpecies) -> Sprite {
    match species {
        TreeSpecies::Oak => overworld::TREE_GREEN_BIG,
        TreeSpecies::Beech => overworld::TREE_GREEN_ROUND,
        TreeSpecies::Ash => overworld::TREE_GREEN_TALL,
        TreeSpecies::Hazel => overworld::TREE_GREEN_SMALL,
        TreeSpecies::Holly => overworld::TREE_EVERGREEN,
        TreeSpecies::Rowan => overworld::TREE_FRUIT_RED,
    }
}

pub fn terrain_sprite(kind: TerrainKind) -> Sprite {
    match kind {
        TerrainKind::Grass => overworld::GRASS,
        TerrainKind::BareDirt => overworld::DIRT,
        TerrainKind::SandShore => overworld::SAND,
        TerrainKind::TreeTrunk => overworld::TREE_GREEN_BIG, // species refines in compose
        TerrainKind::StreamWater | TerrainKind::PondWater => overworld::WATER,
        TerrainKind::Wall | TerrainKind::StoneWall => walls::STONE,
        TerrainKind::WoodWall => walls::WOOD,
        TerrainKind::Floor => overworld::COBBLE,
        TerrainKind::CobbleRoad => overworld::COBBLE,
    }
}

pub fn decoration_sprite(d: &Decoration) -> Sprite {
    match d {
        Decoration::None => overworld::GRASS,
        Decoration::Fern { .. } => overworld::BUSH_GREEN,
        Decoration::Moss => overworld::GRASS_TUFTS,
        Decoration::Bramble { .. } => overworld::BUSH_RASPBERRY,
        Decoration::Bracken { .. } => overworld::TREE_BARE,
        Decoration::Gorse { .. } => overworld::BUSH_YELLOW,
        Decoration::Sapling { .. } => overworld::SAPLING,
        Decoration::Mushroom { .. } => items::MUSHROOM,
    }
}

pub fn item_sprite(kind: ItemKind) -> Sprite {
    use items as I;
    match kind {
        // Tools / containers / survival
        ItemKind::Axe => I::AXE,
        ItemKind::Knife => I::KNIFE,
        ItemKind::Pack => I::BASKET,
        ItemKind::Tent => I::BASKET,
        ItemKind::Bedroll => I::BASKET,
        ItemKind::CookingPan => I::HAMMER,
        ItemKind::Waterskin => I::WATERSKIN,
        ItemKind::FlintAndSteel => I::FLINT_AND_STEEL,
        ItemKind::Herb => I::HERB,
        ItemKind::Twig => I::STICK,
        ItemKind::Stick => I::STICK,
        ItemKind::Firewood => I::WOOD_LOG,
        ItemKind::GrassBlade => I::LEAF_GREEN,
        ItemKind::Stone => I::STONE,
        ItemKind::MossPatch => I::LEAF_GREEN,
        ItemKind::Mud => I::STONE,
        ItemKind::Log => I::WOOD_LOG,
        // Food / forage
        ItemKind::Ration => I::BREAD,
        ItemKind::Fish => I::FISH,
        ItemKind::Cooked => I::MEAT_COOKED,
        ItemKind::Moss => I::LEAF_GREEN,
        ItemKind::FernFrond => I::LEAF_GREEN,
        ItemKind::FernRoot => I::LEAF_BROWN,
        ItemKind::GorseFaggot => I::STICK,
        ItemKind::BrackenStraw => I::LEAF_BROWN,
        ItemKind::BrambleFruit => I::BERRIES,
        ItemKind::Hazelnut => I::NUT_HAZEL,
        ItemKind::Acorn => I::NUT_ACORN,
        ItemKind::RowanBerry => I::BERRIES_RED,
        // Melee weapons
        ItemKind::Spear => I::SPEAR,
        ItemKind::ShortSword => I::SHORTSWORD,
        ItemKind::Falchion => I::FALCHION,
        ItemKind::ArmingSword => I::ARMING_SWORD,
        ItemKind::Lance => I::LANCE,
        ItemKind::Gisarme => I::GISARME,
        ItemKind::Cudgel => I::CUDGEL,
        ItemKind::Quarterstaff => I::QUARTERSTAFF,
        // Ranged
        ItemKind::Bow => I::BOW,
        ItemKind::Arrow => I::ARROW,
        ItemKind::Crossbow => I::CROSSBOW,
        ItemKind::CrossbowBolt => I::BOLT,
        // Armor
        ItemKind::SmallRoundShield => I::SHIELD_ROUND,
        ItemKind::LargeShield => I::SHIELD_LARGE,
        ItemKind::IronSkullcap => I::HELM_SKULLCAP,
        ItemKind::KettleHat => I::HELM_KETTLE,
        ItemKind::MailCoif => I::HELM_COIF,
        ItemKind::GreatHelm => I::HELM_GREAT,
        ItemKind::PaddedDoublet => I::TORSO_DOUBLET,
        ItemKind::LeatherJerkin => I::TORSO_JERKIN,
        ItemKind::Hauberk => I::TORSO_HAUBERK,
        ItemKind::CoatOfPlates => I::TORSO_PLATES,
        ItemKind::MailChausses => I::LEGS_CHAUSSES,
    }
}

// ===========================================================================
// Live overrides — the in-game sprite picker reassigns these; they persist
// in meta.cbor via save::SpriteOverride. The compose loop resolves through
// `Overrides` so an override wins over the registry default.
// ===========================================================================

/// A reassignable render target shown in the sprite picker.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RemapTarget {
    Terrain(TerrainKind),
    Tree(TreeSpecies),
    Item(ItemKind),
}

impl RemapTarget {
    /// Stable save key, e.g. `terrain:grass` / `tree:oak` / `item:axe`.
    pub fn key(self) -> String {
        match self {
            RemapTarget::Terrain(k) => format!("terrain:{}", k.save_key()),
            RemapTarget::Tree(s) => format!("tree:{}", s.save_key()),
            RemapTarget::Item(k) => format!("item:{}", k.save_key()),
        }
    }

    pub fn from_key(s: &str) -> Option<RemapTarget> {
        let (kind, key) = s.split_once(':')?;
        match kind {
            "terrain" => TerrainKind::from_save_key(key).map(RemapTarget::Terrain),
            "tree" => TreeSpecies::from_save_key(key).map(RemapTarget::Tree),
            "item" => ItemKind::from_save_key(key).map(RemapTarget::Item),
            _ => None,
        }
    }

    /// Human-readable label for the picker list.
    pub fn name(self) -> &'static str {
        match self {
            RemapTarget::Terrain(k) => k.def().name,
            RemapTarget::Tree(s) => s.save_key(),
            RemapTarget::Item(k) => k.def().name,
        }
    }

    /// Registry default sprite, ignoring overrides.
    pub fn default_sprite(self) -> Sprite {
        match self {
            RemapTarget::Terrain(k) => terrain_sprite(k),
            RemapTarget::Tree(s) => tree_sprite(s),
            RemapTarget::Item(k) => item_sprite(k),
        }
    }

    /// Targets for the picker's "Terrain" tab (terrain kinds + tree species).
    pub fn terrain_tab() -> Vec<RemapTarget> {
        REMAPPABLE_TERRAINS
            .iter()
            .map(|&k| RemapTarget::Terrain(k))
            .chain(ALL_TREE_SPECIES.iter().map(|&s| RemapTarget::Tree(s)))
            .collect()
    }

    /// Targets for the picker's "Items" tab.
    pub fn item_tab() -> Vec<RemapTarget> {
        ItemKind::all().iter().map(|&k| RemapTarget::Item(k)).collect()
    }
}

/// Per-target sprite overrides, keyed by `RemapTarget::key()`.
#[derive(Default, Clone)]
pub struct Overrides {
    map: HashMap<String, Sprite>,
}

impl Overrides {
    pub fn from_save(list: &[SpriteOverride]) -> Self {
        let mut map = HashMap::new();
        for o in list {
            // Drop unknown targets / sheets (forward-compat).
            if RemapTarget::from_key(&o.target).is_none() {
                continue;
            }
            let Some(sheet) = Sheet::from_name(&o.sheet) else {
                continue;
            };
            map.insert(o.target.clone(), Sprite::at(sheet, o.col, o.row));
        }
        Self { map }
    }

    pub fn to_save(&self) -> Vec<SpriteOverride> {
        let mut v: Vec<SpriteOverride> = self
            .map
            .iter()
            .map(|(target, s)| SpriteOverride {
                target: target.clone(),
                sheet: s.sheet.name().to_string(),
                col: s.col,
                row: s.row,
            })
            .collect();
        // Deterministic order so saves don't churn on rewrite.
        v.sort_by(|a, b| a.target.cmp(&b.target));
        v
    }

    pub fn get(&self, t: RemapTarget) -> Option<Sprite> {
        self.map.get(&t.key()).copied()
    }

    pub fn set(&mut self, t: RemapTarget, s: Sprite) {
        self.map.insert(t.key(), s);
    }

    pub fn clear(&mut self, t: RemapTarget) {
        self.map.remove(&t.key());
    }

    /// Override if set, else the registry default.
    pub fn resolve(&self, t: RemapTarget) -> Sprite {
        self.get(t).unwrap_or_else(|| t.default_sprite())
    }

    // Compose-loop conveniences.
    pub fn terrain(&self, k: TerrainKind) -> Sprite {
        self.resolve(RemapTarget::Terrain(k))
    }
    pub fn tree(&self, s: TreeSpecies) -> Sprite {
        self.resolve(RemapTarget::Tree(s))
    }
    pub fn item(&self, k: ItemKind) -> Sprite {
        self.resolve(RemapTarget::Item(k))
    }
}
