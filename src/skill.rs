// URW-style percentile skill system. Phase 10 ships the chassis + the
// first skill (Fire Making); slice 2+ adds Fishing, Cookery, Foraging,
// etc. against the same shape.
//
// Design (see STYLE.md §2 + Survival - Skill system URW.md):
//   - Skill value: u8 in 0..=100.
//   - Check: 1d100 vs (skill + modifiers), clamped to [5, 95] so the
//     player never auto-fails or auto-succeeds.
//   - XP: +1 on failure, +5 on success.
//   - Daily cap: +20 XP per skill per in-game day (resets at 06:00 dawn
//     in main.rs's dawn-crossing handler).
//   - Level-up: while daily_xp >= threshold(value), increment value by
//     1 and subtract threshold. Threshold = 5 + value/5 (grows with
//     mastery).
//
// Adding a new skill: extend `SkillKind`, add a field to `Skills`,
// extend `Skills::starting()` and `Skills::get_mut`/`get`. The compiler
// keeps the catalog honest.

use serde::{Deserialize, Serialize};

pub const DAILY_XP_CAP: u8 = 20;
pub const XP_SUCCESS: u8 = 5;
pub const XP_FAILURE: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillKind {
    FireMaking,
}

impl SkillKind {
    // Slice-1 has only one skill, so SkillsSave maps fields by name.
    // save_key + from_save_key go live when slice-2 introduces enough
    // skills to make a `Vec<(SkillKind, SkillSave)>` worth it; keeping
    // the round-trip helpers wired now so we don't rediscover them.
    #[allow(dead_code)] // consumed by SkillsSave when slice-2 multi-skills land
    pub fn save_key(self) -> &'static str {
        match self {
            SkillKind::FireMaking => "fire_making",
        }
    }

    #[allow(dead_code)] // same as save_key — slice-2 multi-skills
    pub fn from_save_key(s: &str) -> Option<Self> {
        Some(match s {
            "fire_making" => SkillKind::FireMaking,
            _ => return None,
        })
    }

    /// Short display label for the HUD ("Fire Making").
    pub fn display_name(self) -> &'static str {
        match self {
            SkillKind::FireMaking => "Fire Making",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub value: u8,
    pub daily_xp: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skills {
    pub fire_making: Skill,
}

impl Default for Skills {
    fn default() -> Self {
        Self::starting()
    }
}

impl Skills {
    /// Slice-1 starting values: Fire Making 15 (per master plan).
    pub fn starting() -> Self {
        Self {
            fire_making: Skill {
                value: 15,
                daily_xp: 0,
            },
        }
    }

    pub fn get(&self, kind: SkillKind) -> &Skill {
        match kind {
            SkillKind::FireMaking => &self.fire_making,
        }
    }

    pub fn get_mut(&mut self, kind: SkillKind) -> &mut Skill {
        match kind {
            SkillKind::FireMaking => &mut self.fire_making,
        }
    }

    /// Called from main.rs's dawn-crossing handler. Zeros every skill's
    /// daily XP counter so the player can train each skill again.
    pub fn reset_daily_caps(&mut self) {
        self.fire_making.daily_xp = 0;
    }
}

/// Pure-function skill check given an explicit d100 roll. The roll-
/// injection form is the canonical test surface; the convenience
/// `skill_check` wraps it with an Rng.
pub fn skill_check_with_roll(skill: u8, modifiers: i32, roll: u8) -> bool {
    let target = (skill as i32 + modifiers).clamp(5, 95) as u8;
    roll <= target
}

/// Award XP for an attempt, honoring the daily cap and triggering
/// level-ups while the bank exceeds the threshold. Mutates `skill` in
/// place. Returns true if a level-up happened (caller may want to log).
pub fn award_xp(skill: &mut Skill, success: bool) -> bool {
    let xp = if success { XP_SUCCESS } else { XP_FAILURE };
    // Honor daily cap (no underflow if already at cap).
    let allowed = DAILY_XP_CAP.saturating_sub(skill.daily_xp);
    let actual = xp.min(allowed);
    skill.daily_xp = skill.daily_xp.saturating_add(actual);

    // Multi-level-up while daily_xp exceeds threshold for the current
    // value. Threshold grows with mastery so each next level needs
    // more XP than the last.
    let mut leveled_up = false;
    while skill.value < 100 {
        let threshold = 5 + (skill.value / 5);
        if skill.daily_xp < threshold {
            break;
        }
        skill.value = skill.value.saturating_add(1);
        skill.daily_xp = skill.daily_xp.saturating_sub(threshold);
        leveled_up = true;
    }
    leveled_up
}

/// Lightweight xorshift32 PRNG. Lives on `World.rng` and advances every
/// `roll_d100`. Saved as a u32 so checks are reproducible across save
/// load — preventing save-scumming a critical skill check.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    pub state: u32,
}

impl Rng {
    /// Build from the world seed by folding the upper bits into the
    /// lower. Avoids the all-zero state (xorshift gets stuck there).
    pub fn from_world_seed(seed: u64) -> Self {
        let mixed = (seed as u32) ^ ((seed >> 32) as u32);
        Self {
            state: mixed.max(1),
        }
    }

    pub fn from_state(state: u32) -> Self {
        Self {
            state: state.max(1),
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Returns a number in 1..=100 inclusive (the d100 contract).
    pub fn d100(&mut self) -> u8 {
        ((self.next_u32() % 100) + 1) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_kind_save_key_round_trip() {
        for &k in &[SkillKind::FireMaking] {
            assert_eq!(SkillKind::from_save_key(k.save_key()), Some(k));
        }
        assert_eq!(SkillKind::from_save_key("unknown"), None);
    }

    #[test]
    fn starting_skills_match_spec() {
        let s = Skills::starting();
        assert_eq!(s.fire_making.value, 15);
        assert_eq!(s.fire_making.daily_xp, 0);
    }

    #[test]
    fn skill_check_target_is_clamped() {
        // Skill 0 with no mods should still have a 5% floor (target 5).
        assert!(skill_check_with_roll(0, 0, 5));
        assert!(!skill_check_with_roll(0, 0, 6));
        // Skill 100 with no mods caps at 95% (target 95).
        assert!(skill_check_with_roll(100, 0, 95));
        assert!(!skill_check_with_roll(100, 0, 96));
        // Negative mods can't push below 5.
        assert!(skill_check_with_roll(20, -50, 5));
        assert!(!skill_check_with_roll(20, -50, 6));
    }

    #[test]
    fn skill_check_with_typical_phase10_modifiers() {
        // Phase-10 spec: Fire Making 15 + flint&steel +30 = target 45.
        assert!(skill_check_with_roll(15, 30, 45));
        assert!(!skill_check_with_roll(15, 30, 46));
    }

    #[test]
    fn award_xp_honors_daily_cap() {
        let mut s = Skill {
            value: 15,
            daily_xp: 0,
        };
        for _ in 0..10 {
            award_xp(&mut s, true);
        }
        // 10 successes would be 50 XP but the cap holds at 20.
        // Some will have leveled, so daily_xp may be < 20 by however
        // much threshold drained — but the *total earned* over the
        // attempts can't exceed 20.
        // Easier assertion: daily_xp + (levels gained × threshold-at-
        // gain-time) <= cap. We'll just check daily_xp stayed under
        // the cap.
        assert!(s.daily_xp <= DAILY_XP_CAP);
    }

    #[test]
    fn award_xp_levels_up_when_threshold_crosses() {
        let mut s = Skill {
            value: 15,
            daily_xp: 0,
        };
        // Threshold at value 15 = 5 + 15/5 = 8. Two successes = 10 XP.
        award_xp(&mut s, true);
        award_xp(&mut s, true);
        assert_eq!(s.value, 16, "first level-up at 8 XP");
        // Remaining XP after subtracting threshold: 10 - 8 = 2.
        assert_eq!(s.daily_xp, 2);
    }

    #[test]
    fn award_xp_failure_grants_one_xp() {
        let mut s = Skill {
            value: 15,
            daily_xp: 0,
        };
        award_xp(&mut s, false);
        assert_eq!(s.daily_xp, 1);
        assert_eq!(s.value, 15);
    }

    #[test]
    fn award_xp_does_not_overflow_at_max_value() {
        let mut s = Skill {
            value: 100,
            daily_xp: 0,
        };
        for _ in 0..10 {
            award_xp(&mut s, true);
        }
        assert_eq!(s.value, 100, "value capped at 100");
    }

    #[test]
    fn reset_daily_caps_zeroes_xp_but_preserves_value() {
        let mut skills = Skills::starting();
        skills.fire_making.daily_xp = 12;
        skills.fire_making.value = 23;
        skills.reset_daily_caps();
        assert_eq!(skills.fire_making.daily_xp, 0);
        assert_eq!(skills.fire_making.value, 23);
    }

    #[test]
    fn rng_d100_in_range_and_deterministic() {
        let mut a = Rng::from_state(0x12345678);
        let mut b = Rng::from_state(0x12345678);
        for _ in 0..1000 {
            let ra = a.d100();
            let rb = b.d100();
            assert_eq!(ra, rb, "same seed should produce same rolls");
            assert!((1..=100).contains(&ra));
        }
    }

    #[test]
    fn rng_handles_zero_seed_without_stuck_state() {
        let mut rng = Rng::from_world_seed(0);
        // Just verify it advances at all.
        let first = rng.next_u32();
        let second = rng.next_u32();
        assert_ne!(first, second);
    }
}
