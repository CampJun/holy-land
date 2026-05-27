# Holy Land

CP437 SDL2 survival roguelike. Cornwall and Devon ~1300, Arthurian-flavored, period-accurate weapons/armor/wrestling. Single binary, gamepad-first, runs on x86_64 desktop and the Miyoo Mini Plus handheld (Onion).

## Status

The combat vertical slice and the authored-city pipeline both shipped. What's live in `main`:

- **World** — Lazy 3×3 chunk ring, follow-cam, value-noise procgen with biome dispatch over the full Cornwall peninsula (~3,275 × 1,750 chunks @ 5 ft/cell). Authored data layer for outline, 8 biomes, 6 rivers, 6 roads, 27 named sites.
- **Overmap + fast-travel** — Full-screen Cornwall map (`M` on desktop), fog of war, chunk-graph A* + lazy per-cell A*, biome/site glyphs, travel banner, resume-from-interrupt.
- **Exeter** — First authored city: walls, gates, streets, blocks, landmarks (cathedral, castle, market, guildhall) loaded from `assets/cities/exeter.ron`.
- **Time + needs** — Gregorian calendar (start 21 Mar 1300, Sarum liturgical), day/night with dusk/dawn tint, four-need decay system, death gate, sleep verb.
- **Tiles + flora** — Grass / Dirt / Sand / Stream / Pond / Trees / Walls. 5 tree species (Oak/Hazel/Holly/Ash/Rowan) + Beech in BeechCombe, 6 decorations (Fern/Moss/Bramble/Bracken/Gorse/Sapling/Mushroom) with seasonal palettes and shade/moisture-banded placement. Chop → sapling regrowth loop.
- **Skills + crafting** — URW-style 0–100 skills with daily 20 XP cap (FireMaking, Foraging, Fishing, Melee, Ranged, Dodge). Fire making, fire-as-light-source, feeding fires, fishing, sleep, cooking (CDDA-style recipe registry, fish + herbs + seasonings).
- **Combat** — Vertical slice through phase 9: bandit enemy with Yeoman/Sergeant/Knight loadout rolls, six-body-part HP with crippling, layered armor with coverage rolls, weapon profiles + proficiencies, reach-2 polearms with no-reach penalty, bow + arrows + targeting cursor, stamina + grapple/throw/disarm, top-level Melee / Ranged / Dodge skills with XP.
- **Saves** — CBOR via `ciborium`, atomic write, schema v3 with v1/v2 → v3 migration chain. Additive fields use `#[serde(default)]` for forward-compat.

Canonical docs live in-repo:

- `AGENTS.md` — agent handoff, render-pipeline rationale, mmiyoo SDL2 quirks, deploy workflow.
- `holyland-PLAN.md` — full design plan.
- `holyland-ROADMAP.md` — session-by-session forward plan.
- `obsidian/Survival.md` — kanban board (Drafts / Implemented / Playtest / Completed). What's queued next.
- `obsidian/Cornwall-World.md` — peninsula scale, biome polygons, named sites.

## Build

Desktop bundles+statically links SDL2 — no system `SDL2-devel` needed. First build is slow.

```
cargo run --release            # desktop dev loop
cargo build                    # desktop sanity check
cargo test --release           # 210+ tests (combat, save, body parts, loadouts, …)
```

Requires Rust, a C compiler, `cmake`, `make`. For the Miyoo cross-build, also `podman`.

### Cross-build (Miyoo Mini Plus / Onion)

```
./cross/build-onion.sh         # builds inside the miyoomini-rust container
./cross/deploy-onion.sh <ip>   # FTP deploys, verifies remote byte size
```

The container drops `gilrs` (no libudev-sys on ARM) and links against Onion's libs under `cross/onion-libs/`. Logs on device: `App/HolyLand/holyland.log`.

## Controls

Desktop:

| Action          | Keyboard         |
|-----------------|------------------|
| Move            | Arrow keys       |
| A / B / X / Y   | A / S / D / F    |
| L / L2 / R / R2 | Z / X / C / V    |
| Start (quit)    | Esc              |
| Select (save)   | Left/Right Shift |
| Overmap         | M                |

Gamepad on desktop via `gilrs` (Xbox-layout: South=A, East=B, West=X, North=Y).

Miyoo: kernel maps Space=A, LCtrl=B, LShift=X, LAlt=Y, Tab=L, Backspace=R, Enter=Start, RCtrl=Select. Overmap chord (Select+R) deferred.

In-game: tap-Y opens the vertical command menu; hold-Y opens a 4-direction radial for quick verbs (Pickup / Eat / PickHerb / Drink). Select opens the info hub (Inventory / Crafting / Skills tabs).

## Architecture (one screen)

```
src/
├── main.rs            SDL init, event loop, render orchestration, follow-cam, save/load wiring
├── render.rs          load_atlas + draw_glyph (surface→surface, the only drawing API)
├── input.rs           Action enum + keymap + gilrs + repeat-on-hold
├── world.rs           hecs ECS, World, Tile/Position/Renderable/Player, chunk ring, time/needs/FOV
├── chunkgen.rs        value-noise procgen + biome dispatch
├── city.rs            authored-city loader (RON → cells)
├── cornwall.rs        peninsula outline + biome polygons + rivers + roads + named sites
├── fasttravel.rs      chunk-A* + lazy per-cell A*
├── combat.rs          WeaponProfile, ArmorDr, resolve_hit, roll_damage (pure)
├── action.rs          ActionId, verbs, multi-turn queue
├── flora.rs           TreeSpecies, Decoration, PlantState, seasonal palettes
├── calendar.rs        365-day Gregorian + Sarum liturgical
├── crafting.rs        recipe registry + cookware ticks
├── items.rs           ItemKind, ItemInstance, Pack, ItemMetadata
├── needs.rs           four-need decay + env wiring
├── fov.rs             recursive shadowcasting
├── save.rs            SaveHeader + MetaSave + RunSave + schema migration chain
├── skill.rs           URW skills + daily XP cap + dawn reset
└── platform/          OS-specific code (XDG paths, etc.)
```

## Render pipeline

CPU-composited surface diff. Each frame: walk the viewport (40×30 cells), compose `Cell { glyph, fg, bg }`, diff against `prev_cells`, blit changed cells into a single ARGB8888 surface, upload as one texture, present. **Don't refactor it back to per-glyph `canvas.copy`** — mmiyoo SDL2 no-ops `SetTextureColorMod` / `AlphaMod` / `BlendMode` / `SetVSync` and drops most `QueueCopy` work. See `AGENTS.md` for the full reason.

## What's not in yet

Most-wanted drafts in `obsidian/Survival.md` Drafts column: drop-item verb, wait/pass-turn, menu wrap-around, STR/AGI/CON/INT/SPIRIT attributes, carry overload + stride modes (Creep/Walk/Jog). Also deferred: NPCs and dialogue, sea/boat fast-travel, plant lifecycle scheduler, fire-spread, audio, additional authored cities beyond Exeter, Android build (`platform/android.rs` is a stub).

## Saves

Two files per `save_dir`:

- `meta.cbor` — xp, currency, affinity, unlocks, settings (always-sync tier).
- `run.cbor` — current run state (player, world clock, chunk mutations, hostiles, equipment, body parts).

Every save starts with `SaveHeader { schema_version, build_version, save_counter, device_id, timestamp }`. v1 saves are friendly-rejected ("This save belongs to the Holy Land design. Start a new game…"). v2 → v3 is additive-only (`#[serde(default)]`).

`save_dir` per platform: `$XDG_DATA_HOME/holyland` → `$HOME/.local/share/holyland` → exe-adjacent `saves/` on Linux. Android stub today.
