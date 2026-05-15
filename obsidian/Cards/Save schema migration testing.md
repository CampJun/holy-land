Validate the migration chain end-to-end before the first real schema bump ships.

Session 3 (✅ DONE) already landed `SCHEMA_VERSION = 1` with round-trip and rejects-future-schema tests. The roadmap flags that the first *real* schema bump will probably happen during the depth pass — likely tied to [[Chunk-based world persistence]] (`Position` → i64). This card is the test discipline that has to be in place *before* that bump.

What needs to exist:
- A test that loads a v1 save, migrates to v2, asserts the result matches a hand-rolled v2 expected struct.
- A frozen v1 save fixture committed to the repo so the migration test stays meaningful even after v1-writing code is gone.
- The pattern repeats for every subsequent bump — vN fixture + vN→vN+1 migration test.
- Forward-compat sanity: confirm `#[serde(default)]` is still on every non-header field of every save struct.

Cross-cutting rule from `holyland-ROADMAP.md`: "Every new field is `#[serde(default)]`. Every schema bump adds a migration function. Old saves never silently break."

Source: `holyland-ROADMAP.md` § Session 3, § Session 8+ depth pass (Save migrations), § Cross-cutting rules; `src/save.rs` header comment.
