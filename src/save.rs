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
// shrine_unlocked, oasis_intro_complete, reeds, ground_items, region). Schema
// version stays at 1 for now; phase 19's "Save schema v2" card bumps it to 2
// once needs/clock/inventory/chunks land.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

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
}

impl MetaSave {
    pub fn empty(header: SaveHeader) -> Self {
        Self {
            header,
            xp: 0,
            unlocks: Vec::new(),
        }
    }
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

fn check_schema(header: &SaveHeader) -> io::Result<()> {
    match header.schema_version {
        SCHEMA_VERSION => Ok(()),
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
        };
        save_atomic(&path, &run).unwrap();
        let loaded = load_run(&path).unwrap();
        assert_eq!(loaded.player_x, 5);
        assert_eq!(loaded.player_y, 7);
        assert_eq!(loaded.pack.capacity_g, 0);
        assert!(loaded.pack.contents.is_empty());
        assert!(loaded.cell_items.is_empty());
        assert_eq!(loaded.clock_seconds, 50_400);
        assert_eq!(loaded.needs.warmth, 100);

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
            },
            rng_state: 0xC0FFEE,
            terrain_mutations: vec![TerrainMutationSave {
                x: 5,
                y: 7,
                kind: "grass".to_string(),
            }],
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
