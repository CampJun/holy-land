# First Prototype Plan — Holy Land CP437 Sim

> **2026-05-14 amendment — combat & skills system reworked.**
> The blessing model described in this doc (passive stat buffs per deity, e.g. "Ishtar +25% damage") is **superseded**. Combat is now a 100-AP-per-turn economy with 5 WoW-style attributes (Stam/Str/Agi/Int/Spi), per-weapon AP swing costs, Might-based damage scaling, and **active deity-granted powers** modified by **multiplicatively-stacking blessings**. Each god grants one power + one blessing. Skills are Runescape-shaped: 15 total (1 combat + 2 crafting per attribute), feeding attributes via 10-level milestones — so a "Stardew day" of crafting in the oasis legitimately powers up the character. See kanban cards [[Combat math foundation]], [[Action point system]], [[Power system]], [[Blessing system]], [[Skill system]], [[Enemy tiered stat blocks]], [[Combat HUD]], [[Action enum overhaul]], [[16-god roster design]].

## Context

Greenfield game project. Through conversation we landed on:

- **Setting:** low-fantasy biblical Holy Land. Salvageable ruins (Canaanite, Babylonian, Assyrian, Egyptian, biblical) infected by demons. Player base is an oasis they expand.
- **Loops:** Stardew-style oasis (social sim, crafting, harvesting) + CDDA-style wilderness (deep sim, survival). Wilderness runs are roguelike: drop items on death, respawn at oasis. Meta progression (xp/levels + demon-currency) persists. Roguelike blessings from deities (Asherah, Ishtar, Hathor, etc.) stack/combo per run.
- **Rendering:** CP437 with `(glyph, fg, bg)` as the source-of-truth contract. Optional tile sprites layered on top later, à la Caves of Qud. Color palettes do the cultural distinctiveness work (Egyptian ochre/lapis, Babylonian bronze/indigo, Assyrian iron/oxblood, Canaanite olive/bone; demon-infected = desaturate + sickly overlay).
- **Input:** gamepad-first. **Floor: dpad + 6 face/shoulder buttons + Start + Select** (the SNES/Miyoo baseline — A, B, X, Y, L, R, Start, Select). **Ceiling: dual analog sticks driving radial action wheels** for Retroid-class devices, surfacing CDDA-depth actions (examine, craft, throw, cast blessing, eat, drop, talk, look) without bloating the button count. The wheel is an additive UX layer for stick-equipped devices, not a requirement — every action accessible via a wheel must also be reachable via menu on SNES-class devices, even if slower. This is a **Session 3+** concern; flagged here so Session 1's input architecture doesn't paint us into a corner.
- **Portability targets (heterogeneous — three distinct deployment paths):**
  - **Desktop** (Linux/macOS/Windows x86_64) — easy case.
  - **Linux retro handhelds** — Miyoo Mini Plus (ARMv7 Cortex-A7, 128MB RAM, glibc Linux) and the wider family (RG35XX, TrimUI Smart Pro, Anbernic mainline). musl static binary, framebuffer or basic GLES, 4:3 @ 640×480 typical.
  - **Android handhelds** — Retroid Pocket 5 (Snapdragon 865, AArch64, 8GB RAM, Android), and the cohort it lives in (Odin 2, AYN Odyssey, RP4 Pro). APK packaging, GLES 3+, 16:9 @ 1080p typical, full Android lifecycle (onPause/onResume).
  - These are *different OSes*, not just different ABIs. The application code is shared; the build pipelines diverge. SDL2 is the abstraction layer that makes one codebase cover all three.
- **Language:** Rust. Picked because the user is a vibe coder working through Claude CLI; the compiler converts subtle bugs (which AI-generated code is prone to) into immediate red errors instead of save-corrupting bugs that surface months later. Cargo also keeps the toolchain conversation short.
- **Risk to test first:** Rust on Miyoo Mini Plus is architecturally sound (`armv7-unknown-linux-musleabihf` is Tier 2) but not a well-trodden homebrew path. Rust + SDL2 on Android is well-trodden but adds packaging complexity. We validate Miyoo first (it's the more constrained, more novel target — if it works, Android is a known-effort follow-up).

## Portability Architecture (Load-Bearing From Day One)

Because we're targeting three OS families, not just three ABIs, the codebase has to assume portability from the first commit. The constraints:

- **All platform-specific code lives in `src/platform/`.** Nothing else in the codebase calls OS-level APIs directly. File I/O, gamepad init, window creation, asset loading — all routed through this module. Backends: `desktop.rs`, `linux_handheld.rs` (musl/framebuffer specifics), `android.rs` (asset manager, lifecycle hooks). Selected at compile time via cfg.
- **All rendering goes through `draw_glyph(x, y, glyph: u8, fg, bg)`.** This single primitive is the entire renderer's public API. Tile-mode upgrade later, GLES vs. software backend swap later — none of it touches game code.
- **All input goes through an `Action` enum.** Gamepad events never leak into game logic. Keyboard support on desktop maps to the same enum. The `Action` enum is the universal currency: button-driven inputs and (later) analog-stick action-wheel selections both emit the same actions. The wheel is a UI layer above the input layer, not a parallel input system.
- **Two input tiers, one action vocabulary.** Tier 1 (floor): dpad + 8 buttons (Miyoo-class). Tier 2 (ceiling): adds dual analog sticks + L2/R2/L3/R3 (Retroid-class). Tier 2 surfaces a radial action wheel for CDDA-depth actions; Tier 1 reaches the same actions via menu navigation. Architecture must allow the wheel to be added without restructuring input — i.e., adding a stick-event branch in `input.rs` should be additive, not invasive.
- **Flexible grid sizing.** Miyoo is 4:3 @ 640×480; Retroid is 16:9 @ 1080p; desktop is whatever. The grid is defined in *cells* (e.g. 60×34); pixel size and letterboxing are computed at runtime per platform. No hardcoded resolutions.
- **No threading assumptions.** Android lifecycle is hostile to background threads. Single-threaded game loop until proven necessary.
- **No filesystem assumptions outside `platform/`.** Android uses asset manager; Linux uses plain paths.

## Audio Strategy (Decided Now, Implemented Later)

User wants runtime-synthesized / sequenced music that reacts to gameplay, not streamed MP3s. "PS1 as goal" reframed honestly: PS1 was mostly sample-based playback (CD-DA + ADPCM through hardware reverb), not pure synthesis. Achievable target = **PS1-era production quality via dynamic sequenced playback**, with a path to custom synthesis later.

**Decisions:**

- **Audio output: `cpal`** (cross-platform: ALSA on Linux/Miyoo, AAudio/OpenSL on Android, CoreAudio/WASAPI on desktop). Audio's equivalent of SDL2.
- **Mixer/dynamic-layering: `kira`** — game-oriented, stem crossfading, parameter modulation. Built on cpal.
- **Music engine: tracker-based first** (libxmp Rust binding or a pure-Rust tracker). Tiny files, very low CPU (~negligible on Miyoo), dynamic by nature — game state mutes/unmutes channels, swaps patterns, modulates tempo. Pattern files (.xm/.it/.mod) compose like sequenced music with sampled instruments.
- **Upgrade path to PS1-grade production:** swap tracker for MIDI+SoundFont (oxisynth). Higher CPU but closer to actual PS1 sound (General MIDI + sample banks is how FF7-era worked). Decide if/when game proves out.
- **Custom synthesis path (the "insane" version):** keep open via the trait below. If a custom sonic identity becomes worth months of synth-engine work, write voices in `fundsp` and drive them from the same tracker conductor. Not a wasted decision either way.

**Architecture:**

```rust
trait MusicEngine {
    fn play_song(&mut self, song: SongHandle);
    fn set_layer_volume(&mut self, layer: LayerId, vol: f32);  // game→music coupling
    fn set_intensity_param(&mut self, param: ParamId, value: f32);
    fn stop(&mut self);
}
```

Tracker backend implements this first; synth backend later implements the same trait. Game code couples to the trait, never the backend.

**CPU budget honesty for Miyoo:** dual-core Cortex-A7 @ 1.2GHz — audio gets 5–10% of one core responsibly. Tracker mixing fits comfortably. MIDI+SF2 fits with discipline (limit voices, skip reverb). Pure synthesis fits only voice-limited (≤8) without expensive filters. Plan accordingly.

**Implementation timing:** not Session 1. Slots in around Session 5–6 (after oasis tilemap exists, before NPC density grows). Library choices locked now so the platform abstraction doesn't get rewritten.

## Save Strategy (Portable + Sync-Friendly From Day One)

User requirement: saves must be portable across all three deployment targets and sync-friendly from the start. This means schema discipline applies to the *first* save ever written — retrofitting later corrupts existing saves.

**Decisions:**

- **Format: CBOR via `ciborium`.** Compact, language-agnostic, deterministic across endianness/platform. No raw `bincode` (Rust-only). JSON kept as optional debug-export for development.
- **Serialization: `serde`** for everything, with `#[serde(deny_unknown_fields)]` off (forward-compat: newer saves with extra fields readable by older binaries that ignore them).
- **Schema versioning: every save starts with a header.**
  ```rust
  struct SaveHeader {
      schema_version: u32,
      build_version: String,    // semver of the game build that wrote it
      save_counter: u64,        // monotonic, increments on each write
      device_id: Uuid,          // random per install, stable across saves
      timestamp: i64,           // unix seconds, UTC
  }
  ```
  On load, dispatch by `schema_version` to a migration chain. Without this, sync between an older device and a newer one corrupts. With it, the schema can evolve for years.
- **Atomic writes.** Write to `save.cbor.tmp`, `fsync`, rename to `save.cbor`. Prevents half-written saves on crash/power-loss (common on handhelds).
- **Two-tier saves:**
  - **Meta save** (small, always-sync candidate): xp, demon-currency, unlocks, deity-affinity, settings. Few KB. Cheap to sync everywhere.
  - **Run save** (larger, local-by-default): in-progress wilderness run state. Optional sync. Lost-on-death anyway, so sync matters less.
- **Sync conflict resolution: last-write-wins by default, supported by `save_counter` + `device_id` in the header.** If a richer conflict UI is wanted later (keep both, prompt user), the data is already there.
- **Location vs format decoupled.** *Where* a save lives is platform-specific (XDG on Linux, app-private on Android, Miyoo SD path) — that lives in `src/platform/`. *What* a save contains is portable — that's `src/save.rs` and is byte-identical on every device.
- **Cloud-sync surfaces deferred.** Steam Cloud / Google Play Saves / manual export-import are platform integrations done much later. The architecture above means any sync layer is a pure addition, not a rewrite.
- **Compression: optional, deferred.** zstd if file size matters; skip until measured.

**Implementation timing:** Session 3, immediately after ECS lands. Every subsequent save-schema change ships with a migration. The first save format we write is the format every future save format must migrate from.

## Approach: Hybrid Hello-World, Session 1

Single goal: end the session with a `@` walking around a CP437 grid via gamepad, on desktop and ideally on the Miyoo. Android (Retroid) is **deferred to Session 2** to keep Session 1 focused — but the architecture above ensures it's a build-pipeline problem, not a code-rewrite problem. Split into two halves so we always end with something working.

### Half 1 — Desktop hello-world

Stack:

- **`sdl2` crate** (with bundled SDL2 features off — link against system SDL2 on desktop, statically link for handheld).
- **`gilrs`** for gamepad input. Cleaner abstraction than raw SDL2 joystick, handles SNES-style mappings well, and exposes analog stick axes natively — important for the Tier-2 action-wheel work later.
- **No ECS yet.** Just a `World` struct with a player position and a tilemap. ECS comes when we have more than one entity type.
- **CP437 font atlas:** ship a single PNG (e.g. the classic 8x8 or 8x16 IBM VGA font) bundled in `assets/`. Render via SDL2 texture, source rect = glyph index.
- **Grid choice:** 16x16 cells (square — leans Stardew, plays nicely with gamepad cursor). Locked decision; tile-mode sprites later will assume this.

Repo skeleton:

```
holyland/
├── Cargo.toml
├── assets/
│   └── cp437_8x16.png        # IBM VGA font atlas, 16x16 glyphs
├── src/
│   ├── main.rs               # event loop, init
│   ├── render.rs             # CP437 glyph blitter, palette
│   ├── input.rs              # Action enum, gamepad mapping
│   ├── world.rs              # tilemap + player struct (placeholder)
│   └── platform/
│       ├── mod.rs            # trait/cfg-gated platform abstraction
│       ├── desktop.rs        # cfg(not(target_os = "android")) + std file I/O
│       ├── linux_handheld.rs # musl-specific bits, deferred until Half 2
│       └── android.rs        # cfg(target_os = "android"), deferred to Session 2
└── README.md
```

Dependencies (`Cargo.toml`):

```toml
[dependencies]
sdl2 = "0.37"
gilrs = "0.10"
```

Acceptance criteria for Half 1:

- Window opens at a CP437 grid defined in *cells* (e.g. 40×30), with pixel size computed at runtime. Don't hardcode 640×480 or 1920×1080.
- A `@` glyph renders in default white-on-black.
- Dpad moves the `@` one cell per press (with simple repeat-on-hold).
- A floor tile (`.`) renders for empty cells; one wall tile (`#`) blocks movement.
- Start button quits cleanly.
- Resize the desktop window — letterboxing/scaling behaves sanely. (Cheap proxy for 4:3 vs. 16:9 handhelds working later.)

### Half 2 — Miyoo cross-compile attempt

- Add `armv7-unknown-linux-musleabihf` target via `rustup target add`.
- Set up `cross` (the cross-compile helper) or a `.cargo/config.toml` with the linker pointed at a musl ARM toolchain.
- Statically link SDL2 (build SDL2 from source against musl, or use a prebuilt static lib). This is the part most likely to bite — budgeted ~2 hours.
- Package as a MinUI/Onion-compatible app (just a folder with the binary + `assets/` + a launch script).
- Drop on the device's SD card and run.

Acceptance criteria for Half 2:

- Same binary behavior on Miyoo Mini Plus as on desktop (gamepad → walk, quit works).
- If we hit toolchain pain past ~2 hours: stop, ship desktop, document blockers in README, defer to session 2.

## Files to Create

- `Cargo.toml` — deps above, `[profile.release] strip = true, lto = true, codegen-units = 1` for handheld binary size.
- `src/main.rs` — SDL2 init, gilrs init, main loop pumping events and re-rendering.
- `src/render.rs` — glyph atlas loader, `draw_glyph(x, y, glyph: u8, fg: Color, bg: Color)` primitive. This is the *only* drawing API the rest of the code uses. Tile-mode upgrade later means swapping the impl, nothing else.
- `src/input.rs` — gilrs → `Action` enum (Up/Down/Left/Right/A/B/X/Y/L/R/Start/Select to start; analog-stick and L2/R2/L3/R3 variants reserved for later). Every system in the game eventually consumes `Action`s, never raw events. The enum is designed to grow — Tier-2 wheel actions slot in without touching consumers.
- `src/world.rs` — minimal `World { player: (i32, i32), tiles: Vec<Tile> }`. Throwaway scaffolding; replaced when ECS lands.
- `assets/cp437_8x16.png` — public-domain IBM VGA font (e.g. from Dwarf Fortress tileset repos or VileR's font collection).
- `.cargo/config.toml` — cross-compile target config for Half 2.
- `README.md` — build instructions for desktop and Miyoo, current status.

## Verification

Desktop:

```bash
cargo run --release
# expect: window opens, @ visible, dpad on connected gamepad moves @, # blocks, Start quits
```

Miyoo Mini Plus:

```bash
cargo build --release --target armv7-unknown-linux-musleabihf
# copy target/armv7-unknown-linux-musleabihf/release/holyland + assets/ to SD card App folder
# launch from device menu, verify same behavior
```

If desktop passes and Miyoo build fails, that is acceptable session-1 outcome — captured as a blocker in README, not a failure.

## Session 2 Preview — Android (Retroid Pocket 5)

Not part of this plan, but called out so the architecture stays honest:

- Add `aarch64-linux-android` target via `rustup target add`.
- Use `cargo-ndk` for the build, `cargo-apk` (or a hand-rolled gradle wrapper) for packaging.
- Implement `src/platform/android.rs`: SDL2's `SDL_AndroidGetActivity` for asset manager access, lifecycle hook handling.
- Bundle assets via Android's APK asset path, not raw filesystem.
- Test on Retroid Pocket 5; if it works there, the cohort (Odin 2, AYN Odyssey, RP4 Pro) is essentially free.
- Estimated 1–2 sessions of build/packaging work. Application code should not change.

## Out of Scope (Explicit)

The following are deliberately **not** in session 1 to keep scope honest:

- Android build (Session 2).
- ECS (no entities besides player).
- Save/load (decided in "Save Strategy" section above; implementation Session 3).
- Any game systems (combat, harvesting, NPCs, ruins, blessings, demons).
- Tile-mode sprites (CP437 only).
- Menus/UI beyond quitting.
- Audio (decided in "Audio Strategy" section above; implementation Session 5–6).
- Procedural generation.
- Anything related to the oasis or wilderness loops.

These come in subsequent sessions, in roughly this order: Android build (S2) → ECS + portable CBOR saves with schema versioning (S3) → tiny oasis tilemap → first NPC + dialogue → audio (cpal + kira + tracker engine, dynamic stems wired to game state) → harvest/inventory → first wilderness tile → first demon → first deity blessing → run/death/respawn loop → analog-stick action wheels for Retroid-class devices → cloud-sync integration (Steam Cloud / Google Play Saves / manual export-import) → (much later) MIDI+SF2 or custom synth backend swap if pursuing PS1-grade or signature sonic identity. That's the rough path to vertical-slice; not part of this plan.
