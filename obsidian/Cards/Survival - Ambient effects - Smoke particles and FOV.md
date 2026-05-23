Smoke as a transient particle list emitted from each lit fire. Drifts along the wind vector from [[Survival - Ambient effects - Weather state and wind vector]]. Each occupied cell adds a small visibility penalty to FOV — sim-coupled, not purely cosmetic.

Depends on the weather card landing first (smoke needs a wind vector to consume). Brainstormed in [[Survival - Ambient effects - Brainstorm]].

## Particle list

```rust
// src/effects/smoke.rs
pub struct SmokeParticle {
    pub wx: f32, // sub-cell precision so slow drift is smooth
    pub wy: f32,
    pub age_frames: u32,
    pub lifetime_frames: u32, // ~120-180 frames at 60fps = 2-3 sec
    pub source_fire_id: u32,  // for spawn-rate throttling per fire
}

pub struct SmokeField {
    pub particles: Vec<SmokeParticle>, // transient, never saved
}
```

**Not** saved (per brainstorm decision — pure render state). On load, `SmokeField` starts empty and re-fills naturally as fires emit. The FOV penalty is recomputed every frame from live particles, so the world is internally consistent even with an empty list.

## Per-fire spawn

Each lit fire spawns one particle every `SPAWN_INTERVAL_FRAMES` (~15 at 60fps = 4 Hz emission). Spawn position is the fire's cell + a small jitter; initial age = 0.

## Per-frame tick

```rust
pub fn tick_smoke(field: &mut SmokeField, weather: &WeatherState, dt_frames: f32) {
    let (dx, dy) = weather.wind_dir.unit_vec(); // f32 per octant
    let speed = (weather.wind_intensity as f32 / 255.0) * MAX_DRIFT_PER_FRAME;
    field.particles.retain_mut(|p| {
        p.wx += dx * speed * dt_frames;
        p.wy += dy * speed * dt_frames;
        // Add a constant upward drift independent of wind (hot air rises):
        p.wy -= UPWARD_DRIFT_PER_FRAME * dt_frames;
        p.age_frames += dt_frames as u32;
        p.age_frames < p.lifetime_frames
    });
}
```

`MAX_DRIFT_PER_FRAME` ≈ 0.04 cells/frame at Storm. Calm = 0 (only upward drift). `UPWARD_DRIFT_PER_FRAME` ≈ 0.02.

## Rendering

In the per-cell compose loop, for each particle whose integer `(wx, wy)` falls in the viewport:

- Glyph: pick from `[',', '°', '\'']` by age band (young → older → fading). Could promote to a `GlyphCycle` later from [[Survival - Ambient effects - Glyph cycle framework and demo]].
- Color: grey-to-transparent fade by age. `alpha = 255 - (age * 255 / lifetime)`. Surface blit handles the alpha via `SDL_SetSurfaceAlphaMod` (software path, works on Miyoo).
- The smoke cell becomes "dirty" for the diff. With ~3 fires × ~8 live particles each = ~24 dirty cells/frame steady-state. Comfortably in budget.

## FOV coupling

The brainstorm decided **single-cell visibility penalty**, fixed amount, no stacking. Implementation:

1. Before FOV recompute, build `smoke_penalty: HashMap<(i32, i32), u8>` (or a small `Vec<((i32,i32), u8)>`) listing cells with ≥1 particle, mapping to a constant `SMOKE_VISIBILITY_PENALTY` (e.g. 60 out of 255).
2. In `recompute_fov` (`world.rs:948`), when computing `light_intensity` for a cell, subtract the smoke penalty after light contribution. Saturating sub at 0.
3. Rays passing through smoke cells are not blocked — the penalty only dims the smoky cell itself. (Brainstorm: "fixed amount per cell, one particle = one penalty — no stacking.")

This is the cheapest sim hookup that still gives smoke tactical weight. Reference: brainstorm "single-cell visibility penalty" option.

## Save consistency note

Smoke not saved → FOV penalty for the load-frame is computed from an empty smoke field → first frame after load has slightly brighter cells near fires. Within ~30 frames the field re-fills and FOV settles. Acceptable; not visible as a flash because the brighter cells correctly resolve to "lit by fire" and then dim as smoke arrives.

## Files touched

- `src/effects/smoke.rs` (new) — `SmokeParticle`, `SmokeField`, `tick_smoke`, `spawn_for_fires`.
- `src/effects/weather.rs` — add `Dir8::unit_vec(self) -> (f32, f32)` if not already in the weather card.
- `src/world.rs` — `World` owns a `smoke: SmokeField`. `recompute_fov` consumes `&smoke` to compute the per-cell penalty.
- `src/main.rs` — call `tick_smoke` and `spawn_for_fires` once per render frame; render smoke particles in the per-cell compose loop alongside terrain/items.

## Cost

- Spawn: 1 push per fire per ~15 frames.
- Tick: O(N_particles) per frame; cap at ~50 particles total via an LRU eviction.
- Render: ~24 dirty cells/frame steady-state for 3 fires.
- FOV penalty: O(N_particles) hash inserts + O(visible cells) lookups during shadowcast. Probably <100 ops/frame.

## Verification

- Desktop: light a fire, set weather kind=Gusty (debug hotkey), confirm smoke drifts in the wind direction.
- Confirm FOV is visibly dimmer through a smoke plume (stand behind the smoke at night).
- Save mid-emission, quit, reload — confirm no panic, fires resume emission cleanly.
- Frame-time: log `len(particles)` and `dirty_cells_this_frame` once/sec; particles should stabilize <50, dirty cells <50 even with 5 fires going.
- Miyoo: deploy, confirm smoke is readable on the LCD and that FOV penalty doesn't introduce flicker artifacts.

## Open

- Whether smoke should spawn for *all* `ItemMetadata::Lit` carriers or just fires specifically (future candles probably shouldn't smoke).
- Whether to expose smoke spawn rate as a per-fire field (small candle = trickle, big bonfire = column).
- Whether `SMOKE_VISIBILITY_PENALTY` should scale by particle age (fresh smoke dims more than dissipating smoke).

References: [[Survival - Fire Making]], [[Survival - FOV]], [[Survival - Ambient effects - Brainstorm]], [[Survival - Ambient effects - Weather state and wind vector]].
