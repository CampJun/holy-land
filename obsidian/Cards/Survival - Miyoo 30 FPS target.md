Target 30 FPS on the Miyoo, keep desktop at 60. Battery + pacer consistency win; input lag still imperceptible for the genre.

## Why

- Miyoo Linux kernel oversleeps `std::thread::sleep` by several ms (coarse HZ). At a 16.67 ms target the slop is ~30% of the budget; at 33 ms it's ~10%. So 30 FPS will feel MORE consistent than today's 45–50 FPS.
- 60 Hz panel divides evenly at 30 FPS (no judder).
- Cortex-A7 cores idle ~85% today (sleep ≈ 17–20 ms / 22 ms frame); halving frame rate roughly doubles the idle fraction → real battery savings.
- Turn-based survival, one-cell-per-step scrolling. Visual difference vs. 60 FPS is imperceptible.
- Worst-case input lag goes 16.67 ms → 33 ms; both well under the ~100 ms perceptual threshold.

## Evidence (from follow-cam ship)

`holyland.log` during exploration after the follow-cam slice:

```
fps: 45  timing avg_ms draw=0.24 upload=4.55 ... sleep=17.59  changed_cells=0.3
fps: 51  timing avg_ms draw=0.27 upload=0.00 ... sleep=19.68  changed_cells=0.0
```

Sleep averages exceed `TARGET_FRAME = 16_667 µs`. Investigation showed the pacer is the bottleneck, not draw/upload — mmiyoo `SDL_UpdateTexture` has fixed per-call cost (~3–10 ms) regardless of rect size but compose work is tiny (0.24 ms). The follow-cam scroll path is in budget.

## Change

`src/main.rs:43`. Cfg-split `TARGET_FRAME` the same way `SLEEP_GUARD` is already split (`main.rs:44-47`):

```rust
#[cfg(target_arch = "arm")]
const TARGET_FRAME: Duration = Duration::from_micros(33_333);
#[cfg(not(target_arch = "arm"))]
const TARGET_FRAME: Duration = Duration::from_micros(16_667);
```

## Multi-turn pacing caveat

`MULTI_TURN_GAME_SEC_PER_FRAME = 1` in `world.rs:55` advances 1 game-second per render frame. At 60 FPS a 300-sec PitchTent finishes in ~5 real seconds; at 30 FPS it'd take ~10. Two options:

- Leave it: doubles all multi-turn wall-clock durations. Probably fine — players can still time-skip via Select.
- Bump to `MULTI_TURN_GAME_SEC_PER_FRAME = 2` on ARM via cfg so the felt duration is preserved.

Pick when implementing.

## Verify

- `cargo run --release` on desktop: timing log shows ~60 FPS.
- Cross-build + deploy to Miyoo: log shows ~30 FPS with sleep avg consistently below `TARGET_FRAME` (the oversleep slop fits in the larger budget).
- Walk + scroll: no visible judder; multi-turn banner durations feel acceptable (or adjust per the caveat above).

Reference: `[[Survival - Multi-turn action queue]]` (the pacing constant referenced above).
