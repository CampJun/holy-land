CDDA-scale world: chunked storage, seed-based generation, save-schema v2 migration.

The render pipeline and `World::tile_at(i64, i64)` API are already open-world-shaped. The remaining work is the storage + generation layer behind that API. Plan from `AGENTS.md` § CDDA-scale plan:

- **Coords:** widen `Position` (ECS component) and `RunSave.player_{x,y}` to `i64`. Bump `save::SCHEMA_VERSION` to 2 and add a v1→v2 migration. CBOR int widening is value-compatible; migration is bookkeeping.
- **Storage:** chunks (32×32 or 64×64). `World { chunks: HashMap<ChunkCoord, Box<Chunk>>, seed: u64, save_dir: PathBuf }`. Generate on demand from `(seed, cx, cy)`.
- **LRU cache:** keep ~9–25 chunks resident around the player. Evict-with-persist if mutated, drop otherwise (procgen recreates).
- **Entities:** `hecs` already a dep; entities carry world coords; tick only loaded chunks.
- **Renderer:** unchanged. Viewport-only scanning means nothing in `render.rs` needs to know about chunks.

Two-line camera swap from anchored to player-centered is already documented in `AGENTS.md`; do that swap as part of this card.

Source: `AGENTS.md` § Camera + world API, § CDDA-scale plan, § What is NOT done yet.
