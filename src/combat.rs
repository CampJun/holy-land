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

/// Anatomical slots per the CDDA-classic six. Coverage weights below
/// drive the post-hit body-part roll; see `Armor model.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyPart {
    Head,
    Torso,
    LArm,
    RArm,
    LLeg,
    RLeg,
}

impl BodyPart {
    /// Stable iteration order. Useful for tests, save round-trip, and
    /// any "for each part" loop.
    pub const ALL: [BodyPart; 6] = [
        BodyPart::Head,
        BodyPart::Torso,
        BodyPart::LArm,
        BodyPart::RArm,
        BodyPart::LLeg,
        BodyPart::RLeg,
    ];

    /// Coverage weight on the post-hit body-part roll. Numbers from
    /// `Armor model.md` (head 11, torso 35, arms 12 each, legs 15 each
    /// — sums to 100).
    pub fn coverage_weight(self) -> u8 {
        match self {
            BodyPart::Head => 11,
            BodyPart::Torso => 35,
            BodyPart::LArm => 12,
            BodyPart::RArm => 12,
            BodyPart::LLeg => 15,
            BodyPart::RLeg => 15,
        }
    }

    /// True for arms — used by the crippling rules (any crippled arm
    /// drops the wielded weapon in phase 2; phase 3 splits L/R hands).
    pub fn is_arm(self) -> bool {
        matches!(self, BodyPart::LArm | BodyPart::RArm)
    }

    /// True for legs — leg cripple halves effective speed.
    pub fn is_leg(self) -> bool {
        matches!(self, BodyPart::LLeg | BodyPart::RLeg)
    }

    /// True for head + torso — zero HP here is a death event.
    pub fn is_vital(self) -> bool {
        matches!(self, BodyPart::Head | BodyPart::Torso)
    }

    /// Short label for the in-game log line. Compact arm/leg labels
    /// ("L-arm" / "R-leg") so combat lines fit in the 38-cell HUD
    /// width without truncating the damage number.
    pub fn label(self) -> &'static str {
        match self {
            BodyPart::Head => "head",
            BodyPart::Torso => "chest",
            BodyPart::LArm => "L-arm",
            BodyPart::RArm => "R-arm",
            BodyPart::LLeg => "L-leg",
            BodyPart::RLeg => "R-leg",
        }
    }

    /// Stable save_key for serde round-trip.
    pub fn save_key(self) -> &'static str {
        match self {
            BodyPart::Head => "head",
            BodyPart::Torso => "torso",
            BodyPart::LArm => "l_arm",
            BodyPart::RArm => "r_arm",
            BodyPart::LLeg => "l_leg",
            BodyPart::RLeg => "r_leg",
        }
    }

    pub fn from_save_key(s: &str) -> Option<Self> {
        Some(match s {
            "head" => BodyPart::Head,
            "torso" => BodyPart::Torso,
            "l_arm" => BodyPart::LArm,
            "r_arm" => BodyPart::RArm,
            "l_leg" => BodyPart::LLeg,
            "r_leg" => BodyPart::RLeg,
            _ => return None,
        })
    }
}

/// Roll a body part by coverage weight. Sums to 100 so a single
/// `rng % 100 + 1` pick works in one pass.
pub fn roll_body_part(rng: &mut Rng) -> BodyPart {
    let mut roll = (rng.next_u32() % 100 + 1) as u8;
    for part in BodyPart::ALL {
        let w = part.coverage_weight();
        if roll <= w {
            return part;
        }
        roll -= w;
    }
    // Unreachable — coverage weights sum to 100 and roll is in 1..=100.
    BodyPart::Torso
}

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

/// Static weapon characteristics. Phase 3 lifts these off hardcoded
/// matches onto `ItemDef.weapon`.
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
    /// Max attack distance in Chebyshev tiles. `1` = adjacent only;
    /// `2` = reach-2 polearm (spear / gisarme / lance). Per
    /// `Reach and ranged.md`, swinging a reach-2 weapon at an adjacent
    /// target takes a no-reach penalty (see `NO_REACH_DAMAGE_PCT`).
    pub reach: u8,
}

/// Damage multiplier (percent) when a reach-≥2 weapon strikes an
/// adjacent target — the cards' "no-reach penalty" placeholder ~30%.
pub const NO_REACH_DAMAGE_PCT: u16 = 70;

/// Ranged weapon profile. Lives on `ItemDef.ranged` for bows; a
/// crossbow variant lands in a later phase with explicit reload state.
/// `ammo_kind` save_key for the consumed ammo item — the resolver
/// pulls one from the attacker's pack per shot.
#[derive(Clone, Copy, Debug)]
pub struct RangedProfile {
    pub to_hit: i16,
    pub damage_die: DamageTriplet,
    /// Cost in moves per shot (draw + loose).
    pub move_cost: u32,
    /// Maximum effective range in tiles. Targets beyond this can't be
    /// shot; targets near max take a graduated to-hit penalty.
    pub max_range: u8,
    /// `ItemKind::save_key` of the ammo this weapon expects. Empty
    /// means no ammo required (sling, throwing). Phase 6 supports bow
    /// + arrow only.
    pub ammo_kind: &'static str,
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

/// Roll the per-type damage triplet for a landed hit. Crit and the
/// no-reach penalty compose into a single percent multiplier applied
/// per-type after the Str + proficiency adds so the lift covers
/// everything uniformly; armor DR is subtracted last and the result
/// is floored at 0 per the damage-math card.
pub fn roll_damage(
    weapon: WeaponProfile,
    atk: AttackerStats,
    armor: ArmorDr,
    crit: bool,
    rng: &mut Rng,
) -> DamageTriplet {
    roll_damage_with_mult(weapon, atk, armor, crit, 100, rng)
}

/// Like `roll_damage` but with an extra percent multiplier applied
/// on top of the crit multiplier. Used for the no-reach polearm
/// penalty (mult_pct = NO_REACH_DAMAGE_PCT) and any future situational
/// modifiers (cover, prone, etc.).
pub fn roll_damage_with_mult(
    weapon: WeaponProfile,
    atk: AttackerStats,
    armor: ArmorDr,
    crit: bool,
    mult_pct: u16,
    rng: &mut Rng,
) -> DamageTriplet {
    let bash_raw =
        roll_die(weapon.damage_die.bash, rng) + atk.str_bonus.max(0) as u16 + bonus_u16(atk.weapon_prof);
    let cut_raw =
        roll_die(weapon.damage_die.cut, rng) + atk.str_bonus.max(0) as u16 + bonus_u16(atk.weapon_prof);
    let stab_raw = roll_die(weapon.damage_die.stab, rng)
        + (atk.str_bonus.max(0) as u16) / 2
        + bonus_u16(atk.weapon_prof);
    let crit_mult = if crit { CRIT_DMG_MULT_PCT } else { 100 };
    // Compose crit + situational into one percent factor so the math
    // matches whether you swap order. Divide by 10_000 (two ×100s) at
    // the end to keep precision.
    let factor = crit_mult as u32 * mult_pct as u32;
    let scale = |raw: u16| -> u16 {
        ((raw as u32) * factor / 10_000).min(u16::MAX as u32) as u16
    };
    DamageTriplet {
        bash: apply_dr(scale(bash_raw), armor.bash),
        cut: apply_dr(scale(cut_raw), armor.cut),
        stab: apply_dr(scale(stab_raw), armor.stab),
    }
}

/// Weapon-profile lookup. As of phase 3 this just defers to the
/// `ItemDef.weapon` field — kept as a free function so call sites
/// stay terse.
pub fn weapon_profile_for(kind: ItemKind) -> Option<WeaponProfile> {
    kind.def().weapon
}

/// PR A card 2 — which weapon-proficiency pool a wielded `ItemKind`
/// trains. Returns `None` for items the card calls "improvised" (no
/// proficiency XP from those swings). Used by card 3 to route the
/// per-prof XP grant; unarmed (no `Wielded` component) maps separately
/// at the call site since there's no `ItemKind` to dispatch on.
pub fn proficiency_for(kind: ItemKind) -> Option<crate::skill::Proficiency> {
    use crate::skill::Proficiency;
    Some(match kind {
        ItemKind::Knife => Proficiency::Knife,
        ItemKind::ShortSword | ItemKind::ArmingSword => Proficiency::Sword,
        ItemKind::Falchion => Proficiency::Falchion,
        ItemKind::Axe => Proficiency::Axe,
        ItemKind::Spear | ItemKind::Lance => Proficiency::SpearLance,
        ItemKind::Gisarme => Proficiency::GisarmeBill,
        ItemKind::Bow => Proficiency::Bow,
        ItemKind::Crossbow => Proficiency::Crossbow,
        ItemKind::Cudgel => Proficiency::MaceCudgel,
        ItemKind::Quarterstaff => Proficiency::Quarterstaff,
        _ => return None,
    })
}

/// PR A card 3 — which top-level armor-class skill a worn armor
/// `ItemKind` trains when its layer catches a hit. Padded/leather =
/// LightArmor; iron mail and helms = MediumArmor; coat-of-plates +
/// great helm = HeavyArmor. Returns `None` for shields and any worn
/// item that doesn't sort into a class.
pub fn armor_skill_for(kind: ItemKind) -> Option<crate::skill::SkillKind> {
    use crate::skill::SkillKind;
    Some(match kind {
        ItemKind::PaddedDoublet | ItemKind::LeatherJerkin => SkillKind::LightArmor,
        ItemKind::Hauberk
        | ItemKind::MailChausses
        | ItemKind::MailCoif
        | ItemKind::IronSkullcap
        | ItemKind::KettleHat => SkillKind::MediumArmor,
        ItemKind::CoatOfPlates | ItemKind::GreatHelm => SkillKind::HeavyArmor,
        _ => return None,
    })
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
            reach: 1,
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
            reach: 1,
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
    fn body_part_coverage_weights_sum_to_100() {
        let sum: u32 = BodyPart::ALL.iter().map(|p| p.coverage_weight() as u32).sum();
        assert_eq!(sum, 100);
    }

    #[test]
    fn body_part_roll_distribution_approximates_coverage() {
        use std::collections::HashMap;
        let mut rng = Rng::from_state(0xBEEF_F00D);
        let mut counts: HashMap<BodyPart, u32> = HashMap::new();
        let n = 20_000u32;
        for _ in 0..n {
            *counts.entry(roll_body_part(&mut rng)).or_insert(0) += 1;
        }
        // Expect each part within 3 percentage points of its weight.
        // Loose bound so the test is stable across PRNG variants.
        for part in BodyPart::ALL {
            let observed = *counts.get(&part).unwrap_or(&0) as f64 / n as f64;
            let expected = part.coverage_weight() as f64 / 100.0;
            let delta = (observed - expected).abs();
            assert!(
                delta < 0.03,
                "{:?}: observed {:.4}, expected {:.4} (delta {:.4})",
                part, observed, expected, delta
            );
        }
    }

    #[test]
    fn body_part_save_key_round_trip() {
        for part in BodyPart::ALL {
            assert_eq!(BodyPart::from_save_key(part.save_key()), Some(part));
        }
        assert_eq!(BodyPart::from_save_key("definitely_not_a_part"), None);
    }

    #[test]
    fn body_part_role_helpers() {
        assert!(BodyPart::Head.is_vital() && !BodyPart::Head.is_arm() && !BodyPart::Head.is_leg());
        assert!(BodyPart::Torso.is_vital());
        assert!(BodyPart::LArm.is_arm() && !BodyPart::LArm.is_vital());
        assert!(BodyPart::RLeg.is_leg() && !BodyPart::RLeg.is_vital());
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
