URW-style percentile skill system. Foundation for Fire Making (slice 1) and the wider crafting/survival skill tree (slice 2+).

## Model
- Skills are named: `Skill { name: String, value: u8 /* 0..=100 */, daily_xp: u8 }`.
- HUD displays current value as percent: "Fire Making 23%".

## Check
`1d100 ≤ skill + tool_bonus + condition_modifier`
Caller declares the modifiers; the roller is generic.

```rust
fn skill_check(skill_value: u8, mods: &[Modifier], rng: &mut Rng) -> Outcome {
    let target = (skill_value as i32 + mods.iter().sum::<i32>()).clamp(5, 95);
    if rng.gen_range(1..=100) <= target as i32 { Success } else { Failure }
}
```
Clamping to [5, 95] mirrors URW: never auto-fail and never auto-succeed.

## XP
- Per-attempt: +1 on failure, +5 on success.
- Daily cap: +20 XP per skill, resets at 06:00 in-game dawn.
- Level-up: when `daily_xp_total >= threshold(skill_value)`, increment `value` by 1.
  Threshold grows with current skill: simple linear `threshold = 5 + value / 5` for slice 1.

## Slice-1 skills
Only **Fire Making**. Fishing is skill-less in slice 1 (flat 20% per attempt). Slice 2 adds Fishing, Cookery, Foraging.

## Module
`src/skill.rs` — skill registry, `skill_check`, XP gain, daily-cap reset hook into the dawn auto-save.

Reference: `[[Survival - Fire Making]]`, `[[Survival - Game time clock and needs]]`.
