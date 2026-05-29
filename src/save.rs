// Portable, sync-friendly save format. Format and discipline locked in
// Session 3 (Holy Land); the *machinery* survives the survival redesign even
// as the field contents change.
//
// Format: CBOR (cross-platform, language-agnostic, deterministic). Every save
// starts with `SaveHeader`; the rest is type-specific. Forward-compat by
// `#[serde(default)]` on every non-header field so older binaries skip unknown
// fields silently and newer binaries fill in defaults for absent fields.
//
// Schema evolution path (do this when changing a save struct):
//   1. Bump SCHEMA_VERSION below.
//   2. Add a migrate_vN_to_vNplus1 function and wire it into the migration
//      chain in `migrate_header`.
//   3. If the change is purely additive (new field with sensible default), no
//      migration code is needed — `#[serde(default)]` handles it.
//   4. If the change reshapes an existing field, the migration must do a
//      Value-level read (ciborium::Value) and convert before final deser.
//
// Phase-2 gut: dropped Holy Land run/meta fields (essence/demon_currency,
// shrine_unlocked, oasis_intro_complete, reeds, ground_items, region).
//
// Schema v2 (survival redesign): adds `calendar_day` to RunSave; the
// seasons/flora cluster adds per-cell tree_species / decoration /
// ground_cover (additive, ride this same bump). v1 saves are
// friendly-rejected — no data migration. Players returning to the Holy
// Land design check out the `holy-land-archive` git tag.
//
// Schema v3 (combat foundation): adds `player_health: Option<HealthSave>`
// and `hostiles: Vec<HostileSave>` to RunSave. Both are additive with
// `#[serde(default)]`; v2→v3 needs no migration code, just the header
// version accepts both in `check_schema`. Older saves load with a
// freshly-spawned bandit and the player at full HP.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use uuid::Uuid;

use crate::calendar;

pub const SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveHeader {
    pub schema_version: u32,
    pub build_version: String,
    pub save_counter: u64,
    pub device_id: Uuid,
    pub timestamp: i64,
}

impl SaveHeader {
    pub fn fresh(prev: Option<&SaveHeader>) -> Self {
        let device_id = prev.map(|p| p.device_id).unwrap_or_else(Uuid::new_v4);
        let save_counter = prev.map(|p| p.save_counter + 1).unwrap_or(1);
        Self {
            schema_version: SCHEMA_VERSION,
            build_version: env!("CARGO_PKG_VERSION").to_string(),
            save_counter,
            device_id,
            timestamp: now_unix_secs(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaSave {
    pub header: SaveHeader,
    #[serde(default)]
    pub xp: u64,
    #[serde(default)]
    pub unlocks: Vec<String>,
    /// Player-chosen tileset + per-terrain glyph overrides. Additive
    /// (rides schema v2 via `#[serde(default)]` on the field and on
    /// every inner field) so older meta saves load with an empty
    /// `RenderSettings` and fall back to the default atlas + glyphs.
    #[serde(default)]
    pub render: RenderSettings,
}

impl MetaSave {
    pub fn empty(header: SaveHeader) -> Self {
        Self {
            header,
            xp: 0,
            unlocks: Vec::new(),
            render: RenderSettings::default(),
        }
    }
}

/// In-game-customizable render settings. Empty `atlas_key` means
/// "fall back to the default atlas" (`atlases::DEFAULT_KEY`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RenderSettings {
    #[serde(default)]
    pub atlas_key: String,
    #[serde(default)]
    pub terrain_overrides: Vec<TerrainOverride>,
    /// Per-target sprite overrides set via the in-game sprite picker.
    /// Additive (rides the existing schema via `#[serde(default)]`), so
    /// older meta saves load with no overrides and every target falls
    /// back to its `sprites` registry default. The legacy CP437
    /// `terrain_overrides` above is unused since the sprite migration
    /// but kept for forward-compat.
    #[serde(default)]
    pub sprite_overrides: Vec<SpriteOverride>,
}

/// One terrain → glyph override. `kind_key` is `TerrainKind::save_key`;
/// unknown keys are dropped on load (forward-compat).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainOverride {
    #[serde(default)]
    pub kind_key: String,
    #[serde(default)]
    pub glyph: u8,
}

/// One game-target → sprite override from the in-game picker. `target`
/// is a stable key like `terrain:grass` / `tree:oak` / `item:axe`;
/// `sheet` is `sprites::Sheet::name`. Unknown targets/sheets are dropped
/// on load (forward-compat).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteOverride {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub sheet: String,
    #[serde(default)]
    pub col: u8,
    #[serde(default)]
    pub row: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunSave {
    pub header: SaveHeader,
    #[serde(default)]
    pub player_x: i32,
    #[serde(default)]
    pub player_y: i32,
    // Phase 3 (additive; schema stays v1 because the new fields all carry
    // `#[serde(default)]`):
    #[serde(default)]
    pub pack: PackSave,
    #[serde(default)]
    pub cell_items: Vec<CellItemsSave>,
    // Phase 4 (additive):
    #[serde(default)]
    pub clock_seconds: u64,
    #[serde(default)]
    pub needs: NeedsSave,
    // Phase 6 (additive): explored cells from FOV memory. Sparse
    // (Vec<(x, y)>) since slice 1 is one chunk; can switch to bit-packed
    // per chunk later if explored sets get big.
    #[serde(default)]
    pub explored_cells: Vec<(i32, i32)>,
    // Phase 9 (additive): mid-action queue snapshot so a save during a
    // PitchTent / SetupCamp resumes correctly on load.
    #[serde(default)]
    pub active_action: Option<ActiveActionSave>,
    // Phase 10 (additive): skills + RNG state.
    #[serde(default)]
    pub skills: SkillsSave,
    /// xorshift32 state, persisted so skill-check outcomes can't be
    /// save-scummed by reloading. 0 falls back to "seed from world.seed
    /// at load time" on legacy saves.
    #[serde(default)]
    pub rng_state: u32,
    // Phase 11b (additive): cells whose terrain has been mutated since
    // chunkgen produced them (e.g. ChopTree converts TreeTrunk -> Grass).
    // On load these are re-applied AFTER chunkgen so a chopped tree
    // stays chopped across save/load.
    #[serde(default)]
    pub terrain_mutations: Vec<TerrainMutationSave>,
    // Schema-v2: in-game calendar day, 1-indexed since 1 Jan 1300. The
    // seasons/flora cluster reads this for `season_of(calendar_day)` and
    // the plant lifecycle scheduler. Default = START_DAY (21 Mar 1300)
    // for new games and for any v2 save written before this field
    // existed.
    #[serde(default = "default_calendar_day")]
    pub calendar_day: u32,
    // Combat foundation (action economy slice 1): per-actor CDDA-style
    // speed. Persisted so any future haste/slow status effect survives
    // save/load. Saves written before this field existed default to
    // Speed::BASELINE (100) — identical to the previous implicit value.
    #[serde(default = "default_speed")]
    pub speed: u16,
    // Phase D: per-cell tree_species mutations (currently only "tree
    // chopped → species cleared") and decoration mutations (harvests,
    // sapling spawns, mushroom expiry). Sparse — chunkgen regenerates
    // the deterministic baseline on load; these patches override.
    #[serde(default)]
    pub tree_species_mutations: Vec<TreeSpeciesMutationSave>,
    #[serde(default)]
    pub decoration_mutations: Vec<DecorationMutationSave>,
    /// World seed. Drives per-chunk wilderness contents; the authored
    /// Cornwall layer (biomes/rivers/roads/sites) is identical in every
    /// world regardless of seed. Defaults to `DEFAULT_WORLD_SEED` on
    /// saves written before this field existed so legacy runs reload
    /// onto the same wilderness layout.
    #[serde(default = "default_world_seed")]
    pub seed: u64,
    // Schema v3 (combat foundation): player HP + hostile entities.
    // Both additive with serde defaults — v2 saves load with the
    // player at full HP and the world's fresh-spawn bandit intact.
    #[serde(default)]
    pub player_health: Option<HealthSave>,
    #[serde(default)]
    pub hostiles: Vec<HostileSave>,
    // Phase 2 (combat body parts): split single-pool HP into six body
    // parts per `Armor model.md`. Additive — v3 saves with only
    // player_health load with a defaulted (full-HP) BodyParts and
    // the previous single pool is ignored. Phase 2 stops writing
    // player_health for new saves.
    #[serde(default)]
    pub player_body_parts: Option<BodyPartsSave>,
    // Phase 3 (equip slots): per-slot `ItemKind::save_key` strings.
    // Additive — v3 saves without this field load with the Rabble-
    // tier starting equipment (knife in main hand).
    #[serde(default)]
    pub player_equipment: Option<EquipmentSave>,
    // PR B Attributes card (additive, no schema bump): plumbs STR /
    // AGI / CON / INT / SPIRIT + per-attribute XP for the player.
    // Older saves load with `Attributes::starting_player()` defaults.
    #[serde(default)]
    pub player_attributes: Option<AttributesSave>,
    // PR B stride card (additive, no schema bump): `StrideMode` save_key
    // string ("creep"/"walk"/"jog"). Empty / unknown → `Walk` default.
    #[serde(default)]
    pub player_stride: String,
    // PR B Drag card (additive, no schema bump): true while the player
    // is mid-drag with a Log on their cell. Older saves load with the
    // flag clear (no drag in progress).
    #[serde(default)]
    pub player_dragging: bool,
    // PR A / Stamina card: persisted stamina pool + in-combat
    // cooldown. Additive — saves without this field load with the
    // fresh-spawn full pool (100/100, cooldown 0).
    #[serde(default)]
    pub player_stamina: Option<StaminaSave>,
    /// PR A card 4 — whether the player's crossbow currently has a
    /// bolt chambered. Additive; defaults to false on saves written
    /// before the field existed (matches a fresh boot).
    #[serde(default)]
    pub crossbow_loaded: bool,
}

/// PR B Attributes save block. Five attribute scores + per-attribute
/// XP ledger. Additive — older saves restore via `Attributes::starting_player()`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct AttributesSave {
    #[serde(default)]
    pub str_: u8,
    #[serde(default)]
    pub agi: u8,
    #[serde(default)]
    pub con: u8,
    #[serde(default)]
    pub int_: u8,
    #[serde(default)]
    pub spirit: u8,
    #[serde(default)]
    pub attribute_xp: [u32; 5],
}

/// Stamina pool round-trip per the Stamina card. Carried on the
/// player save and on each `HostileSave`. Additive — `#[serde(default)]`
/// means older saves load with a fresh-spawn pool.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct StaminaSave {
    #[serde(default)]
    pub cur: i16,
    #[serde(default)]
    pub max: i16,
    #[serde(default)]
    pub recent_combat_secs: u16,
}

/// Per-slot equipment round-trip. Each field is a save_key string; an
/// empty string means the slot is unoccupied. Unknown kinds load as
/// empty (forward-compat).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EquipmentSave {
    #[serde(default)]
    pub main_hand: String,
    #[serde(default)]
    pub off_hand: String,
    #[serde(default)]
    pub head: String,
    #[serde(default)]
    pub torso: String,
    #[serde(default)]
    pub l_arm: String,
    #[serde(default)]
    pub r_arm: String,
    #[serde(default)]
    pub l_leg: String,
    #[serde(default)]
    pub r_leg: String,
}

/// Single-pool combat health (phase 1). Phase 2's body-part split
/// retired this in favor of `BodyPartsSave`; the struct stays so v3
/// saves still parse cleanly.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct HealthSave {
    #[serde(default)]
    pub hp: i16,
    #[serde(default)]
    pub max: i16,
}

/// Per-body-part HP round-trip (phase 2). Each part carries its own
/// `(hp, max)`; serde defaults round-trip a fresh-spawn human (40/80/
/// 60/60/60/60). Additive — older v3 saves load via `#[serde(default)]`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct BodyPartSaveCell {
    #[serde(default)]
    pub hp: i16,
    #[serde(default)]
    pub max: i16,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct BodyPartsSave {
    #[serde(default)]
    pub head: BodyPartSaveCell,
    #[serde(default)]
    pub torso: BodyPartSaveCell,
    #[serde(default)]
    pub l_arm: BodyPartSaveCell,
    #[serde(default)]
    pub r_arm: BodyPartSaveCell,
    #[serde(default)]
    pub l_leg: BodyPartSaveCell,
    #[serde(default)]
    pub r_leg: BodyPartSaveCell,
}

/// One hostile entity round-trip. `wielded_kind` is an `ItemKind`
/// save_key string for forward-compat; unknown values drop the
/// wielded weapon on load. `flavor` distinguishes future hostile types
/// from the phase-1 Cornish bandit; unknown flavors load as bandit.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostileSave {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// Phase-1 single-pool HP; phase 2 surfaces the torso pool here as
    /// the closest analog so older binaries reading the file get a
    /// plausible total. Authoritative HP lives in `body_parts`.
    #[serde(default)]
    pub hp: i16,
    #[serde(default)]
    pub max_hp: i16,
    /// `ItemKind::save_key` string for the entity's wielded weapon.
    /// Empty means unarmed.
    #[serde(default)]
    pub wielded_kind: String,
    /// Flavor key, e.g. "cornish_bandit". Defaults to bandit on
    /// unknown values (forward-compat).
    #[serde(default)]
    pub flavor: String,
    /// Phase-2 body-part HP split. Additive — v3 saves without this
    /// field load with a defaulted full-HP body.
    #[serde(default)]
    pub body_parts: Option<BodyPartsSave>,
    /// Phase-3 off-hand `ItemKind::save_key`. Empty means no off-hand.
    /// Additive — older saves load with no off-hand.
    #[serde(default)]
    pub off_hand_kind: String,
    /// Phase-3 worn armor pieces — list of `ItemKind::save_key` strings.
    /// Restore rebuilds the `Worn` component by looking up each kind's
    /// `ItemDef.armor` stats. Unknown / non-wearable keys are skipped
    /// (forward-compat).
    #[serde(default)]
    pub worn_kinds: Vec<String>,
    /// PR A / Stamina card: persisted stamina pool. Additive — older
    /// saves load a fresh-spawn pool.
    #[serde(default)]
    pub stamina: Option<StaminaSave>,
}

/// Backwards-compat seed value matching `world::DEFAULT_SEED`. Used by
/// serde's `default` attribute on `RunSave::seed` so saves written
/// before the seed field existed reload onto the canonical world.
pub const DEFAULT_WORLD_SEED: u64 = 0xC0FFEE_F00D_u64;

fn default_world_seed() -> u64 {
    DEFAULT_WORLD_SEED
}

/// Per-cell tree_species override. `species_key` of empty string means
/// "explicitly None" (a chopped cell). On load, unknown species keys
/// load as None (forward-compat).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TreeSpeciesMutationSave {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// `TreeSpecies::save_key` string, or empty for None (chopped).
    #[serde(default)]
    pub species_key: String,
}

/// Per-cell Decoration override. The Decoration enum is serde-derived
/// in `src/flora.rs`; new variants added at the end are forward-compat
/// (ciborium uses variant names, not order). Existing variants must
/// not be renamed without a migration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecorationMutationSave {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// Decoration value at this cell. Default = None.
    #[serde(default)]
    pub decoration: crate::flora::Decoration,
}

fn default_calendar_day() -> u32 {
    calendar::START_DAY
}

fn default_speed() -> u16 {
    crate::world::Speed::BASELINE
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TerrainMutationSave {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// `TerrainKind::save_key()` string. Unknown keys are dropped on
    /// load (forward-compat).
    #[serde(default)]
    pub kind: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct SkillsSave {
    #[serde(default)]
    pub fire_making: SkillSave,
    // Phase-C: Foraging skill. Defaults to all-zero on saves written
    // before this field existed; the load path falls back to
    // `Skills::starting()` for any save where fire_making is also zero
    // (pre-phase-10), so the new field never sets a brand-new player's
    // skill to 0 by accident.
    #[serde(default)]
    pub foraging: SkillSave,
    // Phase-9 combat skills. Additive — older saves load with all
    // three at 0 (matches Rabble-tier starting values).
    #[serde(default)]
    pub melee: SkillSave,
    #[serde(default)]
    pub ranged: SkillSave,
    #[serde(default)]
    pub dodge: SkillSave,
    // PR A card 2 — defensive skills + per-weapon proficiencies. All
    // additive with serde defaults; older saves load with the pools at
    // zero (matches a fresh Rabble player).
    #[serde(default)]
    pub block: SkillSave,
    #[serde(default)]
    pub light_armor: SkillSave,
    #[serde(default)]
    pub medium_armor: SkillSave,
    #[serde(default)]
    pub heavy_armor: SkillSave,
    #[serde(default)]
    pub proficiencies: ProficienciesSave,
}

/// PR A card 2 — round-trip shape for the 11 per-weapon proficiency
/// pools. Each field is a `SkillSave` (value + daily_xp); all fields
/// are `#[serde(default)]` so saves written before this struct existed
/// load with every pool at zero.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ProficienciesSave {
    #[serde(default)]
    pub knife: SkillSave,
    #[serde(default)]
    pub sword: SkillSave,
    #[serde(default)]
    pub falchion: SkillSave,
    #[serde(default)]
    pub axe: SkillSave,
    #[serde(default)]
    pub mace_cudgel: SkillSave,
    #[serde(default)]
    pub quarterstaff: SkillSave,
    #[serde(default)]
    pub spear_lance: SkillSave,
    #[serde(default)]
    pub gisarme_bill: SkillSave,
    #[serde(default)]
    pub unarmed: SkillSave,
    #[serde(default)]
    pub bow: SkillSave,
    #[serde(default)]
    pub crossbow: SkillSave,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct SkillSave {
    #[serde(default)]
    pub value: u8,
    #[serde(default)]
    pub daily_xp: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActiveActionSave {
    #[serde(default)]
    pub steps: Vec<ActionStepSave>,
    /// "progress_bar" | "time_skip". Defaults to progress_bar on
    /// unknown values for forward-compat.
    #[serde(default)]
    pub view_mode: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActionStepSave {
    /// `ActionId::save_key()` string. Unknown ids are dropped on load.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub elapsed_secs: u32,
    #[serde(default)]
    pub target_secs: u32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct NeedsSave {
    #[serde(default)]
    pub thirst: u8,
    #[serde(default)]
    pub hunger: u8,
    #[serde(default)]
    pub sleep: u8,
    #[serde(default)]
    pub warmth: u8,
    #[serde(default)]
    pub thirst_acc_secs: u32,
    #[serde(default)]
    pub hunger_acc_secs: u32,
    #[serde(default)]
    pub sleep_acc_secs: u32,
    #[serde(default)]
    pub warmth_acc_secs: u32,
}

impl RunSave {
    pub fn empty(header: SaveHeader) -> Self {
        Self {
            header,
            player_x: 0,
            player_y: 0,
            pack: PackSave::default(),
            cell_items: Vec::new(),
            clock_seconds: 0,
            needs: NeedsSave::default(),
            explored_cells: Vec::new(),
            active_action: None,
            skills: SkillsSave::default(),
            rng_state: 0,
            terrain_mutations: Vec::new(),
            calendar_day: calendar::START_DAY,
            speed: crate::world::Speed::BASELINE,
            tree_species_mutations: Vec::new(),
            decoration_mutations: Vec::new(),
            seed: DEFAULT_WORLD_SEED,
            player_health: None,
            hostiles: Vec::new(),
            player_body_parts: None,
            player_equipment: None,
            player_attributes: None,
            player_stride: String::new(),
            player_dragging: false,
            player_stamina: None,
            crossbow_loaded: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PackSave {
    #[serde(default)]
    pub capacity_g: u32,
    #[serde(default)]
    pub contents: Vec<ItemInstanceSave>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ItemInstanceSave {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub count: u16,
    #[serde(default)]
    pub weight_g_each: u32,
    #[serde(default)]
    pub charges: Option<u16>,
    #[serde(default)]
    pub metadata: ItemMetadataSave,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum ItemMetadataSave {
    #[default]
    None,
    Waterskin {
        water_uses: u8,
    },
    /// Item is placed in the world (pitched tent, unrolled bedroll).
    Pitched,
    /// A lit fire with remaining fuel in game-seconds.
    Lit {
        fuel_seconds: u32,
    },
    /// CookingPan placed on a fire. Contents are encoded via two flat
    /// string fields (`contents_kind` + `input`) plus the elapsed-secs
    /// and seasonings u8 so adding a new PanContents variant later is
    /// purely additive (unknown contents_kind -> Empty on load).
    PannedOnFire {
        #[serde(default)]
        contents_kind: String,
        #[serde(default)]
        input: String,
        #[serde(default)]
        elapsed_secs: u32,
        #[serde(default)]
        seasonings: u8,
        #[serde(default)]
        fuel_seconds: u32,
    },
    /// Cooked food. `base`/`state` are stringly-typed so new variants
    /// plug in without bumping schema; unknown strings fall back to
    /// sensible defaults at load time.
    Cooked {
        #[serde(default)]
        base: String,
        #[serde(default)]
        state: String,
        #[serde(default)]
        seasonings: u8,
    },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CellItemsSave {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub items: Vec<ItemInstanceSave>,
}

pub fn save_atomic<T: Serialize>(path: &Path, data: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("cbor.tmp");
    {
        let f = fs::File::create(&tmp)?;
        let mut buf = io::BufWriter::new(f);
        ciborium::into_writer(data, &mut buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        buf.flush()?;
        buf.into_inner()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load_meta(path: &Path) -> io::Result<MetaSave> {
    let bytes = fs::read(path)?;
    let save: MetaSave = ciborium::from_reader(&*bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    check_schema(&save.header)?;
    Ok(save)
}

pub fn load_run(path: &Path) -> io::Result<RunSave> {
    let bytes = fs::read(path)?;
    let save: RunSave = ciborium::from_reader(&*bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    check_schema(&save.header)?;
    Ok(save)
}

/// v1 → v2 is a friendly-reject, not a data migration. The Holy Land
/// design (v1) and the Survival redesign (v2) diverge enough that
/// migrating a v1 oasis save into a v2 wilderness chunk would produce
/// junk. Players who want to keep playing Holy Land can check out the
/// `holy-land-archive` git tag; everyone else starts a new game.
const V1_FRIENDLY_REJECT_MSG: &str =
    "This save belongs to the Holy Land design. Start a new game to play the survival redesign.";

fn check_schema(header: &SaveHeader) -> io::Result<()> {
    match header.schema_version {
        // v2 → v3 is purely additive (player_health + hostiles default
        // via serde). Accept both; the next save re-headers the file to
        // SCHEMA_VERSION so the v2 footprint vanishes over time.
        2 | 3 => Ok(()),
        1 => Err(io::Error::new(io::ErrorKind::Other, V1_FRIENDLY_REJECT_MSG)),
        v if v < SCHEMA_VERSION => Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "save schema v{} predates current v{}; no migration registered yet",
                v, SCHEMA_VERSION
            ),
        )),
        v => Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "save schema v{} was written by a newer build; refusing to read",
                v
            ),
        )),
    }
}

fn now_unix_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::{CookableKind, CookedState, PanContents, Seasonings, ModifierTag};
    use crate::items::{ItemInstance, ItemKind, ItemMetadata};

    #[test]
    fn panned_on_fire_metadata_round_trips_through_cbor() {
        // Mid-cook fish with a herb seasoning, 1100 fuel-seconds left.
        let pan = ItemInstance::unique(
            ItemKind::CookingPan,
            1_000,
            None,
            ItemMetadata::PannedOnFire {
                contents: PanContents::Cooking {
                    input: CookableKind::Fish,
                    elapsed_secs: 47,
                    seasonings: {
                        let mut s = Seasonings::empty();
                        s.set(ModifierTag::Herb);
                        s
                    },
                },
                fuel_seconds: 1_100,
            },
        );
        let saved = pan.to_save();
        let mut bytes = Vec::new();
        ciborium::into_writer(&saved, &mut bytes).unwrap();
        let decoded: ItemInstanceSave = ciborium::from_reader(&*bytes).unwrap();
        let restored = ItemInstance::from_save(&decoded).unwrap();
        match restored.metadata {
            ItemMetadata::PannedOnFire { contents, fuel_seconds } => {
                assert_eq!(fuel_seconds, 1_100);
                match contents {
                    PanContents::Cooking { input, elapsed_secs, seasonings } => {
                        assert_eq!(input, CookableKind::Fish);
                        assert_eq!(elapsed_secs, 47);
                        assert!(seasonings.has(ModifierTag::Herb));
                    }
                    other => panic!("expected Cooking, got {:?}", other),
                }
            }
            other => panic!("expected PannedOnFire, got {:?}", other),
        }
    }

    #[test]
    fn cooked_metadata_round_trips_with_seasonings_and_state() {
        let mut s = Seasonings::empty();
        s.set(ModifierTag::Herb);
        let cooked = ItemInstance::unique(
            ItemKind::Cooked,
            400,
            None,
            ItemMetadata::Cooked {
                base: CookableKind::Fish,
                state: CookedState::Burnt,
                seasonings: s,
            },
        );
        let saved = cooked.to_save();
        let mut bytes = Vec::new();
        ciborium::into_writer(&saved, &mut bytes).unwrap();
        let decoded: ItemInstanceSave = ciborium::from_reader(&*bytes).unwrap();
        let restored = ItemInstance::from_save(&decoded).unwrap();
        match restored.metadata {
            ItemMetadata::Cooked { base, state, seasonings } => {
                assert_eq!(base, CookableKind::Fish);
                assert_eq!(state, CookedState::Burnt);
                assert!(seasonings.has(ModifierTag::Herb));
            }
            other => panic!("expected Cooked, got {:?}", other),
        }
    }

    #[test]
    fn round_trip_meta() {
        let dir = std::env::temp_dir().join(format!("survival-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.cbor");

        let mut meta = MetaSave::empty(SaveHeader::fresh(None));
        meta.xp = 42;
        meta.unlocks.push("first_fire".to_string());
        save_atomic(&path, &meta).unwrap();

        let loaded = load_meta(&path).unwrap();
        assert_eq!(loaded.xp, 42);
        assert_eq!(loaded.unlocks, vec!["first_fire"]);
        assert_eq!(loaded.header.schema_version, SCHEMA_VERSION);
        assert_eq!(loaded.header.device_id, meta.header.device_id);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trip_meta_render_settings() {
        let dir = std::env::temp_dir().join(format!("survival-render-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.cbor");

        let mut meta = MetaSave::empty(SaveHeader::fresh(None));
        meta.render.atlas_key = "aesomatica".to_string();
        meta.render.terrain_overrides.push(TerrainOverride {
            kind_key: "cobble_road".to_string(),
            glyph: 0xCD,
        });
        meta.render.sprite_overrides.push(SpriteOverride {
            target: "terrain:grass".to_string(),
            sheet: "overworld".to_string(),
            col: 1,
            row: 0,
        });
        save_atomic(&path, &meta).unwrap();

        let loaded = load_meta(&path).unwrap();
        assert_eq!(loaded.render.atlas_key, "aesomatica");
        assert_eq!(loaded.render.terrain_overrides.len(), 1);
        assert_eq!(loaded.render.terrain_overrides[0].kind_key, "cobble_road");
        assert_eq!(loaded.render.terrain_overrides[0].glyph, 0xCD);
        assert_eq!(loaded.render.sprite_overrides.len(), 1);
        assert_eq!(loaded.render.sprite_overrides[0].target, "terrain:grass");
        assert_eq!(loaded.render.sprite_overrides[0].sheet, "overworld");
        assert_eq!(loaded.render.sprite_overrides[0].col, 1);
        assert_eq!(loaded.render.sprite_overrides[0].row, 0);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_meta_loads_with_default_render_settings() {
        // Simulate a pre-render-settings meta by serializing only the
        // fields that existed before. The #[serde(default)] on
        // `render` must fill in an empty RenderSettings.
        #[derive(Serialize)]
        struct LegacyMeta {
            header: SaveHeader,
            xp: u64,
            unlocks: Vec<String>,
        }
        let dir =
            std::env::temp_dir().join(format!("survival-meta-legacy-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy_meta.cbor");

        let legacy = LegacyMeta {
            header: SaveHeader::fresh(None),
            xp: 7,
            unlocks: vec!["a".to_string()],
        };
        save_atomic(&path, &legacy).unwrap();

        let loaded = load_meta(&path).unwrap();
        assert_eq!(loaded.xp, 7);
        assert!(loaded.render.atlas_key.is_empty());
        assert!(loaded.render.terrain_overrides.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trip_run_with_combat_state() {
        let dir = std::env::temp_dir().join(format!("survival-combat-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.cbor");

        let header = SaveHeader::fresh(None);
        let mut run = RunSave::empty(header);
        run.player_health = Some(HealthSave { hp: 42, max: 80 });
        run.hostiles = vec![HostileSave {
            x: 10,
            y: 12,
            hp: 65,
            max_hp: 80,
            wielded_kind: "spear".to_string(),
            flavor: "cornish_bandit".to_string(),
            body_parts: None,
            off_hand_kind: String::new(),
            worn_kinds: Vec::new(),
            stamina: None,
        }];
        save_atomic(&path, &run).unwrap();

        let loaded = load_run(&path).unwrap();
        assert_eq!(loaded.player_health.map(|h| (h.hp, h.max)), Some((42, 80)));
        assert_eq!(loaded.hostiles.len(), 1);
        assert_eq!(loaded.hostiles[0].x, 10);
        assert_eq!(loaded.hostiles[0].hp, 65);
        assert_eq!(loaded.hostiles[0].wielded_kind, "spear");
        assert_eq!(loaded.hostiles[0].flavor, "cornish_bandit");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trip_run() {
        let dir = std::env::temp_dir().join(format!("survival-run-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.cbor");

        let header = SaveHeader::fresh(None);
        let run = RunSave {
            header,
            player_x: 5,
            player_y: 7,
            pack: PackSave::default(),
            cell_items: Vec::new(),
            clock_seconds: 50_400,
            needs: NeedsSave {
                thirst: 75,
                hunger: 75,
                sleep: 75,
                warmth: 100,
                thirst_acc_secs: 0,
                hunger_acc_secs: 0,
                sleep_acc_secs: 0,
                warmth_acc_secs: 0,
            },
            explored_cells: Vec::new(),
            active_action: None,
            skills: SkillsSave::default(),
            rng_state: 0xDEADBEEF,
            terrain_mutations: Vec::new(),
            calendar_day: 100,
            speed: crate::world::Speed::BASELINE,
            tree_species_mutations: Vec::new(),
            decoration_mutations: Vec::new(),
            seed: 0xABCD_1234_5678_9ABC,
            player_health: None,
            hostiles: Vec::new(),
            player_body_parts: None,
            player_equipment: None,
            player_attributes: None,
            player_stride: String::new(),
            player_dragging: false,
            player_stamina: None,
            crossbow_loaded: false,
        };
        save_atomic(&path, &run).unwrap();
        let loaded = load_run(&path).unwrap();
        assert_eq!(loaded.player_x, 5);
        assert_eq!(loaded.player_y, 7);
        assert_eq!(loaded.seed, 0xABCD_1234_5678_9ABC);
        assert_eq!(loaded.pack.capacity_g, 0);
        assert!(loaded.pack.contents.is_empty());
        assert!(loaded.cell_items.is_empty());
        assert_eq!(loaded.clock_seconds, 50_400);
        assert_eq!(loaded.needs.warmth, 100);
        assert_eq!(loaded.calendar_day, 100);
        assert_eq!(loaded.speed, crate::world::Speed::BASELINE);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trip_run_with_pack_and_cell_items() {
        let dir = std::env::temp_dir().join(format!("survival-run-full-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.cbor");

        let header = SaveHeader::fresh(None);
        let pack = PackSave {
            capacity_g: 15_000,
            contents: vec![
                ItemInstanceSave {
                    kind: "axe".to_string(),
                    count: 1,
                    weight_g_each: 1000,
                    charges: None,
                    metadata: ItemMetadataSave::None,
                },
                ItemInstanceSave {
                    kind: "waterskin".to_string(),
                    count: 1,
                    weight_g_each: 1200,
                    charges: Some(4),
                    metadata: ItemMetadataSave::Waterskin { water_uses: 4 },
                },
                ItemInstanceSave {
                    kind: "twig".to_string(),
                    count: 7,
                    weight_g_each: 5,
                    charges: None,
                    metadata: ItemMetadataSave::None,
                },
            ],
        };
        let cell_items = vec![CellItemsSave {
            x: 21,
            y: 15,
            items: vec![ItemInstanceSave {
                kind: "stone".to_string(),
                count: 1,
                weight_g_each: 200,
                charges: None,
                metadata: ItemMetadataSave::None,
            }],
        }];
        let run = RunSave {
            header,
            player_x: 21,
            player_y: 15,
            pack,
            cell_items,
            clock_seconds: 0,
            needs: NeedsSave::default(),
            explored_cells: vec![(21, 15), (22, 16)],
            active_action: Some(ActiveActionSave {
                steps: vec![ActionStepSave {
                    id: "pitch_tent".to_string(),
                    elapsed_secs: 42,
                    target_secs: 300,
                }],
                view_mode: "time_skip".to_string(),
            }),
            skills: SkillsSave {
                fire_making: SkillSave {
                    value: 23,
                    daily_xp: 6,
                },
                foraging: SkillSave::default(),
                melee: SkillSave::default(),
                ranged: SkillSave::default(),
                dodge: SkillSave::default(),
                block: SkillSave::default(),
                light_armor: SkillSave::default(),
                medium_armor: SkillSave::default(),
                heavy_armor: SkillSave::default(),
                proficiencies: ProficienciesSave::default(),
            },
            rng_state: 0xC0FFEE,
            terrain_mutations: vec![TerrainMutationSave {
                x: 5,
                y: 7,
                kind: "grass".to_string(),
            }],
            calendar_day: calendar::START_DAY,
            speed: 150,
            tree_species_mutations: Vec::new(),
            decoration_mutations: Vec::new(),
            seed: DEFAULT_WORLD_SEED,
            player_health: None,
            hostiles: Vec::new(),
            player_body_parts: None,
            player_equipment: None,
            player_attributes: None,
            player_stride: String::new(),
            player_dragging: false,
            player_stamina: None,
            crossbow_loaded: false,
        };
        save_atomic(&path, &run).unwrap();
        let loaded = load_run(&path).unwrap();
        assert_eq!(loaded.pack.capacity_g, 15_000);
        assert_eq!(loaded.explored_cells, vec![(21, 15), (22, 16)]);
        let active = loaded.active_action.expect("active_action round-trip");
        assert_eq!(active.steps.len(), 1);
        assert_eq!(active.steps[0].id, "pitch_tent");
        assert_eq!(active.steps[0].elapsed_secs, 42);
        assert_eq!(active.view_mode, "time_skip");
        assert_eq!(loaded.skills.fire_making.value, 23);
        assert_eq!(loaded.skills.fire_making.daily_xp, 6);
        assert_eq!(loaded.rng_state, 0xC0FFEE);
        assert_eq!(loaded.terrain_mutations.len(), 1);
        assert_eq!(loaded.terrain_mutations[0].kind, "grass");
        assert_eq!(loaded.pack.contents.len(), 3);
        assert_eq!(loaded.pack.contents[0].kind, "axe");
        assert_eq!(loaded.pack.contents[1].kind, "waterskin");
        assert!(matches!(
            loaded.pack.contents[1].metadata,
            ItemMetadataSave::Waterskin { water_uses: 4 }
        ));
        assert_eq!(loaded.pack.contents[2].count, 7);
        assert_eq!(loaded.cell_items.len(), 1);
        assert_eq!(loaded.cell_items[0].x, 21);
        assert_eq!(loaded.cell_items[0].items[0].kind, "stone");
        assert_eq!(loaded.speed, 150);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trip_player_attributes() {
        let dir = std::env::temp_dir().join(format!("survival-attrs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.cbor");
        let mut run = RunSave::empty(SaveHeader::fresh(None));
        run.player_attributes = Some(AttributesSave {
            str_: 14,
            agi: 12,
            con: 11,
            int_: 9,
            spirit: 13,
            attribute_xp: [10, 20, 30, 40, 50],
        });
        save_atomic(&path, &run).unwrap();
        let loaded = load_run(&path).unwrap();
        let attrs = loaded.player_attributes.expect("attributes round-trip");
        assert_eq!(attrs.str_, 14);
        assert_eq!(attrs.agi, 12);
        assert_eq!(attrs.con, 11);
        assert_eq!(attrs.int_, 9);
        assert_eq!(attrs.spirit, 13);
        assert_eq!(attrs.attribute_xp, [10, 20, 30, 40, 50]);
        // Pre-Attributes saves load with `None` and the world falls
        // back to `Attributes::starting_player()` at restore time.
        let mut legacy = RunSave::empty(SaveHeader::fresh(None));
        legacy.player_attributes = None;
        let legacy_path = dir.join("legacy.cbor");
        save_atomic(&legacy_path, &legacy).unwrap();
        let legacy_loaded = load_run(&legacy_path).unwrap();
        assert!(legacy_loaded.player_attributes.is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn loads_old_run_save_without_pack_fields() {
        // Simulate a pre-phase-3 save by serializing only the header +
        // position fields, then deserializing into the new RunSave shape.
        // The #[serde(default)] attrs on pack/cell_items must fill in
        // safely so old saves stay loadable.
        #[derive(Serialize)]
        struct LegacyRun {
            header: SaveHeader,
            player_x: i32,
            player_y: i32,
        }
        let dir = std::env::temp_dir().join(format!("survival-legacy-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("legacy.cbor");

        let legacy = LegacyRun {
            header: SaveHeader::fresh(None),
            player_x: 3,
            player_y: 4,
        };
        save_atomic(&path, &legacy).unwrap();

        let loaded = load_run(&path).unwrap();
        assert_eq!(loaded.player_x, 3);
        assert_eq!(loaded.player_y, 4);
        assert_eq!(loaded.pack.capacity_g, 0);
        assert!(loaded.pack.contents.is_empty());
        assert!(loaded.cell_items.is_empty());
        // Phase-4 fields must default safely on legacy saves too.
        assert_eq!(loaded.clock_seconds, 0);
        assert_eq!(loaded.needs.thirst, 0);
        assert_eq!(loaded.needs.warmth, 0);
        // Schema-v2 calendar_day defaults to START_DAY (21 Mar 1300).
        assert_eq!(loaded.calendar_day, calendar::START_DAY);
        // Seed defaults to DEFAULT_WORLD_SEED on saves written before
        // the field existed, so legacy runs reload onto canonical Cornwall.
        assert_eq!(loaded.seed, DEFAULT_WORLD_SEED);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn v1_save_friendly_rejected() {
        // A v1 save (Holy Land era) must produce the friendly-reject
        // message rather than crash or silently load. Build a minimal
        // CBOR blob carrying just a v1 header — the survival redesign
        // never migrates v1 data.
        let dir = std::env::temp_dir().join(format!("survival-v1reject-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v1.cbor");

        let header = SaveHeader {
            schema_version: 1,
            build_version: "0.0.0".to_string(),
            save_counter: 1,
            device_id: Uuid::new_v4(),
            timestamp: 0,
        };
        // A v1 RunSave likely had different fields, but the load path
        // only inspects `header.schema_version`. A header-only blob is
        // enough to trip the check.
        let meta = MetaSave {
            header,
            xp: 0,
            unlocks: Vec::new(),
            render: RenderSettings::default(),
        };
        save_atomic(&path, &meta).unwrap();

        let err = load_meta(&path).unwrap_err();
        assert!(
            err.to_string().contains("Holy Land design"),
            "v1 reject must mention Holy Land; got {:?}",
            err
        );
        assert!(err.to_string().contains("Start a new game"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_future_schema() {
        let dir = std::env::temp_dir().join(format!("survival-fut-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("future.cbor");

        let mut header = SaveHeader::fresh(None);
        header.schema_version = SCHEMA_VERSION + 999;
        let meta = MetaSave::empty(header);
        save_atomic(&path, &meta).unwrap();

        let err = load_meta(&path).unwrap_err();
        assert!(err.to_string().contains("newer build"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_counter_monotonic() {
        let h1 = SaveHeader::fresh(None);
        let h2 = SaveHeader::fresh(Some(&h1));
        let h3 = SaveHeader::fresh(Some(&h2));
        assert_eq!(h1.save_counter, 1);
        assert_eq!(h2.save_counter, 2);
        assert_eq!(h3.save_counter, 3);
        assert_eq!(h1.device_id, h2.device_id);
        assert_eq!(h2.device_id, h3.device_id);
    }
}
