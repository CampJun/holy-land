Wire the audio stack the planning doc already committed to: `cpal` for output, `kira` as the mixer/dynamics layer, and a tracker engine (libxmp Rust binding or pure-Rust tracker) as the music backend.

Library choices are locked in `holyland-PLAN.md` § Audio Strategy — the platform abstraction is supposed to land before content sessions paint us into a corner. Implementation timing per the plan is around Session 5–6.

Key architectural pieces from the plan:

```rust
trait MusicEngine {
    fn play_song(&mut self, song: SongHandle);
    fn set_layer_volume(&mut self, layer: LayerId, vol: f32);
    fn set_intensity_param(&mut self, param: ParamId, value: f32);
    fn stop(&mut self);
}
```

Tracker backend implements this first; a MIDI+SoundFont (oxisynth) or custom `fundsp` synth backend implements the same trait later. Game code couples to the trait, never the backend.

CPU budget honesty for Miyoo (Cortex-A7 @ 1.2GHz, dual-core): audio gets 5–10% of one core. Tracker mixing fits comfortably; MIDI+SF2 fits with voice discipline; pure synthesis only voice-limited (≤8) without expensive filters.

Source: `holyland-PLAN.md` § Audio Strategy.
