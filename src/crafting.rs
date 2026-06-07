// Crafting data types — the CDDA-style core that the cooking card now
// rides on. The "system" (in the card sense) is small: a handful of
// shared enums (ModifierTag/Seasonings, CookableKind, CookedState,
// PanContents) plus a Recipe descriptor used by the Crafting menu tab.
//
// Execution paths still live in action.rs as per-verb ActionId arms
// (PlacePan, CookFish, ...). Recipe entries here name the ActionId they
// trigger, the ingredients/tools they need for menu greens/reds, and
// the modifier slots the row exposes. New cooking recipes plug in by
// adding a CookableKind variant + a Recipe row; the runtime is data-
// driven where it matters (which kind is cooking, which seasonings are
// set) and code-driven where it doesn't (the five-line execute fn).

use crate::action::ActionId;

/// Things that can be cooked in a pan. Drives `PanContents::Cooking` and
/// `ItemMetadata::Cooked` so we don't proliferate per-permutation
/// `ItemKind`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookableKind {
    Fish,
}

impl CookableKind {
    pub fn save_key(self) -> &'static str {
        match self {
            CookableKind::Fish => "fish",
        }
    }

    pub fn from_save_key(s: &str) -> Option<Self> {
        match s {
            "fish" => Some(CookableKind::Fish),
            _ => None,
        }
    }

    pub fn raw_name(self) -> &'static str {
        match self {
            CookableKind::Fish => "fish",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookedState {
    Ok,
    Burnt,
}

impl CookedState {
    pub fn save_key(self) -> &'static str {
        match self {
            CookedState::Ok => "ok",
            CookedState::Burnt => "burnt",
        }
    }
    pub fn from_save_key(s: &str) -> Option<Self> {
        match s {
            "ok" => Some(CookedState::Ok),
            "burnt" => Some(CookedState::Burnt),
            _ => None,
        }
    }
}

/// Composable seasoning tags. Stored on Cooked outputs and on in-flight
/// `PanContents::Cooking` as a bitfield so a single recipe (Cook fish)
/// covers Fish / Herb-Fish / Salt-Fish / Salt-Herb-Fish without four
/// distinct ItemKinds. Add a tag here + an entry in `Seasonings` helpers
/// + a column in eat-effects to plug in new modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ModifierTag {
    Herb = 0,
    // Salt = 1, Oil = 2, ... (future)
}


/// 8-bit modifier bitfield. Default = no modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Seasonings(pub u8);

impl Seasonings {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn has(self, t: ModifierTag) -> bool {
        (self.0 & (1 << t as u8)) != 0
    }

    pub fn set(&mut self, t: ModifierTag) {
        self.0 |= 1 << t as u8;
    }
}

/// What's currently inside a panned-on-fire cookware. Cookware ticks
/// drive `elapsed_secs` forward each game-second; `Cooking::status()`
/// reads off whether the food is still raw, cooked, or burnt based on
/// the per-input target/burn thresholds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanContents {
    Empty,
    Cooking {
        input: CookableKind,
        elapsed_secs: u32,
        seasonings: Seasonings,
    },
}

// ---- Per-input cooking timings ---------------------------------------
//
// Target = when "Ok" first appears; burn = when state flips to Burnt.
// Past burn the elapsed counter keeps climbing harmlessly (no triple
// state). Card spec: 120s target. Burn = 2x target.

impl CookableKind {
    pub fn target_secs(self) -> u32 {
        match self {
            CookableKind::Fish => 120,
        }
    }
    pub fn burn_secs(self) -> u32 {
        self.target_secs().saturating_mul(2)
    }
}

/// Current cook state of an in-flight `PanContents::Cooking`. Reading
/// helper so the menu UI + complete handlers don't reinvent the
/// threshold math.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookProgress {
    Raw,
    Ok,
    Burnt,
}

pub fn cook_progress(input: CookableKind, elapsed_secs: u32) -> CookProgress {
    if elapsed_secs >= input.burn_secs() {
        CookProgress::Burnt
    } else if elapsed_secs >= input.target_secs() {
        CookProgress::Ok
    } else {
        CookProgress::Raw
    }
}

// ---- Recipe registry -------------------------------------------------
//
// The Crafting tab iterates this list. Each row maps to a dedicated
// ActionId so the execute path stays in action.rs alongside every other
// verb. Adding a new recipe = append a Recipe row + add an ActionId arm.
//
// Per the "modifiers" decision: do NOT split into per-permutation rows
// (Herbed cook fish, Salted cook fish, ...). Modifiers stack at runtime
// via Seasonings; one base recipe handles every output combination.

pub struct Recipe {
    pub action: ActionId,
    pub name: &'static str,
    /// Crafting-station skill this recipe requires (e.g. "metallurgy" for an
    /// anvil). `None` = craftable anywhere; `Some(skill)` is gated by
    /// `World::player_near_station` in `action::evaluate`.
    pub station: Option<&'static str>,
}

pub const RECIPES: &[Recipe] = &[
    Recipe {
        action: ActionId::PlacePan,
        name: "Place pan on fire",
        station: None,
    },
    Recipe {
        action: ActionId::PickUpPan,
        name: "Pick up pan",
        station: None,
    },
    Recipe {
        action: ActionId::CookFish,
        name: "Cook fish",
        station: None,
    },
    Recipe {
        action: ActionId::SeasonPan,
        name: "Season pan w/ herb",
        station: None,
    },
    Recipe {
        action: ActionId::TakeFromPan,
        name: "Take from pan",
        station: None,
    },
    Recipe {
        action: ActionId::EatHerb,
        name: "Eat herb",
        station: None,
    },
    // Phase 3 station recipe — only craftable beside an anvil.
    Recipe {
        action: ActionId::ForgeNail,
        name: "Forge iron nails",
        station: Some("metallurgy"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasonings_bitfield_set_and_has() {
        let mut s = Seasonings::empty();
        assert!(!s.has(ModifierTag::Herb));
        s.set(ModifierTag::Herb);
        assert!(s.has(ModifierTag::Herb));
    }

    #[test]
    fn cook_progress_thresholds_fish() {
        assert_eq!(cook_progress(CookableKind::Fish, 0), CookProgress::Raw);
        assert_eq!(cook_progress(CookableKind::Fish, 119), CookProgress::Raw);
        assert_eq!(cook_progress(CookableKind::Fish, 120), CookProgress::Ok);
        assert_eq!(cook_progress(CookableKind::Fish, 239), CookProgress::Ok);
        assert_eq!(cook_progress(CookableKind::Fish, 240), CookProgress::Burnt);
        assert_eq!(cook_progress(CookableKind::Fish, 9999), CookProgress::Burnt);
    }

    #[test]
    fn recipes_table_includes_cook_fish_and_season_pan() {
        assert!(RECIPES.iter().any(|r| r.action == ActionId::CookFish));
        assert!(RECIPES.iter().any(|r| r.action == ActionId::SeasonPan));
    }
}
