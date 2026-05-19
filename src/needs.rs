// Four-meter needs system: Thirst, Hunger, Sleep, Warmth.
//
// Phase 4 ships the data, decay, HUD wiring, and per-action time cost. Death
// at 0 is intentionally gated off via `DEATH_ENABLED` so phase-4 builds let
// the player learn the loop without punishment. Phase 14 flips that constant.
//
// Decay accuracy: each need carries its own `_acc_secs` accumulator that
// holds the sub-point fractional progress between whole-point decrements.
// Without this, small per-action ticks (e.g. a 5-second move feeding hunger
// at 1-per-180-seconds) would truncate to zero loss every time, and decay
// would silently never advance. The accumulators round-trip through saves
// so save/load preserves sub-unit precision.
//
// See Survival - Game time clock and needs.md and Survival - Day night
// cycle.md for the full mechanics spec.

use serde::{Deserialize, Serialize};

pub const NEED_MAX: u8 = 100;

/// Whether `is_dead()` returns true when any need hits 0. Flipped in
/// phase 14 (death gate); main.rs reads `is_dead()` per frame and
/// shows the death overlay.
pub const DEATH_ENABLED: bool = true;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needs {
    pub thirst: u8,
    pub hunger: u8,
    pub sleep: u8,
    pub warmth: u8,
    /// Fractional accumulator for thirst loss; `value -= acc / 60` per tick,
    /// then `acc %= 60`. Same idea for the others with their own denominators.
    pub thirst_acc_secs: u32,
    pub hunger_acc_secs: u32,
    pub sleep_acc_secs: u32,
    pub warmth_acc_secs: u32,
}

impl Default for Needs {
    fn default() -> Self {
        Self::starting()
    }
}

impl Needs {
    /// Slice-1 starting values (per master plan + Survival - Game time clock
    /// and needs.md). Warmth starts at 100 because day decay is 0.
    pub fn starting() -> Self {
        Self {
            thirst: 75,
            hunger: 75,
            sleep: 75,
            warmth: 100,
            thirst_acc_secs: 0,
            hunger_acc_secs: 0,
            sleep_acc_secs: 0,
            warmth_acc_secs: 0,
        }
    }

    pub fn is_dead(&self) -> bool {
        DEATH_ENABLED
            && (self.thirst == 0 || self.hunger == 0 || self.sleep == 0 || self.warmth == 0)
    }

    /// Returns the additive action-cost modifier as a percentage (e.g. 20 =
    /// +20%). Worst-need wins; penalties don't multiplicatively compound.
    pub fn action_cost_penalty_pct(&self) -> u32 {
        let worst = self
            .thirst
            .min(self.hunger)
            .min(self.sleep)
            .min(self.warmth);
        if worst < 10 {
            50
        } else if worst < 25 {
            20
        } else {
            0
        }
    }

    /// Apply a game-time delta. Per-need accumulators carry sub-point
    /// progress so a sequence of small ticks decays the same as one big one.
    pub fn tick(&mut self, elapsed_game_seconds: u32, env: NeedsEnv) {
        if elapsed_game_seconds == 0 {
            return;
        }

        // Thirst: 1 point per 60 game-seconds, always.
        accumulate_loss(
            &mut self.thirst,
            &mut self.thirst_acc_secs,
            elapsed_game_seconds,
            60,
        );

        // Hunger: 1 point per 180 game-seconds (1/3 min).
        accumulate_loss(
            &mut self.hunger,
            &mut self.hunger_acc_secs,
            elapsed_game_seconds,
            180,
        );

        // Sleep: 1 per 300 (day) or 1 per 180 (night unless in bedroll).
        let sleep_denom = if env.is_night && !env.in_bedroll {
            180
        } else {
            300
        };
        accumulate_loss(
            &mut self.sleep,
            &mut self.sleep_acc_secs,
            elapsed_game_seconds,
            sleep_denom,
        );

        // Warmth: 0/min during the day; at night, 3/min unsheltered minus
        // 1/min for each adjacent fire and 1/min for each bedroll the player
        // is in. Phase 4 never reaches positive net (tent gives +0.5/min,
        // not enough alone), so we only handle loss.
        let warmth_loss_per_min: u32 = if env.is_night {
            let mut n: u32 = 3;
            if env.adjacent_fire {
                n = n.saturating_sub(1);
            }
            if env.in_bedroll {
                n = n.saturating_sub(1);
            }
            // Tent contributes +0.5/min in the full spec. Approximate it here
            // as the difference between "we'd still lose 1/min" and "we hold
            // steady" — if we're already at 1/min loss, the tent zeros it.
            // The 0.5/min bookkeeping arrives properly in phase 13 when
            // weather lands and warmth needs better resolution.
            if env.inside_tent && n == 1 {
                n = 0;
            }
            n
        } else {
            0
        };
        if warmth_loss_per_min > 0 {
            // Loss-per-min × seconds gives "loss-seconds"; each 60 of them
            // is 1 warmth point.
            let units = elapsed_game_seconds.saturating_mul(warmth_loss_per_min);
            accumulate_loss(&mut self.warmth, &mut self.warmth_acc_secs, units, 60);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NeedsEnv {
    pub is_night: bool,
    pub adjacent_fire: bool,
    pub inside_tent: bool,
    pub in_bedroll: bool,
}

/// Selector for need-restoration verbs (eat/drink/sleep/...). Shared
/// across modules so action.rs, debug_console.rs, and future verb code
/// don't redefine it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeedKind {
    Thirst,
    Hunger,
    Sleep,
    Warmth,
}

impl Needs {
    /// Single source of truth for "address a need by selector"; every
    /// mutator method below goes through this. Returns the two mutable
    /// references (value, accumulator) so callers can update both
    /// without re-matching on the kind.
    fn mut_pair(&mut self, kind: NeedKind) -> (&mut u8, &mut u32) {
        match kind {
            NeedKind::Thirst => (&mut self.thirst, &mut self.thirst_acc_secs),
            NeedKind::Hunger => (&mut self.hunger, &mut self.hunger_acc_secs),
            NeedKind::Sleep => (&mut self.sleep, &mut self.sleep_acc_secs),
            NeedKind::Warmth => (&mut self.warmth, &mut self.warmth_acc_secs),
        }
    }

    /// Add `amount` points to a need, clamped at NEED_MAX, and reset the
    /// matching sub-point accumulator so the next decay tick starts fresh
    /// from the new value (otherwise stale fractional progress could
    /// immediately knock the need back down).
    pub fn restore(&mut self, kind: NeedKind, amount: u8) {
        let (value, acc) = self.mut_pair(kind);
        *value = (*value as u16)
            .saturating_add(amount as u16)
            .min(NEED_MAX as u16) as u8;
        *acc = 0;
    }

    /// Set a need to a specific value (clamped at NEED_MAX) and reset its
    /// accumulator. Used by the debug console; gameplay restorers prefer
    /// `restore` so they accumulate from the current value.
    pub fn set(&mut self, kind: NeedKind, value: u8) {
        let (v, acc) = self.mut_pair(kind);
        *v = value.min(NEED_MAX);
        *acc = 0;
    }

    /// Zero the sub-point accumulator for `kind` without touching the
    /// meter. Verbs that mutate a need by other means call this so the
    /// next decay tick starts fresh.
    #[allow(dead_code)] // wired in once a verb needs it independently
    pub fn reset_accumulator(&mut self, kind: NeedKind) {
        *self.mut_pair(kind).1 = 0;
    }
}

/// Add `add_secs` to `acc`; for each whole `denom_secs` accumulated, lose 1
/// point and roll the accumulator over. Saturates at 0.
fn accumulate_loss(value: &mut u8, acc: &mut u32, add_secs: u32, denom_secs: u32) {
    *acc = acc.saturating_add(add_secs);
    while *acc >= denom_secs {
        *value = value.saturating_sub(1);
        *acc -= denom_secs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_needs_match_spec() {
        let n = Needs::starting();
        assert_eq!(n.thirst, 75);
        assert_eq!(n.hunger, 75);
        assert_eq!(n.sleep, 75);
        assert_eq!(n.warmth, 100);
    }

    #[test]
    fn thirst_decays_one_per_game_minute() {
        let mut n = Needs::starting();
        n.tick(60, NeedsEnv::default());
        assert_eq!(n.thirst, 74);
    }

    #[test]
    fn hunger_accumulates_across_small_ticks() {
        let mut n = Needs::starting();
        // 60s + 120s = 180s = exactly 1 hunger lost. Each call alone
        // would truncate to 0 without the accumulator.
        n.tick(60, NeedsEnv::default());
        assert_eq!(n.hunger, 75);
        n.tick(120, NeedsEnv::default());
        assert_eq!(n.hunger, 74);
    }

    #[test]
    fn many_small_ticks_match_one_big_tick() {
        let mut a = Needs::starting();
        let mut b = Needs::starting();
        // Drip-feed 60 × 1-second ticks vs one 60-second tick: thirst loss
        // must match to the unit.
        for _ in 0..60 {
            a.tick(1, NeedsEnv::default());
        }
        b.tick(60, NeedsEnv::default());
        assert_eq!(a.thirst, b.thirst);
    }

    #[test]
    fn warmth_holds_during_day() {
        let mut n = Needs::starting();
        n.tick(600, NeedsEnv::default()); // 10 game-min of daytime
        assert_eq!(n.warmth, 100);
    }

    #[test]
    fn warmth_decays_three_per_minute_at_night_unsheltered() {
        let mut n = Needs::starting();
        n.tick(
            60,
            NeedsEnv {
                is_night: true,
                ..Default::default()
            },
        );
        assert_eq!(n.warmth, 97);
    }

    #[test]
    fn fire_adjacency_slows_warmth_loss() {
        let mut n = Needs::starting();
        n.tick(
            60,
            NeedsEnv {
                is_night: true,
                adjacent_fire: true,
                ..Default::default()
            },
        );
        // Net rate at night with fire: -3 + 1 = -2 per min.
        assert_eq!(n.warmth, 98);
    }

    #[test]
    fn fire_and_bedroll_combined_softens_loss() {
        let mut n = Needs {
            warmth: 80,
            ..Needs::starting()
        };
        n.tick(
            60,
            NeedsEnv {
                is_night: true,
                adjacent_fire: true,
                in_bedroll: true,
                ..Default::default()
            },
        );
        // Net rate: -3 + 1 (fire) + 1 (bedroll) = -1 per min.
        assert_eq!(n.warmth, 79);
    }

    #[test]
    fn need_clamps_at_zero_and_triggers_death() {
        let mut n = Needs {
            thirst: 5,
            ..Needs::starting()
        };
        n.tick(3600, NeedsEnv::default()); // an hour at 1/min would lose 60
        assert_eq!(n.thirst, 0);
        // Phase 14 flipped DEATH_ENABLED on; zeroing any need is now
        // terminal — main.rs reads is_dead per frame and shows the
        // death overlay.
        assert!(n.is_dead(), "phase 14: any need at 0 is fatal");
    }

    #[test]
    fn restore_clamps_at_need_max() {
        let mut n = Needs {
            thirst: 90,
            ..Needs::starting()
        };
        n.restore(NeedKind::Thirst, 30);
        assert_eq!(n.thirst, NEED_MAX);
    }

    #[test]
    fn restore_resets_matching_accumulator() {
        let mut n = Needs::starting();
        n.tick(45, NeedsEnv::default()); // thirst_acc_secs == 45
        assert_eq!(n.thirst_acc_secs, 45);
        n.restore(NeedKind::Thirst, 5);
        assert_eq!(n.thirst_acc_secs, 0);
        // Hunger accumulator is independent — should still be 45 (from 45s
        // of tick) since hunger denom is 180.
        assert_eq!(n.hunger_acc_secs, 45);
    }

    #[test]
    fn action_cost_penalty_kicks_in_at_25_and_10() {
        let mut n = Needs::starting();
        assert_eq!(n.action_cost_penalty_pct(), 0);
        n.thirst = 24;
        assert_eq!(n.action_cost_penalty_pct(), 20);
        n.thirst = 9;
        assert_eq!(n.action_cost_penalty_pct(), 50);
    }
}
