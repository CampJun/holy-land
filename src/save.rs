// Portable, sync-friendly save format. Format and discipline locked here in
// Session 3 — every save we ever write going forward must be loadable via the
// migration chain below.
//
// Format: CBOR (cross-platform, language-agnostic, deterministic). Every save
// starts with `SaveHeader`; the rest is type-specific. Forward-compat is by
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

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub demon_currency: u64,
    #[serde(default)]
    pub deity_affinity: BTreeMap<String, i32>,
    #[serde(default)]
    pub unlocks: Vec<String>,
}

impl MetaSave {
    pub fn empty(header: SaveHeader) -> Self {
        Self {
            header,
            xp: 0,
            demon_currency: 0,
            deity_affinity: BTreeMap::new(),
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
}

impl RunSave {
    pub fn empty(header: SaveHeader) -> Self {
        Self {
            header,
            player_x: 0,
            player_y: 0,
        }
    }
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

    #[test]
    fn round_trip_meta() {
        let dir = std::env::temp_dir().join(format!("holyland-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.cbor");

        let mut meta = MetaSave::empty(SaveHeader::fresh(None));
        meta.xp = 42;
        meta.unlocks.push("starter_oasis".to_string());
        save_atomic(&path, &meta).unwrap();

        let loaded = load_meta(&path).unwrap();
        assert_eq!(loaded.xp, 42);
        assert_eq!(loaded.unlocks, vec!["starter_oasis"]);
        assert_eq!(loaded.header.schema_version, SCHEMA_VERSION);
        assert_eq!(loaded.header.device_id, meta.header.device_id);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trip_run() {
        let dir = std::env::temp_dir().join(format!("holyland-run-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.cbor");

        let header = SaveHeader::fresh(None);
        let run = RunSave {
            header,
            player_x: 5,
            player_y: 7,
        };
        save_atomic(&path, &run).unwrap();
        let loaded = load_run(&path).unwrap();
        assert_eq!(loaded.player_x, 5);
        assert_eq!(loaded.player_y, 7);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_future_schema() {
        let dir = std::env::temp_dir().join(format!("holyland-fut-{}", std::process::id()));
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
