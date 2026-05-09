# Roadmap

Session-by-session forward plan. Pulled from the chat that produced the project. See `PLAN.md` for the full design rationale (CP437, Rust+SDL2, three deployment paths, two-tier input, etc.).

The numbering is rough — sessions are sized by "amount of work that fits before a sane checkpoint," not calendar time. Slip and re-order as reality demands.

---

## Session 1 — Skeleton ✅ DONE (Half 1) / 🔁 DEFERRED (Half 2)

**Goal:** smallest viable project end-to-end. Walk an `@` around a CP437 grid via gamepad on desktop, then on Miyoo.

- ✅ **H1: Desktop hello-world.** Rust + SDL2 (bundled+static) + gilrs + image. 16x16 cell grid, runtime logical-size letterboxing. Repeat-on-hold dpad. Guybrush 16x16 atlas embedded via `include_bytes!`. Magenta-keyed glyph blit with color modulation. Floor / wall / player. Start quits.
- 🔁 **H2: Miyoo cross-compile.** `armv7-unknown-linux-musleabihf` target, statically-linked SDL2, MinUI/Onion app packaging. Bailed in-session because no actual Miyoo to test on and the sandbox didn't have a musl ARM toolchain. Resume from your dev machine.

**Resume H2 checklist:**

1. `rustup target add armv7-unknown-linux-musleabihf` (already done in tarball builds).
2. Fetch the musl ARM cross-toolchain from `musl.cc`: `armv7l-linux-musleabihf-cross.tgz`.
3. Update `.cargo/config.toml` linker path or use `cross` (Docker-based).
4. SDL2's `bundled+static-link` features should compile SDL2 from source against musl. Watch for `pkg-config` gotchas around `libudev-sys` (transitive via gilrs); may need to disable gilrs hot-plug or stub udev.
5. Package: binary + `assets/` + a launch shell script in a folder dropped on the Miyoo SD card under `App/HolyLand/`.
6. Test: launch from device menu, confirm dpad walks the `@`, Start quits.

---

## Session 2 — Android (Retroid Pocket 5)

**Goal:** same hello-world running on a Retroid-class Android handheld.

- `rustup target add aarch64-linux-android`.
- Add `cargo-ndk` for the build, `cargo-apk` (or hand-rolled gradle) for packaging.
- Implement `src/platform/android.rs`: SDL2 `SDL_AndroidGetActivity` for asset manager access, lifecycle hooks (`onPause`/`onResume` translate to game-loop pause).
- Bundle assets via the APK asset path; `include_bytes!` for the atlas already works platform-agnostically.
- Test on Retroid Pocket 5; cohort (Odin 2, AYN Odyssey, RP4 Pro) inherits for free if the touch/lifecycle paths are clean.
- Estimated 1–2 sessions of build/packaging pain; application code shouldn't change.

**Acceptance:** `@` walks via gamepad on RP5, lifecycle suspend/resume doesn't crash.

---

## Session 3 — ECS + Saves ✅ DONE (early)

**Goal:** architectural foundation that everything else assumes.

- ✅ **hecs ECS.** Player is a real entity with `Position`, `Renderable`, `Player` components. Tilemap stays `Vec<Tile>`.
- ✅ **CBOR saves with schema versioning.** `SaveHeader { schema_version, build_version, save_counter, device_id (Uuid), timestamp }`. Atomic writes (`.tmp` + `fsync` + rename). Forward-compat via `#[serde(default)]` on every non-header field. Migration chain dispatched by `schema_version`. Future-version saves rejected with explanatory error.
- ✅ **Two-tier saves.** `meta.cbor` (xp, demon-currency, deity affinity, unlocks; sync-friendly) + `run.cbor` (current run state, local-only).
- ✅ **Tests.** Round-trip meta + run, monotonic counter (preserves `device_id`), rejects-future-schema. `cargo test --release` is green.
- ✅ `Select` saves both; auto-load on launch.

**Still open in S3:** when more entity types arrive, RunSave needs to serialize ECS state (per-component blocks, not blanket hecs serde — explicit). Don't paint into a corner.

---

## Session 4 — Tiny Oasis + First NPC

**Goal:** first taste of the Stardew oasis loop. Not a real game yet, but proves the dialogue + interact path.

- Hand-authored oasis tilemap (small, ~30×20). Real glyph variety: water (`~`), palm (`T`), sand (`.`), path (`,`), well (`o`), wall variations.
- One NPC entity: `Position`, `Renderable`, `Dialogue { lines: Vec<String> }`.
- Interact button (A) when adjacent to NPC opens a dialogue box (CP437-rendered, gamepad-navigable, A advances).
- Persistent dialogue state in `MetaSave` (which lines have been seen).
- Camera follows player when oasis exceeds the viewport.

**Acceptance:** walk to NPC, press A, read 3 lines, dialogue dismisses, save+reload remembers the conversation happened.

---

## Session 5 — Harvest + Inventory

**Goal:** Stardew "do a thing, get a thing, store a thing" loop.

- One harvestable tile type (e.g. wheat). Interact button on it: tile becomes harvested-state, item enters inventory.
- `Inventory` component on player: `Vec<ItemStack>`. Items defined as data (id, name, glyph, fg).
- Inventory menu (Y opens, dpad navigates, gamepad-only). Renders as a CP437 panel.
- Harvested tiles regrow on a timer stored in the tile (use `Vec<Tile>` extension or per-tile component).
- Persist inventory + harvest state in `RunSave` (and copy to `MetaSave` for the persistent oasis state).

**Acceptance:** harvest 5 wheat, open inventory, see them; quit, reload, still there.

---

## Session 6 — Wilderness + First Demon

**Goal:** prove the CDDA wilderness loop works alongside the oasis loop.

- One wilderness tile (the desert just outside the oasis). Hardcoded layout, ~60×40. Boundary tile teleports player back to oasis.
- One demon entity: `Position`, `Renderable`, `Hostile`, `Health`, simple chase AI (move toward player on each player turn).
- Combat: bump-attack (move into demon = attack). Demon dies, drops `demon_essence` currency.
- **Death:** if player health hits 0 in wilderness, drop all inventory items as ground entities at death position, respawn at oasis with full health. `meta.demon_currency` and oasis state persist; `run.cbor` resets the wilderness.
- Currency stays in `MetaSave` (it's the meta progression hook).

**Acceptance:** walk into wilderness, kill demon, get currency. Walk into demon, die, respawn at oasis with currency intact, items lost.

---

## Session 7 — First Deity Blessing + Run Loop

**Goal:** the roguelike layer that sits on top of everything.

- Run starts when you leave the oasis. At that moment, offer 2–3 random blessings from a pool (Asherah, Ishtar, Hathor, Marduk, Yahweh-as-warrior, etc.). Pick one.
- Each blessing is a component or a systemic effect. E.g. Ishtar's blessing = +25% damage; Hathor = +1 HP regen / 5 turns; Asherah = wooden weapons +50% durability.
- Blessing persists for the run; cleared on respawn at oasis.
- Display active blessing in HUD (top-right CP437 strip).
- Add 1–2 more demons + 1–2 more blessings to start surfacing combo space.

**Acceptance:** every run picks a blessing, blessing visibly affects combat, dying clears it.

---

## Session 8+ — Depth Pass

Roughly in this order, each ~1–3 sessions:

- **Cultural ruin biomes.** Egyptian / Babylonian / Assyrian / Canaanite tilesets (palette swaps, glyph remixes, distinct enemy + item rosters). Wilderness gets multiple zones; the run becomes "go deeper, get more valuable salvage."
- **Tile-mode sprite override.** Optional 16×16 PNG sprites layered on top of the CP437 contract. Can be enabled per-glyph; ASCII remains canonical.
- **Audio.** `cpal` for output, custom mod/tracker engine (`it`/`xm` decoder) per the original audio sketch. Mood layering (overworld vs. ruin vs. demon-presence).
- **Analog stick action wheels (Tier 2 input).** Action wheels for stick-equipped devices (Retroid, Odin). Same `Action` enum as Tier 1; wheel is purely a UI layer.
- **NPC schedules + relationships.** Stardew-shaped: NPCs have daily routines, gift preferences, friendship levels.
- **Crafting.** Recipes as data; workbench tiles in oasis.
- **Building / oasis expansion.** Spend salvaged materials to expand the oasis grid; defeated-demon currency unlocks new buildings.
- **Save migrations.** First real schema bump will probably happen here. Validate the migration chain works end-to-end.
- **Settings menu, audio mixing, accessibility.** Lower-priority but ship-blocking eventually.

---

## Cross-cutting rules (do not violate)

- **All rendering through `draw_glyph`.** Tile-mode and GLES backends swap behind this. Game code never touches SDL directly.
- **All input through `Action`.** Wheel selections will emit the same enum.
- **All platform-specific code in `src/platform/`.** Never call OS APIs from anywhere else.
- **Saves are forward-compat.** Every new field is `#[serde(default)]`. Every schema bump adds a migration function. Old saves never silently break.
- **Single-threaded game loop.** Android lifecycle is hostile to background threads. Don't start parallelizing without a real reason.
- **Test what survives a year.** Round-trip tests on every save struct. UI feel can be tested by humans; data correctness cannot.
