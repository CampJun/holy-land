// Item identity, weights, metadata, and player pack. Phase 3 introduces this
// module from scratch; the previous Holy Land Inventory/Item enum lived in
// world.rs and was gutted in phase 2.
//
// Design notes (see Survival - ItemInstance and weights.md, Survival - Chunk
// and per-cell items.md):
// - `ItemKind` partitions into fungible (twig, stick, ...) and unique (axe,
//   waterskin, ...). Fungibles stack into a single `ItemInstance` with
//   `count >= 1`; uniques always have `count == 1` and carry per-instance
//   metadata.
// - Weight is per-unit (`weight_g_each`); total stack weight is `count *
//   weight_g_each`. Source of truth: `ItemInstance::total_weight_g`.
// - Save format uses stable string `save_key()`s for forward-compat. Unknown
//   keys on load are dropped silently (see `Pack::from_save`).

use crate::combat::{ArmorDr, BodyPart, DamageTriplet, RangedProfile, WeaponProfile};
use crate::crafting::{CookableKind, CookedState, PanContents, Seasonings};
use crate::save::{ItemInstanceSave, ItemMetadataSave, PackSave};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Axe,
    Knife,
    Pack,
    Tent,
    Bedroll,
    CookingPan,
    Waterskin,
    FlintAndSteel,
    Herb,
    Twig,
    Stick,
    Firewood,
    GrassBlade,
    Stone,
    MossPatch,
    Mud,
    Ration,
    Fish,
    /// Output of cooking. All variants (cooked fish, herbed cooked
    /// fish, burnt fish, ...) share this single kind; per-output
    /// flavor lives in `ItemMetadata::Cooked { base, state, seasonings }`.
    /// Display_label spells out the human-readable name.
    Cooked,
    // ---- Phase C: undergrowth harvest yields + tree mast ----
    Moss,
    FernFrond,
    FernRoot,
    /// Gorse cuttings bundled for fire — kindling, burns hot+fast.
    GorseFaggot,
    BrackenStraw,
    BrambleFruit,
    Hazelnut,
    Acorn,
    RowanBerry,
    /// Two-handed thrusting weapon. Yeoman-tier main_hand for the
    /// Cornish bandit (50% of rolls per `Bestiary slice 1.md`).
    /// Combat profile lives on `ItemDef.weapon`.
    Spear,
    /// One-handed arming sword. Yeoman / Sergeant-tier main_hand.
    /// Balanced cut + stab.
    ShortSword,
    /// Single-edged cleaver, cut-heavy. Yeoman alternate to ShortSword.
    Falchion,
    /// Off-hand defensive piece. Phase 3 carries it as inventory only;
    /// the block-chance bonus lands in a later phase per the cards.
    SmallRoundShield,
    /// Iron skullcap — cheap head protection, Yeoman tier (70% roll).
    IronSkullcap,
    /// Padded fabric torso + arms armor. Yeoman tier (80% torso roll).
    PaddedDoublet,
    /// Cheaper torso-only armor. Yeoman tier (20% torso roll).
    LeatherJerkin,
    /// Drawn-string bow. Period-correct ranged weapon; 20% chance per
    /// Yeoman bandit spawn (per `Bestiary slice 1.md`). Consumes one
    /// `Arrow` from the attacker's pack per shot.
    Bow,
    /// Stackable bow ammo. Consumed per shot; drops on hit target's
    /// cell for ~70% recovery (phase 6 placeholder break rule).
    Arrow,
    // ---- Phase 8: Sergeant + Knight tiers ----
    /// One-handed arming sword. Sergeant-tier main_hand (75% roll per
    /// Status armament tiers.md). Slightly better balance than the
    /// Yeoman short sword.
    ArmingSword,
    /// Two-handed reach-2 thrusting lance. Knight-tier main_hand. Best
    /// stab damage in the game; awkward at adjacent (no-reach penalty).
    Lance,
    /// Mail shirt covering torso + arms. The signature elite armor of
    /// the period; rolls on Sergeant + Knight loadouts.
    Hauberk,
    /// Mail leggings — legs. Knight tier.
    MailChausses,
    /// Mail hood — head. Knight tier (often under a great helm).
    MailCoif,
    /// Iron / steel wide-brim helm. Sergeant tier (50% head roll).
    KettleHat,
    /// Heavy enclosed cavalry helm. Knight tier.
    GreatHelm,
    /// Riveted small plates over fabric — the bleeding-edge transition
    /// armor of ~1300. Knight tier (40% roll, layered over Hauberk).
    CoatOfPlates,
    /// Large kite/round shield — Knight off-hand.
    LargeShield,
    // ---- PR A card 5: Rabble melee ----
    /// One-handed bash weapon — commoner blunt per the Statute of
    /// Winchester rabble class. Trains Mace/Cudgel proficiency.
    Cudgel,
    /// Two-handed wooden pole; pure bash. Commoner training weapon.
    Quarterstaff,
    // ---- PR A card 4: ranged + final polearm ----
    /// Sickle-bladed two-handed polearm — reach-2 cut+stab. The period
    /// polearm proper per the Weapon-skills card; replaces the older
    /// halberd/bardiche niche. No tier currently rolls it; available
    /// via debug `give` and future authored loadouts.
    Gisarme,
    /// Mechanical-trigger ranged weapon. Slow draw + a separate Reload
    /// verb; holds one bolt loaded at a time. Higher damage per shot
    /// than the bow, with a stab + bash damage profile.
    Crossbow,
    /// Stackable crossbow ammo. Consumed per shot; same 70/30 recovery
    /// shape as `Arrow`.
    CrossbowBolt,
}

/// All per-kind metadata in one place. Adding a new `ItemKind` variant is
/// a one-stop edit: extend `ItemKind`, then extend `def()` with a new
/// arm. The compiler's exhaustive-match check enforces both halves.
pub struct ItemDef {
    pub save_key: &'static str,
    pub name: &'static str,
    pub is_fungible: bool,
    pub glyph: u8,
    pub color: [u8; 3],
    /// Weight per unit (grams) when a freshly-constructed default
    /// instance is created (e.g. debug `give` command). Waterskins
    /// here are EMPTY (200g, water_uses=0); a full one is heavier and
    /// is constructed explicitly by `starting_pack`.
    pub default_weight_g: u32,
    /// Aesthetic flag: organic detritus (twigs, grass, moss, mud) reads
    /// as part of the floor texture when rendered on the ground —
    /// main.rs mixes the item color heavily toward the cell's terrain
    /// fg so the eye glides past it. Distinct items (stones, firewood,
    /// herbs, tools, structures) keep their saturation so they pierce
    /// the floor as visual landmarks.
    pub blends_with_terrain: bool,
    /// Weapon stats — `Some` for anything an entity can wield in melee.
    /// Phase-3 ships melee only; ranged ammo / bow weapons get a sibling
    /// `ranged: Option<RangedProfile>` in phase 5.
    pub weapon: Option<WeaponProfile>,
    /// Armor stats + body-region mask + coverage% + encumbrance per
    /// covered region. `Some` for anything wearable; phase-3 reads this
    /// to build the bandit's `Worn` from its rolled loadout instead of
    /// hand-assembling pieces in world.rs.
    pub armor: Option<ArmorStats>,
    /// Ranged profile — `Some` for bows / crossbows. The Aim verb
    /// surfaces only when the main-hand item has this set.
    pub ranged: Option<RangedProfile>,
}

/// Static armor description for an `ItemKind`. The wearable mask
/// determines which body regions a single equipped piece covers; the
/// coverage % drives the per-hit catch roll; DR feeds the per-type
/// subtraction; encumbrance is added once per covered region for the
/// Dodge / move-cost penalties.
#[derive(Clone, Copy, Debug)]
pub struct ArmorStats {
    pub regions: &'static [BodyPart],
    pub coverage_pct: u8,
    pub dr: ArmorDr,
    pub encumbrance: u8,
}

/// Iteration order used by `from_save_key` and tests. Keep in sync with
/// the `ItemKind` enum variants.
const ALL_KINDS: &[ItemKind] = &[
    ItemKind::Axe,
    ItemKind::Knife,
    ItemKind::Pack,
    ItemKind::Tent,
    ItemKind::Bedroll,
    ItemKind::CookingPan,
    ItemKind::Waterskin,
    ItemKind::FlintAndSteel,
    ItemKind::Herb,
    ItemKind::Twig,
    ItemKind::Stick,
    ItemKind::Firewood,
    ItemKind::GrassBlade,
    ItemKind::Stone,
    ItemKind::MossPatch,
    ItemKind::Mud,
    ItemKind::Ration,
    ItemKind::Fish,
    ItemKind::Cooked,
    ItemKind::Moss,
    ItemKind::FernFrond,
    ItemKind::FernRoot,
    ItemKind::GorseFaggot,
    ItemKind::BrackenStraw,
    ItemKind::BrambleFruit,
    ItemKind::Hazelnut,
    ItemKind::Acorn,
    ItemKind::RowanBerry,
    ItemKind::Spear,
    ItemKind::ShortSword,
    ItemKind::Falchion,
    ItemKind::SmallRoundShield,
    ItemKind::IronSkullcap,
    ItemKind::PaddedDoublet,
    ItemKind::LeatherJerkin,
    ItemKind::Bow,
    ItemKind::Arrow,
    ItemKind::ArmingSword,
    ItemKind::Lance,
    ItemKind::Hauberk,
    ItemKind::MailChausses,
    ItemKind::MailCoif,
    ItemKind::KettleHat,
    ItemKind::GreatHelm,
    ItemKind::CoatOfPlates,
    ItemKind::LargeShield,
    ItemKind::Gisarme,
    ItemKind::Crossbow,
    ItemKind::CrossbowBolt,
    ItemKind::Cudgel,
    ItemKind::Quarterstaff,
];

impl ItemKind {
    /// Single source of truth for per-kind metadata. The match is
    /// exhaustive — adding a new `ItemKind` variant is a compile error
    /// until this gains an arm.
    pub fn def(self) -> ItemDef {
        match self {
            ItemKind::Axe => ItemDef {
                save_key: "axe",
                name: "axe",
                is_fungible: false,
                glyph: b'P',
                color: [180, 180, 200],
                default_weight_g: 1_000,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 0,
                    damage_die: DamageTriplet { bash: 3, cut: 9, stab: 0 },
                    move_cost: 130,
                    reach: 1,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::Knife => ItemDef {
                save_key: "knife",
                name: "knife",
                is_fungible: false,
                glyph: b'-',
                color: [180, 180, 200],
                default_weight_g: 200,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 1,
                    damage_die: DamageTriplet { bash: 0, cut: 2, stab: 8 },
                    move_cost: 70,
                    reach: 1,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::Pack => ItemDef {
                save_key: "pack",
                name: "pack",
                is_fungible: false,
                glyph: b'[',
                color: [130, 90, 50],
                default_weight_g: 1_000,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Tent => ItemDef {
                save_key: "tent",
                name: "tent",
                is_fungible: false,
                glyph: 0x1E,
                color: [200, 180, 140],
                default_weight_g: 5_000,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Bedroll => ItemDef {
                save_key: "bedroll",
                name: "bedroll",
                is_fungible: false,
                glyph: b'=',
                color: [220, 200, 160],
                default_weight_g: 2_000,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::CookingPan => ItemDef {
                save_key: "cooking_pan",
                name: "cooking pan",
                is_fungible: false,
                glyph: b'O',
                color: [80, 80, 90],
                default_weight_g: 1_000,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Waterskin => ItemDef {
                save_key: "waterskin",
                name: "waterskin",
                is_fungible: false,
                glyph: b'u',
                color: [100, 140, 200],
                default_weight_g: 200,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::FlintAndSteel => ItemDef {
                save_key: "flint_and_steel",
                name: "flint and steel",
                is_fungible: false,
                glyph: b'!',
                color: [230, 140, 60],
                default_weight_g: 100,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Herb => ItemDef {
                save_key: "herb",
                name: "herb",
                is_fungible: false,
                glyph: 0xE7,
                // Saturated green tints the grayscale atlas sprite.
                // Atlas pixel * fg / 255 → shaded green herb.
                color: [100, 165, 75],
                default_weight_g: 10,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            // Organic detritus: blends into the floor texture so the
            // eye glides past it. ChopTree drops firewood (which
            // pierces) so material is still visible.
            ItemKind::Twig => ItemDef {
                save_key: "twig",
                name: "twig",
                is_fungible: true,
                glyph: b',',
                color: [200, 170, 110],
                default_weight_g: 5,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Stick => ItemDef {
                save_key: "stick",
                name: "stick",
                is_fungible: true,
                glyph: b'/',
                color: [200, 170, 110],
                default_weight_g: 50,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Firewood => ItemDef {
                save_key: "firewood",
                name: "firewood",
                is_fungible: true,
                glyph: 0x16,
                // Saturated wood-brown tints the grayscale log-pile.
                color: [150, 100, 55],
                default_weight_g: 500,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::GrassBlade => ItemDef {
                save_key: "grass_blade",
                name: "grass blade",
                is_fungible: true,
                glyph: b'"',
                color: [80, 160, 70],
                default_weight_g: 2,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Stone => ItemDef {
                save_key: "stone",
                name: "stone",
                is_fungible: true,
                glyph: 0x07,
                // Cool gray — the atlas's shading still shows the
                // rounded silhouette; this multiplier keeps it from
                // looking too white.
                color: [165, 165, 175],
                default_weight_g: 200,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::MossPatch => ItemDef {
                save_key: "moss_patch",
                name: "moss patch",
                is_fungible: true,
                glyph: b'%',
                color: [50, 100, 50],
                default_weight_g: 10,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Mud => ItemDef {
                save_key: "mud",
                name: "mud",
                is_fungible: true,
                glyph: b'%',
                color: [110, 80, 50],
                default_weight_g: 300,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Ration => ItemDef {
                save_key: "ration",
                name: "ration",
                is_fungible: true,
                glyph: 0xE0,
                // Warm tan/skin — tints the grayscale chicken sprite.
                color: [225, 180, 110],
                default_weight_g: 500,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Fish => ItemDef {
                save_key: "fish",
                name: "fish",
                is_fungible: true,
                // Reuses the small-letter F glyph; no fish sprite in
                // the atlas yet. Cool blue-grey reads "raw fish."
                glyph: b'f',
                color: [140, 170, 210],
                default_weight_g: 400,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Cooked => ItemDef {
                save_key: "cooked",
                name: "cooked food",
                // Per-instance state lives in metadata; treat as
                // non-fungible so the (base, seasonings, state) triple
                // never silently stack-merges across permutations.
                is_fungible: false,
                glyph: b'%',
                color: [200, 150, 90],
                default_weight_g: 400,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            // Phase C: undergrowth harvest yields.
            ItemKind::Moss => ItemDef {
                save_key: "moss",
                name: "moss",
                is_fungible: true,
                glyph: 0x07,
                color: [80, 130, 80],
                default_weight_g: 10,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::FernFrond => ItemDef {
                save_key: "fern_frond",
                name: "fern frond",
                is_fungible: true,
                glyph: 0xF0,
                color: [90, 145, 70],
                default_weight_g: 15,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::FernRoot => ItemDef {
                save_key: "fern_root",
                name: "fern root",
                is_fungible: true,
                glyph: b'/',
                color: [140, 100, 60],
                default_weight_g: 40,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::GorseFaggot => ItemDef {
                save_key: "gorse_faggot",
                name: "gorse faggot",
                is_fungible: true,
                glyph: 0x16,
                color: [200, 170, 60],
                default_weight_g: 250,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::BrackenStraw => ItemDef {
                save_key: "bracken_straw",
                name: "bracken straw",
                is_fungible: true,
                glyph: b'"',
                color: [160, 110, 50],
                default_weight_g: 5,
                blends_with_terrain: true,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::BrambleFruit => ItemDef {
                save_key: "bramble_fruit",
                name: "bramble fruit",
                is_fungible: true,
                glyph: 0xFA,
                color: [140, 60, 95],
                default_weight_g: 25,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Hazelnut => ItemDef {
                save_key: "hazelnut",
                name: "hazelnut",
                is_fungible: true,
                glyph: b'o',
                color: [180, 130, 70],
                default_weight_g: 8,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Acorn => ItemDef {
                save_key: "acorn",
                name: "acorn",
                is_fungible: true,
                glyph: b'o',
                color: [150, 110, 60],
                default_weight_g: 12,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::RowanBerry => ItemDef {
                save_key: "rowan_berry",
                name: "rowan berry",
                is_fungible: true,
                glyph: 0xFA,
                color: [200, 70, 50],
                default_weight_g: 10,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Spear => ItemDef {
                save_key: "spear",
                name: "spear",
                is_fungible: false,
                glyph: b'/',
                color: [170, 140, 90],
                default_weight_g: 1_800,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 1,
                    damage_die: DamageTriplet { bash: 1, cut: 0, stab: 9 },
                    move_cost: 110,
                    // Reach-2 polearm. Player + bandit can swing at a
                    // hostile 2 cells away in a cardinal/diagonal line
                    // (subject to LoS). Adjacent swings take the
                    // NO_REACH_DAMAGE_PCT penalty per the cards.
                    reach: 2,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::ShortSword => ItemDef {
                save_key: "short_sword",
                name: "short sword",
                is_fungible: false,
                glyph: b'(',
                color: [200, 200, 215],
                default_weight_g: 1_100,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 2,
                    damage_die: DamageTriplet { bash: 1, cut: 6, stab: 6 },
                    move_cost: 90,
                    reach: 1,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::Falchion => ItemDef {
                save_key: "falchion",
                name: "falchion",
                is_fungible: false,
                glyph: b')',
                color: [195, 180, 140],
                default_weight_g: 1_400,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 1,
                    damage_die: DamageTriplet { bash: 2, cut: 9, stab: 1 },
                    move_cost: 100,
                    reach: 1,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::SmallRoundShield => ItemDef {
                save_key: "small_round_shield",
                name: "small round shield",
                is_fungible: false,
                glyph: b'o',
                color: [150, 95, 55],
                default_weight_g: 2_000,
                blends_with_terrain: false,
                // Shields carry the block bonus in a later phase; for
                // phase 3 they're inventory ballast that can drop on
                // death.
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::IronSkullcap => ItemDef {
                save_key: "iron_skullcap",
                name: "iron skullcap",
                is_fungible: false,
                glyph: b'^',
                color: [160, 160, 175],
                default_weight_g: 900,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Head],
                    coverage_pct: 70,
                    dr: ArmorDr { bash: 3, cut: 4, stab: 3 },
                    encumbrance: 1,
                }),
                ranged: None,
            },
            ItemKind::PaddedDoublet => ItemDef {
                save_key: "padded_doublet",
                name: "padded doublet",
                is_fungible: false,
                glyph: b'[',
                color: [180, 150, 110],
                default_weight_g: 2_500,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Torso, BodyPart::LArm, BodyPart::RArm],
                    coverage_pct: 80,
                    dr: ArmorDr { bash: 4, cut: 2, stab: 1 },
                    // Per-region enc 1 (sums to 3 across torso + arms) —
                    // padded is the lightest tier; mail bumps this to 2
                    // and plate to 3 when those land in a later phase.
                    encumbrance: 1,
                }),
                ranged: None,
            },
            ItemKind::LeatherJerkin => ItemDef {
                save_key: "leather_jerkin",
                name: "leather jerkin",
                is_fungible: false,
                glyph: b'[',
                color: [130, 90, 55],
                default_weight_g: 1_800,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Torso],
                    coverage_pct: 70,
                    dr: ArmorDr { bash: 3, cut: 2, stab: 2 },
                    encumbrance: 1,
                }),
                ranged: None,
            },
            ItemKind::Bow => ItemDef {
                save_key: "bow",
                name: "bow",
                is_fungible: false,
                glyph: b')',
                color: [150, 110, 70],
                default_weight_g: 800,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: Some(RangedProfile {
                    to_hit: 2,
                    // Stab-heavy; the arrow itself does the work.
                    damage_die: DamageTriplet { bash: 0, cut: 1, stab: 10 },
                    move_cost: 130,
                    max_range: 10,
                    ammo_kind: "arrow",
                }),
            },
            ItemKind::Arrow => ItemDef {
                save_key: "arrow",
                name: "arrow",
                is_fungible: true,
                glyph: b'-',
                color: [180, 150, 100],
                default_weight_g: 40,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            // ---- Phase 8 Sergeant + Knight items ----
            ItemKind::ArmingSword => ItemDef {
                save_key: "arming_sword",
                name: "arming sword",
                is_fungible: false,
                glyph: b'(',
                color: [210, 210, 225],
                default_weight_g: 1_200,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 2,
                    damage_die: DamageTriplet { bash: 1, cut: 7, stab: 7 },
                    move_cost: 90,
                    reach: 1,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::Lance => ItemDef {
                save_key: "lance",
                name: "lance",
                is_fungible: false,
                glyph: b'|',
                color: [180, 150, 100],
                default_weight_g: 2_400,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 2,
                    damage_die: DamageTriplet { bash: 2, cut: 1, stab: 12 },
                    move_cost: 130,
                    reach: 2,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::Hauberk => ItemDef {
                save_key: "hauberk",
                name: "mail hauberk",
                is_fungible: false,
                glyph: b'[',
                color: [160, 165, 180],
                default_weight_g: 11_000,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Torso, BodyPart::LArm, BodyPart::RArm],
                    coverage_pct: 90,
                    // Mail: low bash, high cut, medium stab.
                    dr: ArmorDr { bash: 2, cut: 8, stab: 4 },
                    encumbrance: 2,
                }),
                ranged: None,
            },
            ItemKind::MailChausses => ItemDef {
                save_key: "mail_chausses",
                name: "mail chausses",
                is_fungible: false,
                glyph: b'[',
                color: [155, 160, 175],
                default_weight_g: 5_500,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::LLeg, BodyPart::RLeg],
                    coverage_pct: 85,
                    dr: ArmorDr { bash: 2, cut: 7, stab: 3 },
                    encumbrance: 2,
                }),
                ranged: None,
            },
            ItemKind::MailCoif => ItemDef {
                save_key: "mail_coif",
                name: "mail coif",
                is_fungible: false,
                glyph: b'^',
                color: [150, 155, 170],
                default_weight_g: 1_400,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Head],
                    coverage_pct: 80,
                    dr: ArmorDr { bash: 2, cut: 7, stab: 3 },
                    encumbrance: 1,
                }),
                ranged: None,
            },
            ItemKind::KettleHat => ItemDef {
                save_key: "kettle_hat",
                name: "kettle hat",
                is_fungible: false,
                glyph: b'^',
                color: [170, 170, 180],
                default_weight_g: 1_500,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Head],
                    coverage_pct: 75,
                    dr: ArmorDr { bash: 5, cut: 5, stab: 4 },
                    encumbrance: 1,
                }),
                ranged: None,
            },
            ItemKind::GreatHelm => ItemDef {
                save_key: "great_helm",
                name: "great helm",
                is_fungible: false,
                glyph: b'^',
                color: [200, 200, 210],
                default_weight_g: 3_200,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Head],
                    coverage_pct: 90,
                    dr: ArmorDr { bash: 7, cut: 7, stab: 7 },
                    encumbrance: 3,
                }),
                ranged: None,
            },
            ItemKind::CoatOfPlates => ItemDef {
                save_key: "coat_of_plates",
                name: "coat-of-plates",
                is_fungible: false,
                glyph: b'[',
                color: [180, 175, 165],
                default_weight_g: 8_500,
                blends_with_terrain: false,
                weapon: None,
                armor: Some(ArmorStats {
                    regions: &[BodyPart::Torso],
                    coverage_pct: 95,
                    // Plate: high all three; heavy enc.
                    dr: ArmorDr { bash: 7, cut: 8, stab: 7 },
                    encumbrance: 3,
                }),
                ranged: None,
            },
            ItemKind::LargeShield => ItemDef {
                save_key: "large_shield",
                name: "large shield",
                is_fungible: false,
                glyph: b'O',
                color: [130, 80, 50],
                default_weight_g: 4_000,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            // ---- PR A card 4 items ----
            ItemKind::Gisarme => ItemDef {
                save_key: "gisarme",
                name: "gisarme",
                is_fungible: false,
                glyph: b'/',
                color: [180, 165, 130],
                default_weight_g: 2_600,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 1,
                    // Sickle-bladed cut + stab, slow swing.
                    damage_die: DamageTriplet { bash: 1, cut: 6, stab: 7 },
                    move_cost: 140,
                    reach: 2,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::Crossbow => ItemDef {
                save_key: "crossbow",
                name: "crossbow",
                is_fungible: false,
                glyph: b'}',
                color: [140, 110, 80],
                default_weight_g: 3_500,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: Some(RangedProfile {
                    to_hit: 4,
                    // Stab + bash — the bolt's impact has heft. Higher
                    // damage per shot than the bow per the card; firing
                    // itself is fast (move_cost 90), the slow part is
                    // the Reload verb the player runs between shots.
                    damage_die: DamageTriplet { bash: 4, cut: 0, stab: 14 },
                    move_cost: 90,
                    max_range: 12,
                    ammo_kind: "crossbow_bolt",
                }),
            },
            ItemKind::CrossbowBolt => ItemDef {
                save_key: "crossbow_bolt",
                name: "crossbow bolt",
                is_fungible: true,
                glyph: b'-',
                color: [170, 140, 100],
                default_weight_g: 70,
                blends_with_terrain: false,
                weapon: None,
                armor: None,
                ranged: None,
            },
            ItemKind::Cudgel => ItemDef {
                save_key: "cudgel",
                name: "cudgel",
                is_fungible: false,
                glyph: b'!',
                color: [140, 100, 60],
                default_weight_g: 1_400,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 0,
                    // Pure bash; a worked stick with a knob on the end.
                    damage_die: DamageTriplet { bash: 8, cut: 0, stab: 0 },
                    move_cost: 90,
                    reach: 1,
                }),
                armor: None,
                ranged: None,
            },
            ItemKind::Quarterstaff => ItemDef {
                save_key: "quarterstaff",
                name: "quarterstaff",
                is_fungible: false,
                glyph: b'|',
                color: [150, 120, 80],
                default_weight_g: 2_000,
                blends_with_terrain: false,
                weapon: Some(WeaponProfile {
                    to_hit: 1,
                    // Two-handed bash; modest reach but still adjacent
                    // for v1 — the card notes "modest reach (still
                    // adjacent though)."
                    damage_die: DamageTriplet { bash: 9, cut: 0, stab: 0 },
                    move_cost: 110,
                    reach: 1,
                }),
                armor: None,
                ranged: None,
            },
        }
    }

    /// Build a freshly-constructed `ItemInstance` of this kind with
    /// default metadata + per-unit weight from `def()`. Fungibles use
    /// `count`; uniques always get count=1 regardless of what the
    /// caller passed (uniques don't stack). Waterskins come back
    /// EMPTY (water_uses=0); use `starting_pack` to make a full one.
    pub fn make_default_instance(self, count: u16) -> ItemInstance {
        let d = self.def();
        let metadata = match self {
            ItemKind::Waterskin => ItemMetadata::Waterskin { water_uses: 0 },
            _ => ItemMetadata::None,
        };
        if d.is_fungible {
            ItemInstance::stack(self, count.max(1), d.default_weight_g, None, metadata)
        } else {
            ItemInstance::unique(self, d.default_weight_g, None, metadata)
        }
    }

    pub fn save_key(self) -> &'static str {
        self.def().save_key
    }

    pub fn from_save_key(s: &str) -> Option<Self> {
        ALL_KINDS.iter().copied().find(|k| k.def().save_key == s)
    }

    pub fn is_fungible(self) -> bool {
        self.def().is_fungible
    }

    #[allow(dead_code)] // used by the command-menu (phase 7) and HUD inventory panel
    pub fn name(self) -> &'static str {
        self.def().name
    }

    /// Glyph + RGB foreground color for ground rendering. Background uses
    /// the cell's terrain background so items sit "on" the floor visually.
    pub fn glyph_color(self) -> (u8, [u8; 3]) {
        let d = self.def();
        (d.glyph, d.color)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemMetadata {
    None,
    Waterskin { water_uses: u8 },
    /// The item is placed in the world (pitched tent, unrolled bedroll).
    /// Renders identically to a normal ItemInstance via the glyph table
    /// but never stack-merges, and `try_add` keeps it distinct from
    /// pack-stored counterparts. Phase 13 reads this flag for warmth
    /// shelter detection.
    Pitched,
    /// A lit fire. `fuel_seconds` decrements via `World::tick_fires`
    /// each game-second; reaches 0 -> item is removed from the cell.
    /// Phase 12 adds "feed fire" verb to top up; phase 13 reads this
    /// flag for warmth shelter and night-vision-radius extension.
    Lit {
        fuel_seconds: u32,
    },
    /// A CookingPan placed on a Lit fire. Owns the fire's remaining
    /// fuel so the pan-and-fire act as one item on the cell — pickup
    /// hands the fuel back to a Lit firewood. The cookware tick (in
    /// world.rs) drives `fuel_seconds` down and `PanContents::Cooking`
    /// forward each game-second.
    PannedOnFire {
        contents: PanContents,
        fuel_seconds: u32,
    },
    /// Cooked food. Single `ItemKind::Cooked` covers every permutation
    /// of (base, state, seasonings) — no `CookedFish` / `BurntFish` /
    /// `HerbCookedFish` explosion. `display_label` spells out the
    /// readable name; eat-effects (future verb) read the bitfield.
    Cooked {
        base: CookableKind,
        state: CookedState,
        seasonings: Seasonings,
    },
}

impl Default for ItemMetadata {
    fn default() -> Self {
        ItemMetadata::None
    }
}

impl ItemMetadata {
    pub fn to_save(&self) -> ItemMetadataSave {
        match *self {
            ItemMetadata::None => ItemMetadataSave::None,
            ItemMetadata::Waterskin { water_uses } => ItemMetadataSave::Waterskin { water_uses },
            ItemMetadata::Pitched => ItemMetadataSave::Pitched,
            ItemMetadata::Lit { fuel_seconds } => ItemMetadataSave::Lit { fuel_seconds },
            ItemMetadata::PannedOnFire {
                contents,
                fuel_seconds,
            } => {
                let (kind, input, elapsed_secs, seasonings) = match contents {
                    PanContents::Empty => (String::from("empty"), String::new(), 0, 0),
                    PanContents::Cooking {
                        input,
                        elapsed_secs,
                        seasonings,
                    } => (
                        String::from("cooking"),
                        input.save_key().to_string(),
                        elapsed_secs,
                        seasonings.0,
                    ),
                };
                ItemMetadataSave::PannedOnFire {
                    contents_kind: kind,
                    input,
                    elapsed_secs,
                    seasonings,
                    fuel_seconds,
                }
            }
            ItemMetadata::Cooked {
                base,
                state,
                seasonings,
            } => ItemMetadataSave::Cooked {
                base: base.save_key().to_string(),
                state: state.save_key().to_string(),
                seasonings: seasonings.0,
            },
        }
    }

    pub fn from_save(s: &ItemMetadataSave) -> Self {
        match s {
            ItemMetadataSave::None => ItemMetadata::None,
            ItemMetadataSave::Waterskin { water_uses } => ItemMetadata::Waterskin {
                water_uses: *water_uses,
            },
            ItemMetadataSave::Pitched => ItemMetadata::Pitched,
            ItemMetadataSave::Lit { fuel_seconds } => ItemMetadata::Lit {
                fuel_seconds: *fuel_seconds,
            },
            ItemMetadataSave::PannedOnFire {
                contents_kind,
                input,
                elapsed_secs,
                seasonings,
                fuel_seconds,
            } => {
                let contents = match contents_kind.as_str() {
                    "cooking" => match CookableKind::from_save_key(input) {
                        Some(k) => PanContents::Cooking {
                            input: k,
                            elapsed_secs: *elapsed_secs,
                            seasonings: Seasonings(*seasonings),
                        },
                        // Unknown future cookable -> drop to empty.
                        None => PanContents::Empty,
                    },
                    _ => PanContents::Empty,
                };
                ItemMetadata::PannedOnFire {
                    contents,
                    fuel_seconds: *fuel_seconds,
                }
            }
            ItemMetadataSave::Cooked {
                base,
                state,
                seasonings,
            } => {
                // Unknown base/state -> fall back to a recognisable
                // sentinel. The item is still loadable; the player
                // sees "cooked food" with no flavor.
                let base = CookableKind::from_save_key(base).unwrap_or(CookableKind::Fish);
                let state = CookedState::from_save_key(state).unwrap_or(CookedState::Ok);
                ItemMetadata::Cooked {
                    base,
                    state,
                    seasonings: Seasonings(*seasonings),
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ItemInstance {
    pub kind: ItemKind,
    pub count: u16,
    pub weight_g_each: u32,
    pub charges: Option<u16>,
    pub metadata: ItemMetadata,
}

impl ItemInstance {
    pub fn unique(
        kind: ItemKind,
        weight_g: u32,
        charges: Option<u16>,
        metadata: ItemMetadata,
    ) -> Self {
        Self {
            kind,
            count: 1,
            weight_g_each: weight_g,
            charges,
            metadata,
        }
    }

    pub fn stack(
        kind: ItemKind,
        count: u16,
        weight_g_each: u32,
        charges: Option<u16>,
        metadata: ItemMetadata,
    ) -> Self {
        Self {
            kind,
            count,
            weight_g_each,
            charges,
            metadata,
        }
    }

    pub fn total_weight_g(&self) -> u32 {
        (self.weight_g_each as u64)
            .saturating_mul(self.count as u64)
            .min(u32::MAX as u64) as u32
    }

    /// One-line display label suitable for any list UI (here-line,
    /// inventory menu, future drop dialog). Encapsulates the four
    /// suffix cases — Pitched, Lit, Waterskin uses, plain stack count
    /// — so callers don't reinvent them.
    pub fn display_label(&self) -> String {
        let name = self.kind.name();
        match self.metadata {
            ItemMetadata::Pitched => format!("{} (pitched)", name),
            ItemMetadata::Lit { fuel_seconds } => {
                format!("{} (lit, {}m)", name, fuel_seconds / 60)
            }
            ItemMetadata::Waterskin { water_uses } => match water_uses {
                0 => format!("{} (empty)", name),
                n => format!("{} ({}/4)", name, n),
            },
            ItemMetadata::PannedOnFire {
                contents,
                fuel_seconds,
            } => {
                let m = fuel_seconds / 60;
                match contents {
                    PanContents::Empty => format!("pan-on-fire ({}m fuel)", m),
                    PanContents::Cooking {
                        input,
                        elapsed_secs,
                        seasonings: _,
                    } => {
                        let status = match crate::crafting::cook_progress(input, elapsed_secs) {
                            crate::crafting::CookProgress::Raw => {
                                let pct = (elapsed_secs * 100 / input.target_secs().max(1)).min(99);
                                format!("cooking {} ({}%)", input.raw_name(), pct)
                            }
                            crate::crafting::CookProgress::Ok => {
                                format!("done {}", input.raw_name())
                            }
                            crate::crafting::CookProgress::Burnt => {
                                format!("BURNT {}", input.raw_name())
                            }
                        };
                        format!("pan-on-fire ({}, {}m fuel)", status, m)
                    }
                }
            }
            ItemMetadata::Cooked {
                base,
                state,
                seasonings,
            } => {
                let base_name = base.raw_name();
                let prefix = if seasonings.has(crate::crafting::ModifierTag::Herb) {
                    "herbed "
                } else {
                    ""
                };
                match state {
                    CookedState::Ok => format!("{}cooked {}", prefix, base_name),
                    CookedState::Burnt => format!("burnt {}", base_name),
                }
            }
            ItemMetadata::None if self.count > 1 => format!("{} ({})", name, self.count),
            ItemMetadata::None => name.to_string(),
        }
    }

    pub fn to_save(&self) -> ItemInstanceSave {
        ItemInstanceSave {
            kind: self.kind.save_key().to_string(),
            count: self.count,
            weight_g_each: self.weight_g_each,
            charges: self.charges,
            metadata: self.metadata.to_save(),
        }
    }

    pub fn from_save(s: &ItemInstanceSave) -> Option<Self> {
        let kind = ItemKind::from_save_key(&s.kind)?;
        Some(Self {
            kind,
            count: s.count.max(1),
            weight_g_each: s.weight_g_each,
            charges: s.charges,
            metadata: ItemMetadata::from_save(&s.metadata),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub capacity_g: u32,
    pub contents: Vec<ItemInstance>,
}

impl Pack {
    pub fn empty(capacity_g: u32) -> Self {
        Self {
            capacity_g,
            contents: Vec::new(),
        }
    }

    pub fn total_weight_g(&self) -> u32 {
        self.contents
            .iter()
            .map(|i| i.total_weight_g() as u64)
            .sum::<u64>()
            .min(u32::MAX as u64) as u32
    }

    /// Try to add an item. If it doesn't fit (over capacity), return the item
    /// back to the caller untouched. Fungible items merge into an existing
    /// matching stack when possible.
    pub fn try_add(&mut self, item: ItemInstance) -> Result<(), ItemInstance> {
        let new_weight =
            (self.total_weight_g() as u64).saturating_add(item.total_weight_g() as u64);
        if new_weight > self.capacity_g as u64 {
            return Err(item);
        }
        // Only None-metadata fungibles merge; cooked food / cookware
        // carry per-instance state that must not collapse together.
        if item.kind.is_fungible()
            && matches!(item.metadata, ItemMetadata::None)
            && item.charges.is_none()
        {
            if let Some(existing) = self.contents.iter_mut().find(|i| {
                i.kind == item.kind
                    && matches!(i.metadata, ItemMetadata::None)
                    && i.charges.is_none()
                    && i.weight_g_each == item.weight_g_each
            }) {
                existing.count = existing.count.saturating_add(item.count);
                return Ok(());
            }
        }
        self.contents.push(item);
        Ok(())
    }

    /// True if at least one `ItemInstance` of `kind` is present with
    /// `count > 0`. Works for both fungible stacks and unique entries.
    pub fn has_stack(&self, kind: ItemKind) -> bool {
        self.contents
            .iter()
            .any(|i| i.kind == kind && i.count > 0)
    }

    /// Consume one unit of a stack of `kind`: decrement count by 1; if the
    /// count hits 0, remove the entry. Returns true if a unit was taken.
    /// For unique items (count always 1), this removes the entry entirely.
    pub fn take_one_from_stack(&mut self, kind: ItemKind) -> bool {
        let Some(idx) = self
            .contents
            .iter()
            .position(|i| i.kind == kind && i.count > 0)
        else {
            return false;
        };
        self.contents[idx].count -= 1;
        if self.contents[idx].count == 0 {
            self.contents.remove(idx);
        }
        true
    }

    /// True if any waterskin in the pack still has at least one water use.
    pub fn has_waterskin_with_water(&self) -> bool {
        self.contents.iter().any(|i| {
            i.kind == ItemKind::Waterskin
                && matches!(i.metadata, ItemMetadata::Waterskin { water_uses } if water_uses > 0)
        })
    }

    /// Consume one charge of water from the first waterskin that has any.
    /// Decrements `water_uses` and reduces the waterskin's weight by the
    /// per-use water mass (250 g). Returns true if a charge was consumed.
    pub fn drink_one_water_use(&mut self) -> bool {
        for item in self.contents.iter_mut() {
            if item.kind != ItemKind::Waterskin {
                continue;
            }
            if let ItemMetadata::Waterskin {
                ref mut water_uses,
            } = item.metadata
            {
                if *water_uses > 0 {
                    *water_uses -= 1;
                    item.weight_g_each = item.weight_g_each.saturating_sub(250);
                    return true;
                }
            }
        }
        false
    }

    pub fn to_save(&self) -> PackSave {
        PackSave {
            capacity_g: self.capacity_g,
            contents: self.contents.iter().map(|i| i.to_save()).collect(),
        }
    }

    /// Rebuild a pack from a save. Unknown item kinds are dropped silently
    /// (forward-compat across schema versions).
    pub fn from_save(s: &PackSave) -> Self {
        Self {
            capacity_g: s.capacity_g,
            contents: s
                .contents
                .iter()
                .filter_map(ItemInstance::from_save)
                .collect(),
        }
    }
}

/// Slice-1 starting inventory. Total 13.2 kg in a 15 kg pack.
pub fn starting_pack() -> Pack {
    let mut p = Pack::empty(15_000);
    p.contents
        .push(ItemInstance::unique(ItemKind::Axe, 1_000, None, ItemMetadata::None));
    p.contents
        .push(ItemInstance::unique(ItemKind::Knife, 200, None, ItemMetadata::None));
    p.contents.push(ItemInstance::stack(
        ItemKind::Ration,
        3,
        500,
        None,
        ItemMetadata::None,
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::Waterskin,
        1_200,
        Some(4),
        ItemMetadata::Waterskin { water_uses: 4 },
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::Waterskin,
        1_200,
        Some(4),
        ItemMetadata::Waterskin { water_uses: 4 },
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::FlintAndSteel,
        100,
        None,
        ItemMetadata::None,
    ));
    p.contents
        .push(ItemInstance::unique(ItemKind::Tent, 5_000, None, ItemMetadata::None));
    p.contents.push(ItemInstance::unique(
        ItemKind::Bedroll,
        2_000,
        None,
        ItemMetadata::None,
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::CookingPan,
        1_000,
        None,
        ItemMetadata::None,
    ));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_key_round_trip_for_every_kind() {
        for &k in super::ALL_KINDS {
            let key = k.save_key();
            assert_eq!(
                ItemKind::from_save_key(key),
                Some(k),
                "round-trip failed for {:?} via key {}",
                k,
                key
            );
        }
    }

    #[test]
    fn unknown_save_key_returns_none() {
        assert_eq!(ItemKind::from_save_key("definitely_not_a_thing"), None);
    }

    #[test]
    fn starting_pack_weighs_13200g() {
        assert_eq!(starting_pack().total_weight_g(), 13_200);
    }

    #[test]
    fn starting_pack_has_two_waterskins_as_distinct_uniques() {
        let p = starting_pack();
        let ws_count = p
            .contents
            .iter()
            .filter(|i| i.kind == ItemKind::Waterskin)
            .count();
        assert_eq!(ws_count, 2);
    }

    #[test]
    fn fungible_items_stack_in_pack() {
        let mut pack = Pack::empty(1_000);
        let twigs1 = ItemInstance::stack(ItemKind::Twig, 3, 5, None, ItemMetadata::None);
        let twigs2 = ItemInstance::stack(ItemKind::Twig, 2, 5, None, ItemMetadata::None);
        assert!(pack.try_add(twigs1).is_ok());
        assert!(pack.try_add(twigs2).is_ok());
        assert_eq!(pack.contents.len(), 1);
        assert_eq!(pack.contents[0].count, 5);
        assert_eq!(pack.total_weight_g(), 25);
    }

    #[test]
    fn unique_items_dont_stack() {
        let mut pack = Pack::empty(10_000);
        let ws1 = ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        );
        let ws2 = ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        );
        assert!(pack.try_add(ws1).is_ok());
        assert!(pack.try_add(ws2).is_ok());
        assert_eq!(pack.contents.len(), 2);
    }

    #[test]
    fn has_stack_finds_rations_in_starting_pack() {
        let p = starting_pack();
        assert!(p.has_stack(ItemKind::Ration));
        assert!(p.has_stack(ItemKind::Axe));
        assert!(!p.has_stack(ItemKind::Twig));
    }

    #[test]
    fn take_one_from_stack_decrements_and_removes_on_zero() {
        let mut p = Pack::empty(10_000);
        p.contents
            .push(ItemInstance::stack(ItemKind::Ration, 2, 500, None, ItemMetadata::None));
        assert!(p.take_one_from_stack(ItemKind::Ration));
        assert_eq!(p.contents[0].count, 1);
        assert!(p.take_one_from_stack(ItemKind::Ration));
        assert!(p.contents.is_empty());
        assert!(!p.take_one_from_stack(ItemKind::Ration));
    }

    #[test]
    fn take_one_from_stack_works_for_unique_items() {
        // Unique kinds always have count 1; taking one removes the entry.
        let mut p = Pack::empty(10_000);
        p.contents
            .push(ItemInstance::unique(ItemKind::Knife, 200, None, ItemMetadata::None));
        assert!(p.take_one_from_stack(ItemKind::Knife));
        assert!(p.contents.is_empty());
    }

    #[test]
    fn has_waterskin_with_water_reflects_metadata() {
        let mut p = Pack::empty(10_000);
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        ));
        assert!(p.has_waterskin_with_water());

        // Drained waterskin: still in pack but has no water.
        p.contents[0].metadata = ItemMetadata::Waterskin { water_uses: 0 };
        p.contents[0].weight_g_each = 200; // empty weight
        assert!(!p.has_waterskin_with_water());
    }

    #[test]
    fn drink_one_water_use_decrements_uses_and_weight() {
        let mut p = Pack::empty(10_000);
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        ));
        assert!(p.drink_one_water_use());
        match p.contents[0].metadata {
            ItemMetadata::Waterskin { water_uses } => assert_eq!(water_uses, 3),
            other => panic!("expected Waterskin metadata, got {:?}", other),
        }
        assert_eq!(p.contents[0].weight_g_each, 950); // 1200 - 250
    }

    #[test]
    fn drink_one_water_use_returns_false_when_no_water() {
        let mut p = Pack::empty(10_000);
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            200,
            Some(0),
            ItemMetadata::Waterskin { water_uses: 0 },
        ));
        assert!(!p.drink_one_water_use());
    }

    #[test]
    fn drink_one_water_use_picks_first_with_water_then_second() {
        let mut p = Pack::empty(10_000);
        // First skin empty, second full. The second one should drain.
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            200,
            Some(0),
            ItemMetadata::Waterskin { water_uses: 0 },
        ));
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        ));
        assert!(p.drink_one_water_use());
        match p.contents[1].metadata {
            ItemMetadata::Waterskin { water_uses } => assert_eq!(water_uses, 3),
            other => panic!("got {:?}", other),
        }
    }

    #[test]
    fn make_default_instance_fungible_uses_count_and_weight() {
        let inst = ItemKind::Firewood.make_default_instance(4);
        assert_eq!(inst.kind, ItemKind::Firewood);
        assert_eq!(inst.count, 4);
        assert_eq!(inst.weight_g_each, 500);
        assert_eq!(inst.total_weight_g(), 2_000);
        assert!(matches!(inst.metadata, ItemMetadata::None));
    }

    #[test]
    fn make_default_instance_unique_count_always_one() {
        // Asking for 5 axes via make_default_instance still produces a
        // single ItemInstance with count 1; the caller is expected to
        // loop.
        let inst = ItemKind::Axe.make_default_instance(5);
        assert_eq!(inst.count, 1);
        assert_eq!(inst.weight_g_each, 1_000);
    }

    #[test]
    fn make_default_instance_waterskin_is_empty() {
        let inst = ItemKind::Waterskin.make_default_instance(1);
        assert_eq!(inst.weight_g_each, 200, "empty waterskin");
        assert!(matches!(
            inst.metadata,
            ItemMetadata::Waterskin { water_uses: 0 }
        ));
    }

    #[test]
    fn over_capacity_bounces_entire_item() {
        let mut pack = Pack::empty(1_000);
        let heavy = ItemInstance::unique(ItemKind::Tent, 5_000, None, ItemMetadata::None);
        let result = pack.try_add(heavy);
        assert!(result.is_err());
        assert_eq!(pack.contents.len(), 0);
        assert_eq!(pack.total_weight_g(), 0);
    }
}
