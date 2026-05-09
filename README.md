# Holy Land

Low-fantasy biblical sim/roguelike. CP437 rendering, gamepad-first, cross-platform from desktop to retro handhelds.

See `/root/.claude/plans/yes-but-before-deciding-keen-stonebraker.md` for the full design plan.

## Status

**Session 1/H1 + Session 3 done.** A `@` walks around a CP437 grid (Guybrush 16x16 tileset); `#` blocks; `.` is floor; Start / Esc quits; **Select saves**; player position auto-loads on next launch. Saves are CBOR with schema versioning and atomic write — portable + sync-friendly per plan. Miyoo cross-compile (S1/H2) and Android build (S2) deferred.

## Build (desktop)

SDL2 is bundled and statically linked via the `sdl2` crate's `bundled` + `static-link` features, so no system SDL2 install is required. First build is slow (compiles SDL2 from source); subsequent builds are fast.

```
cargo run --release
```

Requires: Rust toolchain, a C compiler (cc/gcc), `cmake`, `make`.

## Controls

Tier 1 (floor — every device, including SNES-class handhelds):

| Action      | Keyboard                         | Gamepad          |
|-------------|----------------------------------|------------------|
| Move        | Arrow keys / WASD / HJKL         | Dpad             |
| A / B / X / Y | Z / X / C / V                  | Face buttons     |
| L / R       | Q / E                            | Shoulder buttons |
| Save        | Backspace / Right Shift          | Select           |
| Quit        | Enter / Esc                      | Start            |

Tier 2 (analog sticks → action wheels) is reserved for Session 3+; see plan.

## Project layout

```
holyland/
├── Cargo.toml                bundled+static SDL2 + gilrs + image + hecs + serde + ciborium + uuid
├── .cargo/config.toml        cross-compile target stubs
├── assets/cp437_16x16.png    Guybrush CP437 tileset (embedded via include_bytes!)
└── src/
    ├── main.rs               SDL init + event loop + save/load wiring
    ├── render.rs             draw_glyph + load_atlas (the only drawing API)
    ├── input.rs              Action enum + gilrs/keyboard + repeat-on-hold
    ├── world.rs              hecs ECS + Tile/Position/Renderable/Player components
    ├── save.rs               SaveHeader + MetaSave + RunSave + atomic write + migration chain
    └── platform/             OS-specific code lives here, nowhere else
        ├── mod.rs
        ├── desktop.rs        XDG paths, fallback to exe-adjacent saves
        ├── linux_handheld.rs reserved for Miyoo/MinUI specifics
        └── android.rs        stub for Retroid (Session 2)
```

## Saves

CBOR via `ciborium`, written atomically (`.tmp` + `fsync` + rename). Two files per `save_dir`:

- `meta.cbor` — xp, demon-currency, deity affinity, unlocks, settings (the always-sync tier).
- `run.cbor` — current run state (player position; will grow as the wilderness loop lands).

Every save starts with a `SaveHeader { schema_version, build_version, save_counter, device_id, timestamp }`. Loads dispatch by `schema_version`; future bumps add migration functions to `save.rs`. Forward-compat by `#[serde(default)]` on every non-header field — older binaries skip unknown fields, newer binaries default-fill missing ones.

`save_dir` resolves per-platform via `src/platform/`:

- Linux desktop / handheld: `$XDG_DATA_HOME/holyland` → `$HOME/.local/share/holyland` → exe-adjacent `saves/`.
- Android (Session 2): `/data/local/holyland` placeholder — will switch to SDL's internal storage path.

Run `cargo test --release` to exercise the round-trip + future-schema-rejection tests in `save.rs`.

## Cross-compile (Session 1 / Half 2 — Miyoo Mini Plus)

Not yet attempted; planned next. Target triple `armv7-unknown-linux-musleabihf`. Will require a musl ARM cross toolchain and a statically-buildable SDL2. See plan for budget and bail-out criteria.

## Cross-compile (Session 2 — Retroid Pocket 5)

Target triple `aarch64-linux-android`. Will use `cargo-ndk` + `cargo-apk`. Application code unchanged; `src/platform/android.rs` becomes live.

## What's deliberately not here yet

Audio, NPCs, harvesting, combat, blessings, demons, oasis/wilderness loops, action wheels (Tier 2 input), tile-mode sprites. All deferred per plan.
