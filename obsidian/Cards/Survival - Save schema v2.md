Save schema v1 → v2 bump for the survival redesign. Friendly-reject v1 saves.

## Bump
- `SCHEMA_VERSION` = 2 in `src/save.rs`.
- v1→v2 migration: **friendly-reject**, not data migration. Message: "This save belongs to the Holy Land design. Start a new game to play the survival redesign."
- The Holy Land code is archived under git tag `holy-land-archive`; players can return to it.

## New RunSave (rough)
```rust
pub struct RunSave {
    pub header: SaveHeader,
    pub seed: u64,
    pub clock_seconds: u64,
    pub player_x: i64,
    pub player_y: i64,
    pub player_inventory: Vec<ItemInstanceSave>,
    pub needs: NeedsSave,
    pub skills: BTreeMap<String, SkillSave>,
    pub chunks_dirty: Vec<ChunkSave>,
    pub current_action: Option<ActionQueueSave>,
    pub fires: Vec<FireSave>,
    pub structures: Vec<StructureSave>,
}
```

## New MetaSave
Drop Holy Land fields: `essence` (renamed `demon_currency`), `deity_affinity`, `shrine_unlocked`, `oasis_intro_complete`.
Keep generic `xp`, `unlocks`. Header (counter, device_id, timestamp) unchanged.

## Discipline (unchanged from `[[Save schema migration testing]]`)
- Every non-header field carries `#[serde(default)]` for forward-compat.
- Future v2→v3 etc. adds a `migrate_vN_to_vNplus1` and wires it into `migrate_header`.
- Round-trip test + future-version-rejection test required for v2.
- Frozen v1 save fixture committed to repo so the friendly-reject path stays tested even after v1-writing code is gone.

## Tests
- Round-trip v2 save.
- Reject saves with `schema_version: 3` (future).
- v1 fixture → friendly error message on load.
- Mid-action queue save → reload preserves `current_action` step + remaining seconds.

## Atomic write
Existing `save_atomic` in `src/save.rs` (write-to-tmp → fsync → rename) unchanged. Same crash-safety properties.

Reference: existing `[[Save schema migration testing]]` card; `src/save.rs` header comment.
