// Pure-math combat primitives. Phase-1 vertical slice (Cornish bandit):
// contested hit roll + bash/cut/stab damage triplet + flat armor DR.
// Per-body-part HP, layered armor, encumbrance, stamina, proficiencies,
// reach, ranged, grapple all land in later phases against this same
// chassis (see `obsidian/Cards/Survival - Combat - *.md`).
//
// All entry points take their `Rng` and inputs as values so the ECS-
// touching glue (world.rs) is the only place handling entity lookups.
// That keeps the math unit-testable without spinning up a world.
//
// Roll model. The cards spec "normal distribution centered on score"
// (CDDA's actual model). We approximate with a sum-of-two-uniforms
// triangular distribution, which is cheap, deterministic, and avoids
// transcendental functions on the Cortex-A7. Each side's roll:
//
//   side_roll = score + (d100 + d100 - 101) / 10   // range ~ [-9, +9]
//
// Margin = atk_roll - def_roll, in roughly [-18, +18] before scores.
// Tune `SWING_DIVISOR` if combat feels too swingy or too flat.

use crate::items::ItemKind;
use crate::skill::Rng;

/// Per-type damage from one swing. Each component is independent because
/// armor reduces each type independently — see `apply_armor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageTriplet {
    pub bash: u16,
    pub cut: u16,
    pub stab: u16,
}

impl DamageTriplet {
    pub fn total(self) -> u16 {
        self.bash.saturating_add(self.cut).saturating_add(self.stab)
    }
}

/// Flat per-type damage reduction. Mail blocks cut well, padded blocks
/// bash well, plate blocks all but is heavy. Phase 1 treats one armor
/// piece as the entire defender's DR; phase 2 layers pieces with
/// coverage rolls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmorDr {
    pub bash: u16,
    pub cut: u16,
    pub stab: u16,
}

/// Static weapon characteristics. Phase 1 keys these off `ItemKind` in
/// world.rs; phase 3 lifts them onto `ItemDef`.
#[derive(Clone, Copy, Debug)]
pub struct WeaponProfile {
    /// Additive hit bonus on the contested roll.
    pub to_hit: i16,
    /// Maximum value of each per-type damage die. A `0` here means the
    /// weapon does not deal that damage type. A roll of `(rng % die) + 1`
    /// chooses the actual hit; zero values skip the roll cleanly.
    pub damage_die: DamageTriplet,
    /// Swing cost in CDDA-style moves (see `world::MOVES_PER_SECOND`).
    pub move_cost: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AttackerStats {
    pub melee_skill: i16,
    pub weapon_prof: i16,
    pub agi_mod: i16,
    /// Added to bash + cut damage rolls in full; stab gets half (rounded
    /// down) per the damage-math card.
    pub str_bonus: i16,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefenderStats {
    pub dodge_skill: i16,
    pub agi_mod: i16,
    /// Subtracted from the defender's score per the armor-model card.
    pub encumbrance: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitOutcome {
    Miss,
    Hit { margin: i16 },
    Crit { margin: i16 },
}

impl HitOutcome {
    pub fn is_crit(self) -> bool {
        matches!(self, HitOutcome::Crit { .. })
    }
    pub fn landed(self) -> bool {
        !matches!(self, HitOutcome::Miss)
    }
}

/// Margin at which a hit upgrades to a critical. Placeholder per the
/// damage-math card; tune in balance pass.
pub const CRIT_MARGIN: i16 = 15;
/// Crit damage multiplier in percent (×1.5). Placeholder per the card.
pub const CRIT_DMG_MULT_PCT: u16 = 150;
/// Divides the sum-of-two-d100 swing to size the per-side roll variance.
/// Lower = swingier combat.
const SWING_DIVISOR: i16 = 10;

fn triangular_swing(rng: &mut Rng) -> i16 {
    let a = rng.d100() as i16;
    let b = rng.d100() as i16;
    // d100 returns 1..=100; sum 2..=200, mean 101. Centering at 0 and
    // dividing keeps the range roughly [-9, +9] (1..=100 sum tails are
    // rare).
    (a + b - 101) / SWING_DIVISOR
}

/// Contested hit roll. Hidden from the player for melee (the cards spec
/// log-message feedback only); the menu / HUD never reads the margin.
pub fn resolve_hit(
    atk: AttackerStats,
    def: DefenderStats,
    weapon: WeaponProfile,
    rng: &mut Rng,
) -> HitOutcome {
    let atk_score = atk.melee_skill + atk.weapon_prof + weapon.to_hit + atk.agi_mod;
    let def_score = def.dodge_skill + def.agi_mod - def.encumbrance;
    let atk_roll = atk_score + triangular_swing(rng);
    let def_roll = def_score + triangular_swing(rng);
    let margin = atk_roll - def_roll;
    if margin < 0 {
        HitOutcome::Miss
    } else if margin >= CRIT_MARGIN {
        HitOutcome::Crit { margin }
    } else {
        HitOutcome::Hit { margin }
    }
}

/// Roll the per-type damage triplet for a landed hit. Crit applies the
/// multiplier per-type AFTER the Str + proficiency adds so a single
/// 1.5× lift covers everything; armor DR is subtracted last and the
/// result is floored at 0 per the damage-math card.
pub fn roll_damage(
    weapon: WeaponProfile,
    atk: AttackerStats,
    armor: ArmorDr,
    crit: bool,
    rng: &mut Rng,
) -> DamageTriplet {
    let bash_raw =
        roll_die(weapon.damage_die.bash, rng) + atk.str_bonus.max(0) as u16 + bonus_u16(atk.weapon_prof);
    let cut_raw =
        roll_die(weapon.damage_die.cut, rng) + atk.str_bonus.max(0) as u16 + bonus_u16(atk.weapon_prof);
    let stab_raw = roll_die(weapon.damage_die.stab, rng)
        + (atk.str_bonus.max(0) as u16) / 2
        + bonus_u16(atk.weapon_prof);
    let mult = if crit { CRIT_DMG_MULT_PCT } else { 100 };
    DamageTriplet {
        bash: apply_dr(bash_raw * mult / 100, armor.bash),
        cut: apply_dr(cut_raw * mult / 100, armor.cut),
        stab: apply_dr(stab_raw * mult / 100, armor.stab),
    }
}

/// Stat-block lookup for phase-1 weapons. Phase 3 lifts these onto
/// `ItemDef`; for now the table sits next to the math that consumes it
/// so the whole phase-1 combat surface is in one file. `None` means the
/// item isn't a weapon (you can't bump-attack with a waterskin).
pub fn weapon_profile_for(kind: ItemKind) -> Option<WeaponProfile> {
    Some(match kind {
        // Player's starting wield. Per the damage-math card's example
        // dagger: fast, stab-heavy, modest cut.
        ItemKind::Knife => WeaponProfile {
            to_hit: 1,
            damage_die: DamageTriplet { bash: 0, cut: 2, stab: 8 },
            move_cost: 70,
        },
        // Cornish bandit's signature roll. Reach-2 is a phase-5 concern;
        // phase 1 treats the spear as adjacent-only.
        ItemKind::Spear => WeaponProfile {
            to_hit: 1,
            damage_die: DamageTriplet { bash: 1, cut: 0, stab: 9 },
            move_cost: 110,
        },
        // Felling axe doubles as a passable cleaver. Listed so the
        // player can wield their starting axe if they prefer.
        ItemKind::Axe => WeaponProfile {
            to_hit: 0,
            damage_die: DamageTriplet { bash: 3, cut: 9, stab: 0 },
            move_cost: 130,
        },
        _ => return None,
    })
}

/// Phase-1 stand-in for a real armor-piece resolver. Hardcodes the
/// bandit's Yeoman padded-doublet profile; the player has no armor in
/// phase 1 (Rabble tier). Phase 2/3 lift this to per-piece coverage +
/// DR table on equip slots.
pub fn unarmored() -> ArmorDr {
    ArmorDr::default()
}

pub fn padded_doublet_dr() -> ArmorDr {
    // Padded blocks bash well, cut middling, stab poorly (the cards'
    // rock-paper-scissors).
    ArmorDr { bash: 4, cut: 2, stab: 1 }
}

fn roll_die(max: u16, rng: &mut Rng) -> u16 {
    if max == 0 {
        return 0;
    }
    (rng.next_u32() % max as u32 + 1) as u16
}

fn bonus_u16(prof: i16) -> u16 {
    prof.max(0) as u16
}

fn apply_dr(raw: u16, dr: u16) -> u16 {
    raw.saturating_sub(dr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weapon_dagger() -> WeaponProfile {
        WeaponProfile {
            to_hit: 1,
            damage_die: DamageTriplet { bash: 0, cut: 2, stab: 8 },
            move_cost: 70,
        }
    }

    fn no_armor() -> ArmorDr {
        ArmorDr::default()
    }

    #[test]
    fn miss_when_attacker_far_below_defender() {
        let mut rng = Rng::from_state(0xAB12_3456);
        let atk = AttackerStats { melee_skill: 0, ..Default::default() };
        let def = DefenderStats { dodge_skill: 80, ..Default::default() };
        // 100 attempts; defender 80 above attacker. Every attempt should miss
        // since the worst-case attacker swing (+9) plus best-case defender
        // swing (-9 from defender's own roll) still leaves an 80-point gap.
        for _ in 0..100 {
            let out = resolve_hit(atk, def, weapon_dagger(), &mut rng);
            assert!(matches!(out, HitOutcome::Miss), "got {:?}", out);
        }
    }

    #[test]
    fn crit_when_attacker_far_above_defender() {
        let mut rng = Rng::from_state(0xCAFE_1234);
        let atk = AttackerStats { melee_skill: 90, weapon_prof: 5, ..Default::default() };
        let def = DefenderStats { dodge_skill: 0, ..Default::default() };
        for _ in 0..100 {
            let out = resolve_hit(atk, def, weapon_dagger(), &mut rng);
            assert!(matches!(out, HitOutcome::Crit { .. }), "got {:?}", out);
        }
    }

    #[test]
    fn damage_clamps_at_zero_when_armor_eats_everything() {
        let mut rng = Rng::from_state(0xDEAD_BEEF);
        let armor = ArmorDr { bash: 99, cut: 99, stab: 99 };
        let dmg = roll_damage(weapon_dagger(), AttackerStats::default(), armor, false, &mut rng);
        assert_eq!(dmg, DamageTriplet::default());
    }

    #[test]
    fn damage_unarmored_dagger_lands_stab() {
        let mut rng = Rng::from_state(0xFEED_FACE);
        // Dagger has cut=2, stab=8 dies; over many rolls every result
        // should be non-zero on stab.
        for _ in 0..50 {
            let dmg = roll_damage(weapon_dagger(), AttackerStats::default(), no_armor(), false, &mut rng);
            assert!(dmg.stab > 0, "stab should always roll >= 1 on cut+stab weapon");
            assert_eq!(dmg.bash, 0, "dagger has no bash die");
        }
    }

    #[test]
    fn crit_increases_damage_by_50pct() {
        // Same seed + same inputs → same die rolls. Crit path should
        // yield exactly the floor-mul of the non-crit path.
        let mut rng_a = Rng::from_state(0x1234_5678);
        let mut rng_b = Rng::from_state(0x1234_5678);
        let atk = AttackerStats { str_bonus: 4, weapon_prof: 3, ..Default::default() };
        let base = roll_damage(weapon_dagger(), atk, no_armor(), false, &mut rng_a);
        let crit = roll_damage(weapon_dagger(), atk, no_armor(), true, &mut rng_b);
        assert_eq!(crit.bash, base.bash * 150 / 100);
        assert_eq!(crit.cut, base.cut * 150 / 100);
        assert_eq!(crit.stab, base.stab * 150 / 100);
    }

    #[test]
    fn str_bonus_halves_for_stab() {
        // Stab gets str/2 (rounded down), bash+cut get full str. Verify
        // by zeroing the die and reading the pure bonus contribution.
        let mut rng = Rng::from_state(0x9999_AAAA);
        let pure_weapon = WeaponProfile {
            to_hit: 0,
            damage_die: DamageTriplet { bash: 0, cut: 0, stab: 0 },
            move_cost: 100,
        };
        let atk = AttackerStats { str_bonus: 5, weapon_prof: 2, ..Default::default() };
        let dmg = roll_damage(pure_weapon, atk, no_armor(), false, &mut rng);
        // bash: 5 (str) + 2 (prof) = 7
        // cut:  5 + 2 = 7
        // stab: 5/2 + 2 = 4
        assert_eq!(dmg.bash, 7);
        assert_eq!(dmg.cut, 7);
        assert_eq!(dmg.stab, 4);
    }

    #[test]
    fn outcome_helpers() {
        assert!(HitOutcome::Crit { margin: 20 }.is_crit());
        assert!(!HitOutcome::Hit { margin: 5 }.is_crit());
        assert!(HitOutcome::Hit { margin: 0 }.landed());
        assert!(HitOutcome::Crit { margin: 20 }.landed());
        assert!(!HitOutcome::Miss.landed());
    }
}
