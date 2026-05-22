Global weather state machine that owns wind direction + intensity. First-class world property; all ambient-effect consumers read from one source of truth. Save-persistent.

## State machine

```rust
// src/effects/weather.rs
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherKind {
    Calm,
    Breezy,
    Gusty,
    Storm,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct WeatherState {
    pub kind: WeatherKind,
    pub wind_dir: Dir8, // N, NE, E, SE, S, SW, W, NW
    pub wind_intensity: u8, // 0..=255, derived from kind but smoothable over transitions
    pub since_game_seconds: u32, // when current kind began
}
```

Initial implementation uses fixed kind→intensity mapping (Calm 0, Breezy 64, Gusty 160, Storm 240). The `wind_intensity: u8` field is kept separate so a future transition smoother can ramp between kinds without snapping.

## Transition rules (slice 1)

Per in-game minute, roll for a transition with kind-dependent probabilities. Numbers are tunable; the structure is what matters.

| From → | Calm | Breezy | Gusty | Storm |
|---|---|---|---|---|
| Calm   | 0.95 | 0.05 | 0    | 0    |
| Breezy | 0.10 | 0.80 | 0.10 | 0    |
| Gusty  | 0    | 0.15 | 0.80 | 0.05 |
| Storm  | 0    | 0    | 0.30 | 0.70 |

Direction shifts independently: per kind, % chance per minute of rotating one octant CW or CCW. Calm rarely changes; Storm shifts quickly.

Rolls use `world.rng` (xorshift32, already save-persistent — see [[Survival - Skill system URW]]). Determinism intact.

## Save schema impact

- Bump `SCHEMA_VERSION` (see `src/save.rs` header comment).
- Add `weather: WeatherState` field to `RunSave` with `#[serde(default)]` so v_prev saves migrate to Calm + N wind + intensity 0.
- Implement `migrate_vN_to_vNplus1` that fills the default and chains into `migrate_header`.
- Round-trip test: `cargo test --release save::` should grow a `weather_round_trip` test.

The state machine ticks once per game-minute (or once per `clock_seconds % 60 == 0`) from `World::tick_clock` or its caller.

## Consumers

Read-only consumers, in order of planned arrival:

1. **[[Survival - Ambient effects - Smoke particles and FOV]]** — smoke particles drift along `wind_dir`, displacement speed scaled by `wind_intensity`.
2. **Future fire-spread** — burning grass/tree probability biased downwind. Out of scope here.
3. **Future drifting leaves / debris (W3 in brainstorm)** — particle spawner reads wind vector.
4. **Future rain visuals** — diagonal `/` direction biased by wind during Storm.
5. **Future HUD wind indicator (W4)** — tiny corner glyph showing `wind_dir`.

Grass sway is **not** a consumer (per brainstorm decision — sway is independent ambient).

## Files touched

- `src/effects/weather.rs` (new) — `WeatherKind`, `WeatherState`, `Dir8`, `tick_weather(&mut WeatherState, &mut Rng, dt_seconds)`.
- `src/world.rs` — add `pub weather: WeatherState` to `World`, hook into the clock-tick path.
- `src/save.rs` — schema bump, migration, round-trip test.
- `src/main.rs` — no behavior change initially; the field is just present for consumers.

## Cost

Compute: one RNG roll per game-minute plus an integer field read by consumers. Effectively free.

## Verification

- `cargo test --release save::weather_round_trip`.
- `cargo run --release`, idle the game for ~20 in-game minutes (use the time-skip toggle), confirm `weather.kind` actually transitions in a debug log.
- Load a v_prev save (if one was kept) and confirm it migrates to Calm with no panic.

## Open

- Whether transitions should be driven by `world.rng` or a dedicated `weather_rng` so save-scumming weather doesn't perturb other systems.
- Whether `Dir8` should live in `effects::weather` or be promoted to `world.rs` for general use (FOV octants, future pathfinding).

References: [[Survival - Save schema migration testing]], [[Survival - Day night cycle]], [[Survival - Ambient effects - Brainstorm]], [[Survival - Ambient effects - Smoke particles and FOV]].
