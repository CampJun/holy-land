# STYLE.md — Survival code conventions

Operational rules (deploy, mmiyoo quirks, save format) live in
[`AGENTS.md`](AGENTS.md). This file is the **code style + architecture**
reference for anyone (human or agent) editing the Rust source.

Last updated phase 8 (commit-after-this-doc lands at `architecture pass`).

---

## 1. Module ownership map

```
src/
├── main.rs           SDL frame loop, save/load orchestration, HUD draw,
│                     command-menu UI, debug-console plumbing. The only
│                     module that knows about SDL2.
├── render.rs         load_atlas + draw_glyph (CPU surface blit). The
│                     only drawing primitive.
├── input.rs          Action enum, cfg-gated desktop vs Miyoo keymap,
│                     gilrs gamepad polling, key-repeat timing.
├── world.rs          World, Chunk, CellState, Position, TerrainKind,
│                     FOV recompute, game-time clock, action-time
│                     spending. ECS plumbing (hecs). No SDL types.
├── items.rs          ItemKind, ItemDef (per-kind metadata in ONE
│                     place), ItemInstance, ItemMetadata, Pack with
│                     consume/stack helpers, starting_pack(),
│                     save round-trip.
├── needs.rs          Needs struct, NeedKind, NeedsEnv, decay rates,
│                     restore/set/reset_accumulator. The only module
│                     that owns need values.
├── action.rs         ActionId, ContextAction catalog, evaluate(),
│                     execute(), Availability + helpers, the
│                     consume_and_restore chassis for verbs.
├── fov.rs            Recursive shadowcasting. Standalone, world-
│                     agnostic — takes a `blocks` closure.
├── save.rs           CBOR + atomic write + schema-versioning
│                     machinery. Save types mirror runtime types
│                     additively (#[serde(default)] on every field).
├── logging.rs        log_info! / log_debug! / log_verbose! macros.
│                     File + stderr routing with TTY-aware
│                     suppression.
├── debug_console.rs  Stdin reader thread + parse_command +
│                     apply_debug_command. Desktop-only.
├── platform/         save_dir() per-OS resolver.
```

### Where each thing lives — quick lookup

| If you're touching… | Edit… | DON'T edit… |
|---|---|---|
| What a need can do | `needs.rs` | `world.rs` |
| What a verb costs | `world.rs` (`COST_*`) | `action.rs` |
| What a verb DOES | `action.rs` (`evaluate` + `execute`) | `world.rs` |
| Per-cell terrain or items | `world.rs` (`CellState` + `tile_at`/`cell_at*`) | `main.rs` |
| Item appearance/name | `items.rs` (`ItemKind::def()`) | `main.rs` |
| HUD layout | `main.rs` (`build_ui_cells`) | anywhere else |
| Save schema | `save.rs` (additive `#[serde(default)]`) + round-trip test | bump `SCHEMA_VERSION` only on incompatible changes |
| Debug command | `debug_console.rs` (3 places: enum, parse, apply) | n/a |
| Logging | use the macros; don't `eprintln!` | `logging.rs` (unless you need a new category) |

---

## 2. DRY patterns we've settled on

These crystallized through phases 3–8. Use them; if you find yourself
writing a fourth instance of a similar shape, extend the pattern instead
of forking.

### 2.1 `Availability::from_has(has, cost, missing_reason)`
Every verb that asks "is X in my pack/world?" routes through this
constructor in `action.rs`. Example:

```rust
ActionId::EatRation => Availability::from_has(
    pack.has_stack(ItemKind::Ration),
    COST_EAT_RATION,
    "no rations in pack",
),
```

### 2.2 `consume_and_restore` chassis (action.rs)
"Take one thing from pack, restore one need, advance time" verbs share
a single function. The `ConsumeFrom` enum names the two consume shapes
we have today (`Stack(kind)`, `WaterskinCharge`); add a variant when
phase 12 introduces cooked-food consumption that drains state
differently.

### 2.3 `Pack::has_stack(kind)` / `take_one_from_stack(kind)`
"Have one of kind X" / "consume one of kind X". Works for both fungible
stacks (Ration, Twig) and unique items (Knife, Tent) — uniques have
count=1, so "take one" removes the entry.

### 2.4 `Needs::restore(kind, amount)` / `set(kind, value)` / `reset_accumulator(kind)`
All three go through a private `mut_pair(kind)` helper that returns
`(&mut u8, &mut u32)`. Adding a new need is one match arm there; every
mutator method picks up support automatically.

### 2.5 `ItemKind::def()` returning `ItemDef`
One match expression contains save_key + name + is_fungible + glyph +
color for every kind. The five public accessors (`save_key`, `name`,
`is_fungible`, `glyph_color`, `from_save_key`) all read through `def()`.
Adding an item: extend the enum + add one match arm. The compiler's
exhaustive-match check enforces this.

### 2.6 Save additivity
Every non-header field on `MetaSave` / `RunSave` carries
`#[serde(default)]`. Adding a new field is forward-compatible (old saves
load with the default value); subtractive or shape-changing edits
require a `SCHEMA_VERSION` bump + migration. See save.rs header.

### 2.7 Logging categories
`log_info!` = always shown in terminal + always logged.
`log_debug!` = file always; stderr only when stderr is not a TTY (Miyoo)
or `LOG_VERBOSE=1`. Use for per-action chatter (pickup, save).
`log_verbose!` = file always; stderr only with `LOG_VERBOSE=1`. Use for
per-frame noise (fps, timing).

Choose by audience: would a player running interactively want to see it?
→ info. Is it useful in postmortem-log triage? → debug. Is it
per-frame? → verbose.

### 2.8 Locality of design constants

**Source of truth lives as close to its consumer as possible.** Per-X
tuning values stay in X's owning module, not on `world.rs` (which only
owns engine-level primitives).

| What | Lives in | Accessor |
|---|---|---|
| Per-verb base cost (CDDA-style moves) | `action.rs` | `ActionId::move_cost()` — single match arm per verb; wall-clock seconds via `World::moves_to_seconds` (folds in actor speed) |
| Verb-specific design constants (e.g. `FIRE_BONUS_FLINT_AND_STEEL`, `FELLED_FIREWOOD_MIN`) | `action.rs` (grouped by verb) | private `const` |
| Per-item weight / glyph / etc. | `items.rs` | `ItemKind::def()` |
| Per-terrain glyph / walkable / blocks_sight | `world.rs` (TerrainDef table, with terrain) | `TerrainKind::def()` |
| Per-need decay rates, max value, accumulator | `needs.rs` | private `const` |
| Per-skill starting value, daily cap | `skill.rs` | `Skills::starting()`, `DAILY_XP_CAP` |
| Engine primitives: tile-step move-cost (`MOVE_COST_TILE`), `MOVES_PER_SECOND`, day length, FOV radii, brightness curve, multi-turn pacing | `world.rs` | `pub const` |

**Rule:** if you're balancing a verb (cost, materials, output), edit
should be one match arm or one const in `action.rs`. You should never
need to update both `world.rs` AND `action.rs` for the same balance
change.

The phase-9 `try_pickup_all_at_player` primitive in `world.rs` follows
this pattern: it doesn't spend action time itself; the caller (action.rs
execute path, or main.rs's A-button handler) calls
`world.spend_moves(ActionId::Pickup.move_cost())` after the return.
The world primitive is verb-agnostic.

### 2.9 Menu / panel rendering

UI windows (pause menu, command menu, multi-turn banner) compose from
three primitives in `main.rs`:

- `PanelLayout` — positioning struct with `anchored(x, y, w, h)` /
  `centered(w, h)` / `centered_x_at(y, w, h)` constructors and inner
  anchor accessors (`inner_x`, `inner_right`, `title_y`, `first_row_y`,
  `footer_y`).
- `draw_panel_frame(cells, layout, title, footer, palette)` — border +
  title at top + footer at bottom. Use for any menu-shaped window.
- `draw_menu_row(cells, layout, row_y, is_selected, label, label_fg,
  right_status: Option<(&str, Color)>, palette)` — canonical
  cursor (`>`) + label + optional right-aligned status. Truncates the
  status to fit the remaining inner width.

Adding a new menu = pick a layout, call `draw_panel_frame`, iterate
items calling `draw_menu_row`. Body that isn't a simple row list (the
multi-turn banner's progress bar) calls `put_cell` / `put_text`
directly against the same `layout` anchors.

Don't repaint the box yourself; don't compute `x + 2` inline anywhere
— go through the layout's accessors so the convention stays uniform.

### 2.10 `#[allow(dead_code)]` justification format
```rust
#[allow(dead_code)] // consumed by main.rs in phase 14 when death gate enables
pub const DEATH_ENABLED: bool = false;
```
Always cite the consuming module + phase. Audit periodically: if the
phase has passed and the symbol is still dead, it's actually dead.

---

## 3. Adding a new …

### 3.1 …`ItemKind`
1. Add variant to `ItemKind` enum (items.rs).
2. Add arm to `ItemKind::def()` (items.rs) — exhaustive match enforces
   you don't miss this.
3. Add variant to `ALL_KINDS` (items.rs module-scope const, used by
   `from_save_key` + tests).
4. (Optional) Add a test row in `save_key_round_trip_for_every_kind` —
   actually no: that test iterates `ALL_KINDS` so step 3 covers it.

### 3.2 …`ActionId` (verb)
1. Add variant to `ActionId` enum (action.rs).
2. Add entry to `ALL_ACTIONS` catalog (action.rs).
3. Add arm to `evaluate()` (action.rs) — either real logic or a phased
   stub `Availability::Unavailable { reason: "phase N: ..." }`.
4. Add arm to `execute()` (action.rs) — either `consume_and_restore`,
   custom logic, or `ExecuteOutcome::NotImplemented`.

Cost is denominated in CDDA-style moves on `ActionId::move_cost()` —
100 moves = 1 game-second at baseline speed (`Speed::BASELINE = 100`).
Executors call `world.spend_moves(id.move_cost())` for instant verbs
or `world.queue_multi_turn(&[(id, world.moves_to_seconds(id.move_cost()))])`
for multi-turn queueing. Engine-level move-cost primitives (e.g.
`MOVE_COST_TILE`) live in `world.rs`.

### 3.3 …`NeedKind`
1. Add variant to `NeedKind` enum (needs.rs).
2. Add field + accumulator field to `Needs` struct (needs.rs).
3. Add line to `Needs::starting()` for default value (needs.rs).
4. Add arm to `Needs::mut_pair()` (needs.rs) — restore/set/reset all
   pick up support automatically.
5. Add decay logic to `Needs::tick()` (needs.rs).
6. Update `action_cost_penalty_pct()` if the new need affects action
   cost (needs.rs).
7. Add fields + acc fields to `NeedsSave` (save.rs).
8. Wire in `main.rs` save_game + load path.
9. Wire in HUD `build_ui_cells` if the new need should display.
10. Add a test or two in `needs::tests`.

(Yes, 10 edits. Adding a need is genuinely a cross-cutting change.
Documenting this so it doesn't surprise anyone.)

### 3.4 …`SkillKind`
1. Add variant to `SkillKind` enum (skill.rs).
2. Add field to `Skills` struct + arms in `Skills::get/get_mut` (skill.rs).
3. Add line to `Skills::starting()` for the new skill's initial value.
4. Update `Skills::reset_daily_caps()` to zero the new skill's daily_xp.
5. Add a field to `SkillsSave` (save.rs) — additive, `#[serde(default)]`.
6. Wire save/load in main.rs (mirror the existing fire_making block).
7. Add `display_name` + (slice-2) `save_key`/`from_save_key` arms in
   `SkillKind`.
8. HUD: add a row in `build_ui_cells` if you want this skill visible.
9. Add a test or two in `skill::tests`.

Verbs that use the skill route through `skill::skill_check_with_roll(value, mods, roll)` for testability and `skill::award_xp(skill, success)` for the XP + daily cap + level-up logic. Don't reimplement those locally.

### 3.5 …`TerrainKind`
1. Add variant to `TerrainKind` enum (world.rs).
2. Add an arm to `TerrainKind::def()` (world.rs) — save_key, name,
   glyph, fg, bg, walkable, blocks_sight. The exhaustive match
   enforces this.
3. Update `chunkgen.rs` if the new terrain should spawn naturally.
4. (Optional) Add contextual verbs in `action.rs` evaluate that
   need this terrain adjacent (e.g. DrinkFromStream).
5. Add a movement / FOV / gen test in `world::tests` or
   `chunkgen::tests`.

Render + walkability + sight-blocking all read from `def()`, so no
edits to main.rs or world.rs's try_move_player / recompute_fov are
needed for normal cases. Phase 11 collapsed what used to be 8+ edits
into 2 — the `TerrainDef` table is the same pattern as `ItemDef`.

### 3.6 …debug command
1. Add variant to `DebugCommand` enum (debug_console.rs).
2. Add arm to `parse_command()` (debug_console.rs).
3. Add arm to `apply_debug_command()` (debug_console.rs).
4. Add a line to `print_help()` so QA sees the new command.

### 3.7 …save field
**Additive (backward-compatible):** add the field to the relevant Save
struct with `#[serde(default)]`. Update load + save call sites in
main.rs to plumb the value. Schema stays v1. Done.

**Schema-incompatible:** bump `SCHEMA_VERSION` in save.rs. Add a
`migrate_vN_to_vNplus1` function. Wire into `migrate_header`. Add a
frozen `vN` fixture test that round-trips through the migration.

---

## 4. Resource budget (Miyoo Mini target)

The target handheld is Cortex-A7 dual @ 1.2 GHz, 256 MB RAM, 640×480
@ 60 fps cap. Numbers that matter:

| What | Cost | Verdict |
|---|---|---|
| Per-cell-diff render of 1200 cells | ~0.2 ms / frame | free |
| Dirty-rect texture upload (one rect) | ~0.5 ms | free |
| FOV recompute, radius 20 | ~50 µs | free (per-move only) |
| Resolver eval over 13 actions | <1 µs | free |
| Atomic CBOR save (current shape) | ~50 ms | rare, OK |
| Logging 1 line to file | ~10 µs | free |
| `format!()` once per HUD-changed frame | ~5 µs | free |
| `HashSet` alloc per frame | meaningful | avoid (see FOV refactor) |
| `Vec<Option<Cell>>` realloc per frame | meaningful at 1200 entries | reuse where possible |

### Free things (don't over-think)

- Adding small fields to structs (`u8`, `u32`, `bool`)
- Arithmetic in render loops
- Per-cell function calls if they're simple match expressions
- Logging with `log_*!` macros — file write is fire-and-forget

### Things worth a second look

- Per-frame heap allocations (Vec, String, HashMap/Set)
- Anything that grows with chunk count when slice 2 lands
- Recomputing FOV more often than necessary
- Writing the save file more than once per game-second

### When the audit memo applies
Before adding code that does work every frame OR allocates non-trivially
OR holds memory proportional to a growing collection, **surface the
tradeoff in the conversation** and ask if optimization is worth the
complexity. See `~/.claude/projects/-home-campjun-projects-holy-land/memory/feedback_miyoo_resource_optimization.md`.

---

## 5. Testing conventions

### Layout
Each module that has logic carries `#[cfg(test)] mod tests` at the
bottom. Integration tests in `tests/` are unused for now (the binary
crate + per-module tests cover what we have).

### Patterns we use
- Round-trip tests for every save struct (`round_trip_X`).
- "Legacy save without new fields" tests for additive schema changes
  (`loads_old_run_save_without_pack_fields`) — these prove that old
  saves still load after additive evolution.
- Future-version-rejection tests (`rejects_future_schema`).
- "All variants of an enum work" loops (`save_key_round_trip_for_every_kind`).
- Edge cases at boundaries (out-of-bounds, capacity-zero, count-zero,
  off-by-one).

### What we DON'T test
- SDL2 surface composition (the renderer is empirically validated via
  `cargo run`).
- The frame loop itself.
- Logging output content (we test that it doesn't panic, that's it).

### Test running
`cargo test --release` is the canonical command. Currently **71 tests**
across 6 modules (items, needs, world, action, fov, save). Add a test
when:
- A new verb's evaluate or execute logic ships
- A new save field needs round-trip coverage
- A bug shipped → write the test that would have caught it

---

## 6. Forward-looking notes

Phases coming up will need these scaffolds — flag them when you start
the relevant phase:

- **Phase 9 (multi-turn action queue):** `ActionId` needs a duration
  table, `spend_action_time` becomes a partial-tick model, `ActiveAction`
  state lives on World.
- **Phase 10 (Fire Making + skills):** new `src/skill.rs`. `Skill {
  kind, level, daily_xp }` component on player. Skill checks go through
  a shared `roll_d100_vs(skill + mod)` helper. `StartFire` evaluates
  pack + adjacent debris + Fire Making skill.
- **Phase 11 (chunkgen):** new `src/chunkgen.rs`. TerrainKind grows from
  2 → ~7 variants. Add `TerrainDef` analogous to `ItemDef` to keep the
  one-edit-per-terrain promise.
- **Phase 12 (cooking/fire entity):** `Fire` as a hecs component or
  `Vec<Fire>` on World. Cooking action ties pan + fire + ingredient.
- **Phase 13 (warmth wired):** `world.find_adjacent_fire(pos)` helper
  feeds `NeedsEnv`.
- **Phase 15 (hold-Y radial):** dynamic action sorting; menu state
  caching to avoid per-frame `build_ui_cells` rebuilds.
- **Phase 19 (save schema v2):** the big bump. Position → i64 widening
  if multi-chunk traversal arrives. Friendly-reject Holy Land saves.

---

## 7. Code-style nits

- Doc-comments (`///`) on every `pub` item, even if one line.
- Block comments (`//`) at the top of every module explaining its
  charter.
- Cite phase numbers + commit hashes when the comment is forward-
  looking ("// TODO(phase-11): replace with chunkgen.rs").
- Prefer `&str` for parameters, `String` only when ownership is needed.
- Prefer `&'static str` for never-changing strings (reasons, action
  names) — the catalog uses this throughout.
- No `eprintln!` outside `logging.rs`. Use the macros.
- No `unwrap()` on user-driven Result/Option paths. `expect()` only for
  ECS invariants ("player has Position") that are programmer errors.
- Module-level constants `SCREAMING_SNAKE_CASE` with a unit suffix when
  the value is dimensional (`COST_EAT_RATION` is in seconds, not
  ticks — comment if ambiguous).
- One blank line between methods within an impl; two between top-level
  items.
- Imports grouped: `std`, external crates, `crate::`. Within each group,
  alphabetical.

When in doubt: read what's already there and follow the local pattern.
