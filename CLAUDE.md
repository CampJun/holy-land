# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Required reading

- `AGENTS.md` — the authoritative agent handoff. Render pipeline rationale, mmiyoo SDL2 quirks, deploy workflow, what's NOT done yet. Read this before editing render/input/platform code.
- `holyland-PLAN.md` — full design plan.
- `holyland-ROADMAP.md` — session-by-session forward plan.
- `src/save.rs` header comment — save format / schema evolution discipline. Read before changing any save struct.

## Commands

```
cargo run --release            # desktop dev loop
cargo build                    # desktop sanity check
cargo test --release           # save round-trip + future-schema-rejection tests
cargo test --release save::    # subset
./cross/build-onion.sh         # cross-build Miyoo Mini Plus / Onion app folder
./cross/deploy-onion.sh <ip>   # FTP deploy to Miyoo (verifies remote size)
```

Desktop builds bundle+statically link SDL2 (no system SDL2-devel needed); first build is slow. Requires a C compiler, `cmake`, `make`, and `podman` for the cross build.

The cross build runs inside `miyoomini-rust` (built from `cross/Dockerfile.rust`) and uses `--no-default-features` so `gilrs` is dropped (avoids libudev-sys). It links against Onion's libs under `cross/onion-libs/`; the Miyoo loads them via `LD_LIBRARY_PATH` at runtime.

## Architecture

CP437 SDL2 roguelike, `hecs` ECS, gamepad-first, single binary, two real targets: x86_64 desktop and `armv7-unknown-linux-gnueabihf` (Miyoo Mini Plus / Onion). Android is a stub.

**Module layout** (`src/`):
- `main.rs` — SDL init, event loop, render orchestration, camera, fps counter, save/load wiring.
- `render.rs` — `load_atlas` + `draw_glyph` (surface→surface). The only drawing API.
- `input.rs` — `Action` enum, SDL keycode + gilrs button mapping, repeat-on-hold. Both desktop conventions (WASD/ZXCV) and Miyoo kernel keymap are mapped here.
- `world.rs` — `World`, `Tile`, `Position`, `Renderable`, `Player`, `Inventory`, `Region`. Canonical query is `World::tile_at(wx: i64, wy: i64) -> Tile` — render reads world through this.
- `save.rs` — CBOR via `ciborium`, `SaveHeader` + `MetaSave` + `RunSave`, atomic write (`.tmp` + `fsync` + rename), schema-versioned migration chain.
- `platform/mod.rs` dispatches by cfg to `desktop.rs` / `linux_handheld.rs` / `android.rs`. OS APIs live only here.

**Render pipeline (B-style per-cell diff)** — compose into a CPU `Surface` (ARGB8888, 640×480), diff against `prev_cells`, blit only changed cells, upload the whole framebuffer once per frame, present. This shape exists because Onion's mmiyoo SDL2 renderer no-ops `SetTextureColorMod`/`AlphaMod`/`BlendMode`/`SetVSync` and drops most `QueueCopy` work. **Don't refactor it back to per-glyph `canvas.copy`** — it will look fine on desktop and break on Miyoo. See AGENTS.md "Render pipeline" and "mmiyoo SDL2 quirks" for the full reason.

**Frame pacing** — `TARGET_FRAME = 16_667 µs`; sleep after present. mmiyoo present is a non-blocking page flip (no vblank wait), so this cap is the real pacer.

**Camera + coords** — render reads `tile_at(cam_x + vx, cam_y + vy)` with `i64`. `Position` ECS component is still `i32`; the public render contract is `i64` so widening Position later doesn't ripple. Camera is currently anchored at `(0, 0)` because the world fits the viewport; AGENTS.md shows the two-line swap to player-centered.

**Saves** — two files in `save_dir`:
- `meta.cbor` — xp, demon-currency, deity affinity, unlocks, settings (always-sync tier).
- `run.cbor` — current run state (player position; grows with the wilderness loop).

Every save starts with `SaveHeader { schema_version, build_version, save_counter, device_id, timestamp }`. Forward-compat by `#[serde(default)]` on every non-header field. To change a save struct: bump `SCHEMA_VERSION`, add a `migrate_vN_to_vNplus1`, wire it into `migrate_header`. Purely-additive fields need no migration code.

`save_dir` per platform: `$XDG_DATA_HOME/holyland` → `$HOME/.local/share/holyland` → exe-adjacent `saves/` on Linux; Android stub today.

## Conventions worth knowing

- **`Cargo.toml` target-conditional `sdl2` block must stay at the end of `[dependencies]`.** Putting it mid-list sweeps following deps into the conditional.
- **`gilrs` is a default-on feature, off for Miyoo** — desktop gets gamepads, cross build skips libudev-sys.
- **Atlas / assets are embedded with `include_bytes!`** (see `ATLAS_PNG` in `main.rs`). The cross packaging step also copies `assets/` next to the binary; keep both paths working if you add new assets.
- **Miyoo FTP is flaky** — `deploy-onion.sh` verifies remote byte size after upload. busybox FTP can report success after a truncated transfer. Don't remove the size check.
- **Logs on device** land at `App/HolyLand/holyland.log` on the Miyoo SD card; pull with `curl -u onion:onion "ftp://<ip>/App/HolyLand/holyland.log"`.

## Working principles

### 1. Think before coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity first

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical changes

Touch only what you must. Clean up only your own mess.

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that your changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: every changed line should trace directly to the user's request.

### 4. Goal-driven execution

Define success criteria. Loop until verified.

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.
