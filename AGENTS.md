# Holy Land — Agent Handoff Reference

Snapshot for any agent (Codex / Claude / etc.) picking up the Miyoo Mini Plus port.
Written 2026-05-11. Pair with `holyland-PLAN.md` and `holyland-ROADMAP.md` for the broader game design; this file covers the runtime + deploy specifics.

## Project shape

- Rust + SDL2 (sdl2 0.37) CP437 roguelike. Entry: `src/main.rs`.
- Modules: `input`, `platform`, `render`, `save`, `world`. ECS via `hecs`.
- Two build targets:
  - **Desktop** (Linux): `cargo build` — uses bundled+static SDL2 (Cargo target-conditional).
  - **Miyoo Mini Plus / Onion OS**: `./cross/build-onion.sh` — cross-compiles `armv7-unknown-linux-gnueabihf`, links against Onion's prebuilt SDL2, packages into `target/onion/HolyLand/`.

## Render pipeline (B-style per-cell diff)

`src/main.rs` and `src/render.rs`. Pattern:

1. CPU-side `Surface` (`framebuf`) holds the composed framebuffer (`PixelFormatEnum::ARGB8888`, 640×480).
2. Atlas is a CPU `Surface` (also ARGB8888); `set_color_mod` + `set_alpha_mod` + `blit` for tinted glyphs.
3. `prev_cells: Vec<Option<Cell>>` mirrors what was last painted. Each frame walks the viewport (40×30 = 1200 cells), composes terrain+entity into a `Cell { glyph, fg, bg }`, and only blits if it differs from `prev_cells[i]`.
4. Entire framebuf is uploaded to a single streaming texture (`present_tex.update`) once per frame, then `canvas.copy + canvas.present`.

**Why this shape:** Onion's mmiyoo SDL2 renderer drops most per-call work (`QueueCopy` keeps only one rect, `QueueFillRects` no-op, `SetTextureColorMod`/`AlphaMod`/`BlendMode` no-op, `SetVSync` no-op). The only reliable path is "compose into a CPU surface, upload once, present once."

**Idle frame cost** = ~0 cell blits. **Move = 2 cells**.

## Camera + world API (open-world ready)

- `World::tile_at(wx: i64, wy: i64) -> Tile` is the canonical query. Out-of-bounds returns `Tile::Wall`.
- Render loop reads world coords through `tile_at(cam_x + vx, cam_y + vy)`.
- Camera currently anchored at `(0, 0)` because the world fits the viewport. Two-line swap to player-centered when the world grows:
  ```rust
  let cam_x = player.x as i64 - WORLD_W as i64 / 2;
  let cam_y = player.y as i64 - WORLD_H as i64 / 2;
  ```
- `Position` (ECS component) is still `i32`; the public render contract is `i64` so widening Position later doesn't ripple.

## CDDA-scale plan (not yet implemented)

When the world stops fitting the viewport:

- **Coords:** widen `Position` and `RunSave.player_{x,y}` to `i64`. Bump `save::SCHEMA_VERSION` to 2 and add a v1→v2 migration (CBOR int widening is value-compatible; migration is bookkeeping).
- **Storage:** chunks (suggested 32×32 or 64×64). `World { chunks: HashMap<ChunkCoord, Box<Chunk>>, seed: u64, save_dir: PathBuf }`. Generate on demand from `(seed, cx, cy)`.
- **LRU cache:** keep ~9–25 chunks resident around player; evict-with-persist if mutated, drop otherwise (procgen recreates).
- **Entities:** `hecs` already in deps; entities carry world coords; tick only loaded chunks.
- **Renderer:** unchanged. It only ever scans the viewport.

## Miyoo / Onion runtime knowledge

### mmiyoo SDL2 quirks (XK9274/sdl2_miyoo)
- `MMIYOO_SetVSync` is `return 0` — disabling vsync via API is a no-op.
- `MMIYOO_RenderPresent` → `GFX_Flip()` → `ioctl(fb_dev, FBIOPAN_DISPLAY, ...)`. **No `FBIO_WAITFORVSYNC` in the per-frame path** — present is a non-blocking page flip.
- Renderer info reports `flags=0xa` = `ACCELERATED(0x2) | TARGETTEXTURE(0x8)`. PRESENTVSYNC (0x4) is **not** set.
- Texture create only accepts `ARGB8888` and `RGB565`. `RGBA32` (= `ABGR8888` on LE ARM) is rejected.
- 32×32 source rects are explicitly dropped by `QueueCopy`.

### Required env vars (in `launch.sh`)
```sh
export SDL_VIDEODRIVER=mmiyoo
export SDL_AUDIODRIVER=mmiyoo
export EGL_VIDEODRIVER=mmiyoo
export LD_LIBRARY_PATH="$mydir:/mnt/SDCARD/.tmp_update/lib/parasyte:/customer/lib:/customer/lib/parasyte:$LD_LIBRARY_PATH"
```
Without these, `SDL_VideoInit` fails with "No available video device."

### Kernel keymap (Miyoo Mini Plus)
A=Space, B=LCtrl, X=LShift, Y=LAlt, L=Tab, R=Backspace, Start=Enter, Select=RCtrl, Menu=Esc. Both desktop convention (WASD/Z/X/...) and Miyoo keys are mapped in `src/input.rs::keycode_to_action`.

### gilrs is desktop-only
Made an optional cargo feature (default-on for desktop, off for Miyoo build). The cross build uses `--no-default-features` to skip libudev-sys.

## Deploy / iterate workflow

1. Edit code.
2. `cargo build` (desktop sanity).
3. `./cross/build-onion.sh` → `target/onion/HolyLand/`.
4. Push to device. Two paths:
   - **FTP** (Onion's built-in, default `onion:onion` on port 21): `curl --connect-timeout 8 --max-time 120 -u onion:onion -T target/onion/HolyLand/holyland "ftp://<miyoo-ip>/App/HolyLand/holyland"`. **Always verify remote size** — busybox FTP under flaky WiFi can return 426 mid-transfer; curl reports 100% but the file is truncated. Loop until `LIST` shows the expected byte count.
   - **SD shuttle**: copy `target/onion/HolyLand/` to SD card `/App/HolyLand/`, eject, insert.
5. Logs are written to `App/HolyLand/holyland.log` on the SD card. Pull with `curl -u onion:onion "ftp://<ip>/App/HolyLand/holyland.log"`.

### Miyoo WiFi caveat
MT7601 chip, very flaky. Drops association when screen idles. Open the FTP tool screen on the device to keep the radio alive during transfer. If a transfer stalls or the IP is unreachable, the user has to wake the device.

## Frame loop tuning

- `TARGET_FRAME = Duration::from_micros(16_667)` (60fps cap).
- `std::thread::sleep(TARGET_FRAME - elapsed)` after `present`.
- `eprintln!("fps: {}", fps_count)` once per second to the log.
- mmiyoo present doesn't block on vblank, so the cap is the real frame pacer. Without it the loop would burn CPU.

## What is NOT done yet

- World > viewport (camera scrolling). Comment in `main.rs` shows the swap.
- i64 Position + save schema v2 migration.
- Chunk/persistence layer.
- Late input sampling (poll events again right before render to shave one frame of input lag). Not needed yet — game is turn-based-feel.
- FOV / lighting.
- NPC entities (only player exists).

## Files of note

- `src/main.rs` — main loop, render orchestration, camera, fps counter.
- `src/render.rs` — `load_atlas`, `draw_glyph` (surface→surface).
- `src/world.rs` — `World`, `Tile`, `Position`, `tile_at(i64, i64)`.
- `src/input.rs` — Action enum, SDL keycode + gilrs button mapping, repeat handling.
- `src/save.rs` — CBOR save format, schema versioning discipline. **Read the header comment** before changing save structs.
- `src/platform.rs` — `save_dir()`.
- `cross/Dockerfile.rust` — Podman image: union-miyoomini-toolchain + rustup + ARM target.
- `cross/build-onion.sh` — wrapper that runs the container, sets the right RUSTFLAGS, assembles the app folder.
- `cross/onion-libs/` — Onion's libSDL2/libEGL/libGLESv2 + headers + sdl2.pc (with `prefix=/`).
- `cross/onion-pkg/launch.sh` — runtime env setup, suspends MainUI, runs binary, restores MainUI.
- `cross/onion-pkg/config.json` — Onion app menu entry.
- `Cargo.toml` — note the target-conditional sdl2 feature block at the **end** of `[dependencies]`. If you put it in the middle, Cargo sweeps following deps into the conditional. Bit me once.
