// Context-action registry: the verbs the player picks from the command
// menu (tap Y) and later the hold-Y radial overlay (phase 15). Each verb
// declares availability with a human-readable reason if unavailable, plus
// a base cost in game-seconds.
//
// Phase 7 wires the resolver + vertical menu UX. Only `Pickup` is fully
// implemented — every other slice-1 verb is a greyed-out stub whose
// `reason` names the phase that unlocks it. Reading the menu in-game is
// a live punch-list of remaining work.

use crate::crafting::{
    cook_progress, CookProgress, CookableKind, CookedState, ModifierTag, PanContents, Seasonings,
};
use crate::items::{ItemInstance, ItemKind, ItemMetadata};
use crate::needs::NeedKind;
use crate::skill::{self, SkillKind};
use crate::world::{TerrainKind, World};

// ---- Per-verb design constants ---------------------------------------
//
// STYLE.md §2: source of truth for verb tuning lives in this module,
// not in world.rs. World.rs only owns engine-level constants (the
// MOVES_PER_SECOND denominator, the tile-step move-cost, day-length,
// FOV radii). Verb costs are inlined in `ActionId::move_cost()` so
// there's literally one match arm per verb to edit when rebalancing.
// All values are in CDDA-style moves; multiply by 0.01s at baseline
// speed to read as wall-clock. Other verb tuning (materials,
// modifiers, output) lives as `const`s below, grouped by verb.

// Fire Making (verb: StartFire).
const FIRE_BONUS_FLINT_AND_STEEL: i32 = 30;
const FIRE_MIN_TINDER: u32 = 1;
const FIRE_MIN_KINDLING: u32 = 3;
const FIRE_MIN_FUEL: u32 = 2;
const FIRE_FUEL_SECONDS_PER_LIGHT: u32 = 3600;
// FeedFire (phase 12): half the per-light burn since you're skipping
// the tinder+kindling ignition synergy — just tossing a stick on.
const FIRE_FUEL_SECONDS_PER_FEED: u32 = 1800;

// Chop Tree (verb: ChopTree).
const FELLED_FIREWOOD_MIN: u16 = 3;
const FELLED_FIREWOOD_MAX: u16 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionId {
    Pickup,
    DrinkFromStream,
    FillWaterskin,
    DrinkWaterskin,
    EatRation,
    EatHerb,
    ChopTree,
    PickHerb,
    PitchTent,
    UnrollBedroll,
    SetupCamp,
    StartFire,
    FeedFire,
    Sleep,
    Fishing,
    // ---- Crafting (slice-1 cooking card) ----
    /// Move a CookingPan from pack onto an adjacent Lit fire. The fire's
    /// fuel transfers into a single PannedOnFire item on that cell.
    PlacePan,
    /// Reverse of PlacePan: lift an empty PannedOnFire back into pack;
    /// remaining fuel becomes a Lit firewood on the cell.
    PickUpPan,
    /// 5s setup that puts a Fish from pack into a reachable empty
    /// pan-on-fire. The pan ticks the 120s passive cook after.
    CookFish,
    /// Add a Herb modifier to a pan currently cooking. 5s. Sets the
    /// Herb seasoning bit; first stack wins.
    SeasonPan,
    /// Lift a finished cook out of the pan into the pack (or onto the
    /// player's cell if pack is full). Returns the pan to Empty.
    TakeFromPan,
    // ---- Phase D harvest verbs (target cell.decoration adjacent to
    // the player). Each clears the decoration and drops yields into
    // the pack. Foraging XP awarded on successful harvest.
    HarvestMoss,
    CutFern,
    DigFern,
    CutGorse,
    CutBracken,
    /// Open the ranged-targeting cursor. Available when the player's
    /// wielded item has a ranged profile (bow / crossbow). The cursor
    /// commits to a shot via A; cancel via B. Execution lives in main.rs
    /// — the verb itself just signals "open targeting mode."
    Aim,
    /// Devon-wrestling grapple. Adjacent hostile only. Str contest;
    /// success Grapples target (can't move/attack until break).
    /// Drains stamina; requires Stamina >= HEAVY_FLOOR.
    Grapple,
    /// Throw a grappled hostile to the ground (Prone). Adjacent
    /// grappled hostile only.
    Throw,
    /// Knock the wielded weapon out of an adjacent hostile's hand.
    /// Str contest; success drops their Wielded onto their cell.
    Disarm,
}

impl ActionId {
    /// Base cost in CDDA-style moves before need-penalty amplification.
    /// Single source of truth for verb tuning. The resolver translates
    /// to wall-clock game-seconds via `World::moves_to_seconds` (which
    /// folds in the actor's effective speed) for the menu's "Xs"
    /// readout; executors pass the move-cost into `world.spend_moves`.
    /// Change a value here and every call site picks it up.
    ///
    /// Baseline player speed is 100, so a move-cost of N maps to
    /// `N/100` game-seconds at full health. Combat verbs landing in
    /// later cards (Attack/Shoot/Brace/Grapple/…) will be tuned in
    /// these same units alongside per-weapon move-cost tables.
    pub fn move_cost(self) -> u32 {
        match self {
            ActionId::Pickup => 300,
            ActionId::EatRation => 1_000,
            ActionId::EatHerb => 500,
            ActionId::DrinkWaterskin => 500,
            ActionId::DrinkFromStream => 1_000,
            ActionId::FillWaterskin => 2_000,
            ActionId::PitchTent => 30_000,
            ActionId::UnrollBedroll => 3_000,
            // SetupCamp queues PitchTent + UnrollBedroll; the menu's
            // surfaced cost is the sum so the player sees the total.
            ActionId::SetupCamp => Self::PitchTent.move_cost() + Self::UnrollBedroll.move_cost(),
            ActionId::StartFire => 6_000,
            ActionId::FeedFire => 1_500,
            ActionId::ChopTree => 12_000,
            ActionId::PickHerb => 1_000,
            // Sleep computes its real target duration at execute time
            // (next dawn or 8h, whichever is shorter); the menu's cost
            // readout is just a placeholder. Phase 16 ships the
            // duration math; an explicit "Sleep until..." picker is a
            // future polish item.
            ActionId::Sleep => 0,
            ActionId::Fishing => 60_000,
            ActionId::PlacePan => 500,
            ActionId::PickUpPan => 500,
            ActionId::CookFish => 500,
            ActionId::SeasonPan => 500,
            ActionId::TakeFromPan => 500,
            // Phase D harvest verbs (moves = old game-seconds × 100).
            ActionId::HarvestMoss => 3_000,
            ActionId::CutFern => 1_000,
            ActionId::DigFern => 6_000,
            ActionId::CutGorse => 12_000,
            ActionId::CutBracken => 2_000,
            // Aim itself is free — opening the targeting cursor doesn't
            // advance the clock. The shot fires from the cursor's
            // commit path (`World::perform_ranged_attack`), which
            // charges the bow's `RangedProfile.move_cost`.
            ActionId::Aim => 0,
            // Heavy wrestling actions take longer than a normal swing
            // (per the stamina card — grapple/throw/disarm are the
            // canonical heavy-action examples).
            ActionId::Grapple => 200,
            ActionId::Throw => 150,
            ActionId::Disarm => 150,
        }
    }

    /// Stable string key for save round-tripping (ActiveActionSave). Keep
    /// in sync with `from_save_key`. Mirrors the ItemKind::save_key
    /// pattern.
    pub fn save_key(self) -> &'static str {
        match self {
            ActionId::Pickup => "pickup",
            ActionId::DrinkFromStream => "drink_from_stream",
            ActionId::FillWaterskin => "fill_waterskin",
            ActionId::DrinkWaterskin => "drink_waterskin",
            ActionId::EatRation => "eat_ration",
            ActionId::EatHerb => "eat_herb",
            ActionId::ChopTree => "chop_tree",
            ActionId::PickHerb => "pick_herb",
            ActionId::PitchTent => "pitch_tent",
            ActionId::UnrollBedroll => "unroll_bedroll",
            ActionId::SetupCamp => "setup_camp",
            ActionId::StartFire => "start_fire",
            ActionId::FeedFire => "feed_fire",
            ActionId::Sleep => "sleep",
            ActionId::Fishing => "fishing",
            ActionId::PlacePan => "place_pan",
            ActionId::PickUpPan => "pick_up_pan",
            ActionId::CookFish => "cook_fish",
            ActionId::SeasonPan => "season_pan",
            ActionId::TakeFromPan => "take_from_pan",
            ActionId::HarvestMoss => "harvest_moss",
            ActionId::CutFern => "cut_fern",
            ActionId::DigFern => "dig_fern",
            ActionId::CutGorse => "cut_gorse",
            ActionId::CutBracken => "cut_bracken",
            ActionId::Aim => "aim",
            ActionId::Grapple => "grapple",
            ActionId::Throw => "throw",
            ActionId::Disarm => "disarm",
        }
    }

    pub fn from_save_key(s: &str) -> Option<Self> {
        Some(match s {
            "pickup" => ActionId::Pickup,
            "drink_from_stream" => ActionId::DrinkFromStream,
            "fill_waterskin" => ActionId::FillWaterskin,
            "drink_waterskin" => ActionId::DrinkWaterskin,
            "eat_ration" => ActionId::EatRation,
            "eat_herb" => ActionId::EatHerb,
            "chop_tree" => ActionId::ChopTree,
            "pick_herb" => ActionId::PickHerb,
            "pitch_tent" => ActionId::PitchTent,
            "unroll_bedroll" => ActionId::UnrollBedroll,
            "setup_camp" => ActionId::SetupCamp,
            "start_fire" => ActionId::StartFire,
            "feed_fire" => ActionId::FeedFire,
            "sleep" => ActionId::Sleep,
            "fishing" => ActionId::Fishing,
            "place_pan" => ActionId::PlacePan,
            "pick_up_pan" => ActionId::PickUpPan,
            "cook_fish" => ActionId::CookFish,
            "season_pan" => ActionId::SeasonPan,
            "take_from_pan" => ActionId::TakeFromPan,
            "harvest_moss" => ActionId::HarvestMoss,
            "cut_fern" => ActionId::CutFern,
            "dig_fern" => ActionId::DigFern,
            "cut_gorse" => ActionId::CutGorse,
            "cut_bracken" => ActionId::CutBracken,
            "aim" => ActionId::Aim,
            "grapple" => ActionId::Grapple,
            "throw" => ActionId::Throw,
            "disarm" => ActionId::Disarm,
            _ => return None,
        })
    }
}

pub struct ContextAction {
    pub id: ActionId,
    pub name: &'static str,
    pub description: &'static str,
}

/// Render order = priority order for the vertical menu. Phase 15's radial
/// overlay will sort dynamically; this list defines the canonical full
/// catalog.
pub const ALL_ACTIONS: &[ContextAction] = &[
    ContextAction {
        id: ActionId::Pickup,
        name: "Pick up",
        description: "Take items from this cell.",
    },
    ContextAction {
        id: ActionId::DrinkFromStream,
        name: "Drink from stream",
        description: "Drink directly from an adjacent stream or pond.",
    },
    ContextAction {
        id: ActionId::FillWaterskin,
        name: "Fill waterskin",
        description: "Top up an empty waterskin from a water source.",
    },
    ContextAction {
        id: ActionId::DrinkWaterskin,
        name: "Drink waterskin",
        description: "Sip from a carried waterskin.",
    },
    ContextAction {
        id: ActionId::EatRation,
        name: "Eat ration",
        description: "Consume one packed ration.",
    },
    ContextAction {
        id: ActionId::EatHerb,
        name: "Eat herb",
        description: "Eat a fresh herb raw for a small hunger restore.",
    },
    ContextAction {
        id: ActionId::ChopTree,
        name: "Chop tree",
        description: "Fell an adjacent tree (requires axe).",
    },
    ContextAction {
        id: ActionId::PickHerb,
        name: "Pick herb",
        description: "Harvest an adjacent herb patch (requires knife).",
    },
    ContextAction {
        id: ActionId::PitchTent,
        name: "Pitch tent",
        description: "Set up a tent on the current cell.",
    },
    ContextAction {
        id: ActionId::UnrollBedroll,
        name: "Unroll bedroll",
        description: "Lay out a bedroll inside a tent.",
    },
    ContextAction {
        id: ActionId::SetupCamp,
        name: "Setup camp",
        description: "Pitch the tent, then unroll the bedroll inside.",
    },
    ContextAction {
        id: ActionId::StartFire,
        name: "Start fire",
        description: "Strike flint and steel over gathered kindling.",
    },
    ContextAction {
        id: ActionId::FeedFire,
        name: "Feed fire",
        description: "Add a piece of firewood to a lit fire here.",
    },
    ContextAction {
        id: ActionId::Sleep,
        name: "Sleep",
        description: "Sleep until dawn or for 8 game-hours.",
    },
    ContextAction {
        id: ActionId::Fishing,
        name: "Try fishing",
        description: "Cast a hand-line into an adjacent pond.",
    },
    ContextAction {
        id: ActionId::HarvestMoss,
        name: "Harvest moss",
        description: "Scrape moss from a stone or root (knife).",
    },
    ContextAction {
        id: ActionId::CutFern,
        name: "Cut fern",
        description: "Slice fern fronds (knife).",
    },
    ContextAction {
        id: ActionId::DigFern,
        name: "Dig fern",
        description: "Dig the fern up for root + fronds (knife).",
    },
    ContextAction {
        id: ActionId::CutGorse,
        name: "Cut gorse",
        description: "Hack gorse into a faggot of kindling (axe).",
    },
    ContextAction {
        id: ActionId::CutBracken,
        name: "Cut bracken",
        description: "Cut bracken straw for bedding (knife).",
    },
    ContextAction {
        id: ActionId::Aim,
        name: "Aim",
        description: "Raise your bow and pick a target.",
    },
    ContextAction {
        id: ActionId::Grapple,
        name: "Grapple",
        description: "Lock up the adjacent foe — they can't act.",
    },
    ContextAction {
        id: ActionId::Throw,
        name: "Throw",
        description: "Slam a grappled foe to the ground (Prone).",
    },
    ContextAction {
        id: ActionId::Disarm,
        name: "Disarm",
        description: "Strike an adjacent foe's weapon free.",
    },
];

#[derive(Clone, Debug)]
pub enum Availability {
    Available { cost_game_seconds: u32 },
    Unavailable { reason: &'static str },
}

impl Availability {
    /// Helper for the common "available iff some pack/world predicate is
    /// true" pattern. Use as:
    ///   Availability::from_has(pack.has_stack(Ration), cost, "no rations")
    pub fn from_has(has: bool, cost_game_seconds: u32, missing_reason: &'static str) -> Self {
        if has {
            Self::Available { cost_game_seconds }
        } else {
            Self::Unavailable {
                reason: missing_reason,
            }
        }
    }
}

pub fn evaluate(world: &World, id: ActionId) -> Availability {
    let pack = world.player_pack();
    let cost = world.moves_to_seconds(id.move_cost());
    match id {
        ActionId::Pickup => eval_pickup(world),
        ActionId::EatRation => {
            Availability::from_has(pack.has_stack(ItemKind::Ration), cost, "no rations in pack")
        }
        ActionId::EatHerb => {
            Availability::from_has(pack.has_stack(ItemKind::Herb), cost, "no herbs in pack")
        }
        ActionId::DrinkWaterskin => Availability::from_has(
            pack.has_waterskin_with_water(),
            cost,
            "no water in waterskins",
        ),
        ActionId::DrinkFromStream => eval_drink_from_stream(world),
        ActionId::FillWaterskin => eval_fill_waterskin(world),
        ActionId::ChopTree => eval_chop_tree(world),
        ActionId::PickHerb => eval_pick_herb(world),
        ActionId::PitchTent => {
            Availability::from_has(pack.has_stack(ItemKind::Tent), cost, "no tent in pack")
        }
        ActionId::UnrollBedroll => Availability::from_has(
            pack.has_stack(ItemKind::Bedroll),
            cost,
            "no bedroll in pack",
        ),
        ActionId::SetupCamp => Availability::from_has(
            pack.has_stack(ItemKind::Tent) && pack.has_stack(ItemKind::Bedroll),
            cost,
            "need tent + bedroll in pack",
        ),
        ActionId::StartFire => eval_start_fire(world),
        ActionId::FeedFire => eval_feed_fire(world),
        ActionId::Sleep => Availability::Available {
            cost_game_seconds: world.moves_to_seconds(ActionId::Sleep.move_cost()),
        },
        ActionId::Fishing => eval_fishing(world),
        ActionId::PlacePan => eval_place_pan(world),
        ActionId::PickUpPan => eval_pick_up_pan(world),
        ActionId::CookFish => eval_cook_fish(world),
        ActionId::SeasonPan => eval_season_pan(world),
        ActionId::TakeFromPan => eval_take_from_pan(world),
        ActionId::HarvestMoss => eval_harvest_decoration(
            world,
            |d| matches!(d, crate::flora::Decoration::Moss),
            ActionId::HarvestMoss,
            HARVEST_REQUIRES_KNIFE,
            "no moss adjacent",
        ),
        ActionId::CutFern => eval_harvest_decoration(
            world,
            |d| matches!(d, crate::flora::Decoration::Fern { .. }),
            ActionId::CutFern,
            HARVEST_REQUIRES_KNIFE,
            "no fern adjacent",
        ),
        ActionId::DigFern => eval_harvest_decoration(
            world,
            |d| matches!(d, crate::flora::Decoration::Fern { .. }),
            ActionId::DigFern,
            HARVEST_REQUIRES_KNIFE,
            "no fern adjacent",
        ),
        ActionId::CutGorse => eval_harvest_decoration(
            world,
            |d| matches!(d, crate::flora::Decoration::Gorse { .. }),
            ActionId::CutGorse,
            HARVEST_REQUIRES_AXE,
            "no gorse adjacent",
        ),
        ActionId::CutBracken => eval_harvest_decoration(
            world,
            |d| matches!(d, crate::flora::Decoration::Bracken { .. }),
            ActionId::CutBracken,
            HARVEST_REQUIRES_KNIFE,
            "no bracken adjacent",
        ),
        ActionId::Aim => eval_aim(world),
        ActionId::Grapple => eval_grapple_or_disarm(world, false),
        ActionId::Disarm => eval_grapple_or_disarm(world, true),
        ActionId::Throw => eval_throw(world),
    }
}

fn eval_grapple_or_disarm(world: &World, require_wielded: bool) -> Availability {
    if !world.player_has_stamina_for_heavy() {
        return Availability::Unavailable { reason: "winded" };
    }
    let Some(target) = world.adjacent_hostile() else {
        return Availability::Unavailable { reason: "no foe adjacent" };
    };
    if require_wielded && !world.entity_has_wielded(target) {
        return Availability::Unavailable { reason: "foe is unarmed" };
    }
    Availability::Available { cost_game_seconds: 0 }
}

fn eval_throw(world: &World) -> Availability {
    if !world.player_has_stamina_for_heavy() {
        return Availability::Unavailable { reason: "winded" };
    }
    let Some(target) = world.adjacent_hostile() else {
        return Availability::Unavailable { reason: "no foe adjacent" };
    };
    if !world.entity_is_grappled(target) {
        return Availability::Unavailable { reason: "grapple them first" };
    }
    Availability::Available { cost_game_seconds: 0 }
}

fn eval_aim(world: &World) -> Availability {
    // Available iff the player is wielding a ranged weapon AND has at
    // least one round of the matching ammo in pack.
    use crate::items::ItemKind as IK;
    let pack = world.player_pack();
    let wielded = world
        .player_main_hand_kind()
        .and_then(|k| k.def().ranged.map(|r| (k, r)));
    let Some((kind, ranged)) = wielded else {
        return Availability::Unavailable { reason: "no ranged weapon equipped" };
    };
    let _ = kind;
    if !ranged.ammo_kind.is_empty() {
        let Some(ammo) = IK::from_save_key(ranged.ammo_kind) else {
            return Availability::Unavailable { reason: "ammo kind unknown" };
        };
        if !pack.has_stack(ammo) {
            return Availability::Unavailable { reason: "no ammo in pack" };
        }
    }
    Availability::Available { cost_game_seconds: 0 }
}

/// Counts the materials reachable by a Fire Making attempt: the
/// player's pack PLUS the 3x3 square of cells centered on the player.
/// Per the design card (Survival - Fire Making.md), materials in any
/// adjacent cell or in inventory count toward the requirements.
#[derive(Default, Debug, Clone, Copy)]
struct FireMaterials {
    pub tinder: u32,   // twigs or grass blades
    pub kindling: u32, // sticks
    pub fuel: u32,     // firewood
}

fn count_fire_materials(world: &World) -> FireMaterials {
    let mut m = FireMaterials::default();
    let add_item = |m: &mut FireMaterials, i: &ItemInstance| {
        let count = i.count as u32;
        match i.kind {
            ItemKind::Twig | ItemKind::GrassBlade => m.tinder += count,
            ItemKind::Stick => m.kindling += count,
            ItemKind::Firewood => m.fuel += count,
            _ => {}
        }
    };
    // Pack contents.
    for item in world.player_pack().contents.iter() {
        add_item(&mut m, item);
    }
    // 3x3 cells centered on the player.
    let p = world.player_pos();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(cell) = world.cell_at((p.x + dx) as i64, (p.y + dy) as i64) else {
                continue;
            };
            for item in cell.items.iter() {
                // Don't count lit-fire fuel as available reserve — it's
                // currently burning.
                if matches!(item.metadata, ItemMetadata::Lit { .. }) {
                    continue;
                }
                add_item(&mut m, item);
            }
        }
    }
    m
}

fn eval_start_fire(world: &World) -> Availability {
    if !world.player_pack().has_stack(ItemKind::FlintAndSteel) {
        return Availability::Unavailable {
            reason: "no flint and steel",
        };
    }
    let m = count_fire_materials(world);
    if m.tinder < FIRE_MIN_TINDER {
        return Availability::Unavailable {
            reason: "need tinder (twig or grass)",
        };
    }
    if m.kindling < FIRE_MIN_KINDLING {
        return Availability::Unavailable {
            reason: "need 3 sticks",
        };
    }
    if m.fuel < FIRE_MIN_FUEL {
        return Availability::Unavailable {
            reason: "need 2 firewood",
        };
    }
    Availability::Available {
        cost_game_seconds: world.moves_to_seconds(ActionId::StartFire.move_cost()),
    }
}

fn player_cell_has_lit_fire(world: &World) -> bool {
    let p = world.player_pos();
    world
        .cell_at(p.x as i64, p.y as i64)
        .map_or(false, |c| {
            c.items
                .iter()
                .any(|i| matches!(i.metadata, ItemMetadata::Lit { .. }))
        })
}

fn eval_feed_fire(world: &World) -> Availability {
    if !player_cell_has_lit_fire(world) {
        return Availability::Unavailable {
            reason: "no fire here to feed",
        };
    }
    // count_fire_materials already skips lit-fire fuel, so this won't
    // falsely count the burning Firewood as a feedable reserve.
    if count_fire_materials(world).fuel < 1 {
        return Availability::Unavailable {
            reason: "no firewood within reach",
        };
    }
    Availability::Available {
        cost_game_seconds: world.moves_to_seconds(ActionId::FeedFire.move_cost()),
    }
}

fn eval_pickup(world: &World) -> Availability {
    let pos = world.player_pos();
    let cell = match world.cell_at(pos.x as i64, pos.y as i64) {
        Some(c) => c,
        None => {
            return Availability::Unavailable {
                reason: "off the map",
            }
        }
    };
    if cell.items.is_empty() {
        return Availability::Unavailable {
            reason: "nothing here",
        };
    }
    let stack_weight: u32 = cell.items.iter().map(|i| i.total_weight_g()).sum();
    let pack = world.player_pack();
    if pack
        .total_weight_g()
        .saturating_add(stack_weight)
        > pack.capacity_g
    {
        return Availability::Unavailable {
            reason: "pack too full",
        };
    }
    Availability::Available {
        cost_game_seconds: world.moves_to_seconds(ActionId::Pickup.move_cost()),
    }
}

#[derive(Debug)]
pub enum ExecuteOutcome {
    /// The action ran; the inner message is suitable for log_info.
    Done(String),
    /// The action wants to open the ranged-targeting cursor instead of
    /// resolving immediately. Main.rs catches this and switches input
    /// mode; the actual shot resolution lives on the cursor's commit
    /// path (`World::perform_ranged_attack`).
    OpenAim,
}

impl ExecuteOutcome {
    /// Convenience for callers that want the log message (or empty
    /// string for non-Done outcomes).
    pub fn message(&self) -> &str {
        match self {
            ExecuteOutcome::Done(s) => s.as_str(),
            ExecuteOutcome::OpenAim => "",
        }
    }
}

pub fn execute(world: &mut World, id: ActionId) -> ExecuteOutcome {
    match id {
        ActionId::Pickup => {
            let picked = world.try_pickup_all_at_player();
            if picked > 0 {
                world.spend_moves(ActionId::Pickup.move_cost());
            }
            ExecuteOutcome::Done(format!("picked up {} stack(s)", picked))
        }
        ActionId::EatRation => consume_and_restore(
            world,
            ConsumeFrom::Stack(ItemKind::Ration),
            NeedKind::Hunger,
            25,
            world.moves_to_seconds(ActionId::EatRation.move_cost()),
            "ate a ration (+25 hunger)",
            "no rations to eat",
        ),
        ActionId::EatHerb => consume_and_restore(
            world,
            ConsumeFrom::Stack(ItemKind::Herb),
            NeedKind::Hunger,
            5,
            world.moves_to_seconds(ActionId::EatHerb.move_cost()),
            "ate a herb (+5 hunger)",
            "no herbs to eat",
        ),
        ActionId::DrinkWaterskin => consume_and_restore(
            world,
            ConsumeFrom::WaterskinCharge,
            NeedKind::Thirst,
            20,
            world.moves_to_seconds(ActionId::DrinkWaterskin.move_cost()),
            "drank from waterskin (+20 thirst)",
            "no water to drink",
        ),
        ActionId::PitchTent => {
            let tgt = world.moves_to_seconds(ActionId::PitchTent.move_cost());
            world.queue_multi_turn(&[(ActionId::PitchTent, tgt)]);
            ExecuteOutcome::Done("pitching tent...".to_string())
        }
        ActionId::UnrollBedroll => {
            let tgt = world.moves_to_seconds(ActionId::UnrollBedroll.move_cost());
            world.queue_multi_turn(&[(ActionId::UnrollBedroll, tgt)]);
            ExecuteOutcome::Done("unrolling bedroll...".to_string())
        }
        ActionId::SetupCamp => {
            let tent = world.moves_to_seconds(ActionId::PitchTent.move_cost());
            let bed = world.moves_to_seconds(ActionId::UnrollBedroll.move_cost());
            world.queue_multi_turn(&[
                (ActionId::PitchTent, tent),
                (ActionId::UnrollBedroll, bed),
            ]);
            ExecuteOutcome::Done("setting up camp...".to_string())
        }
        ActionId::StartFire => execute_start_fire(world),
        ActionId::FeedFire => execute_feed_fire(world),
        ActionId::ChopTree => execute_chop_tree(world),
        ActionId::PickHerb => execute_pick_herb(world),
        ActionId::DrinkFromStream => execute_drink_from_stream(world),
        ActionId::FillWaterskin => execute_fill_waterskin(world),
        ActionId::Sleep => execute_sleep(world),
        ActionId::Fishing => execute_fishing(world),
        ActionId::PlacePan
        | ActionId::PickUpPan
        | ActionId::CookFish
        | ActionId::SeasonPan
        | ActionId::TakeFromPan => execute_queue_5s(world, id),
        ActionId::HarvestMoss => execute_harvest_moss(world),
        ActionId::CutFern => execute_cut_fern(world),
        ActionId::DigFern => execute_dig_fern(world),
        ActionId::CutGorse => execute_cut_gorse(world),
        ActionId::CutBracken => execute_cut_bracken(world),
        ActionId::Aim => ExecuteOutcome::OpenAim,
        ActionId::Grapple => execute_grapple(world),
        ActionId::Throw => execute_throw(world),
        ActionId::Disarm => execute_disarm(world),
    }
}

fn execute_grapple(world: &mut World) -> ExecuteOutcome {
    let Some(target) = world.adjacent_hostile() else {
        return ExecuteOutcome::Done("no foe adjacent".to_string());
    };
    let msg = world.perform_grapple(target);
    world.spend_moves(ActionId::Grapple.move_cost());
    ExecuteOutcome::Done(msg)
}

fn execute_throw(world: &mut World) -> ExecuteOutcome {
    let Some(target) = world.adjacent_hostile() else {
        return ExecuteOutcome::Done("no foe adjacent".to_string());
    };
    let msg = world.perform_throw(target);
    world.spend_moves(ActionId::Throw.move_cost());
    ExecuteOutcome::Done(msg)
}

fn execute_disarm(world: &mut World) -> ExecuteOutcome {
    let Some(target) = world.adjacent_hostile() else {
        return ExecuteOutcome::Done("no foe adjacent".to_string());
    };
    let msg = world.perform_disarm(target);
    world.spend_moves(ActionId::Disarm.move_cost());
    ExecuteOutcome::Done(msg)
}

/// Crafting verbs share a single 5-second multi-turn queue path; the
/// real work happens in `complete_step` once the timer runs out. This
/// keeps the player visibly committed to the recipe (and lets the menu
/// system surface a progress bar) without each verb reinventing the
/// queue boilerplate.
fn execute_queue_5s(world: &mut World, id: ActionId) -> ExecuteOutcome {
    let tgt = world.moves_to_seconds(id.move_cost());
    world.queue_multi_turn(&[(id, tgt)]);
    let msg = match id {
        ActionId::PlacePan => "placing pan on fire...",
        ActionId::PickUpPan => "lifting pan...",
        ActionId::CookFish => "setting fish in pan...",
        ActionId::SeasonPan => "seasoning...",
        ActionId::TakeFromPan => "taking from pan...",
        _ => "...",
    };
    ExecuteOutcome::Done(msg.to_string())
}

/// Phase-10 instant verb (not multi-turn for slice 1 — see card). Pays
/// the attempt cost, rolls a Fire Making skill check, and either lights
/// a fire (consuming 1 tinder + 2 kindling + 1 fuel) or fails (consuming
/// 1 tinder). Either outcome awards XP per skill.rs's rules.
fn execute_start_fire(world: &mut World) -> ExecuteOutcome {
    // Re-check materials defensively; the menu's evaluate should have
    // already gated this, but a debug command could have removed them
    // between menu-open and confirm.
    let m = count_fire_materials(world);
    if m.tinder < FIRE_MIN_TINDER || m.kindling < FIRE_MIN_KINDLING || m.fuel < FIRE_MIN_FUEL {
        return ExecuteOutcome::Done("not enough materials".to_string());
    }
    if !world.player_pack().has_stack(ItemKind::FlintAndSteel) {
        return ExecuteOutcome::Done("no flint and steel".to_string());
    }

    // Roll the check FIRST so we know whether to consume the success
    // materials (1 tinder + 2 kindling + 1 fuel) vs only the failure
    // materials (1 tinder spent striking).
    let skill_value = world.player_skills().fire_making.value;
    let roll = world.rng.d100();
    let success = skill::skill_check_with_roll(skill_value, FIRE_BONUS_FLINT_AND_STEEL, roll);

    // Always consume 1 tinder regardless of outcome (the strike at least
    // singes the kindling whether it catches or not).
    consume_one_fire_material(world, FireMaterial::Tinder);

    let msg = if success {
        // Success cost (per Survival - Fire Making.md): 2 kindling + 1
        // fuel on top of the always-consumed 1 tinder. The extra
        // reserve (3rd kindling, 2nd fuel) stays in inventory/ground.
        for _ in 0..2 {
            consume_one_fire_material(world, FireMaterial::Kindling);
        }
        consume_one_fire_material(world, FireMaterial::Fuel);

        // Place the lit fire on the player's current cell.
        let pos = world.player_pos();
        if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500, // a single firewood weighs ~500g; fixed for slice 1
                None,
                ItemMetadata::Lit {
                    fuel_seconds: FIRE_FUEL_SECONDS_PER_LIGHT,
                },
            ));
        }

        // Award success XP + log level-up if it crosses threshold.
        let mut skills = world.player_skills();
        let leveled = skill::award_xp(skills.get_mut(SkillKind::FireMaking), true);
        world.set_player_skills(skills);
        let value = world.player_skills().fire_making.value;
        if leveled {
            format!("fire lit (+5 Fire Making XP, leveled to {})", value)
        } else {
            format!("fire lit (+5 Fire Making XP, now {})", value)
        }
    } else {
        // Failure: award +1 XP, no fire spawned, kindling/fuel untouched.
        let mut skills = world.player_skills();
        skill::award_xp(skills.get_mut(SkillKind::FireMaking), false);
        world.set_player_skills(skills);
        format!("strike failed (+1 Fire Making XP)")
    };

    world.spend_moves(ActionId::StartFire.move_cost());
    // Phase 13b: a freshly lit fire bumps night FOV radius — recompute
    // so the extended sight shows on the same frame as the success log,
    // not after the next move.
    if success {
        world.recompute_fov();
    }
    ExecuteOutcome::Done(msg)
}

/// Phase 16 Sleep verb. Queues a single multi-turn step whose target
/// is "seconds until next dawn (06:00) or 8 game-hours, whichever is
/// shorter." Uses `queue_multi_turn_raw` so the duration is NOT
/// amplified by the need-penalty — sleep is wall-clock, not
/// effort-scaled. Need decay during the queue is real, so a player
/// who lies down hungry/cold may wake from an interrupt before dawn.
/// `complete_step(Sleep)` restores Sleep to NEED_MAX on a full sleep.
fn execute_sleep(world: &mut World) -> ExecuteOutcome {
    const HOURS_8_SECS: u64 = 8 * 3600;
    const DAY_SECS: u64 = crate::world::DAY_LENGTH_SECONDS;
    const DAWN_SECS: u64 = crate::world::DAWN_HOUR * 3600;
    let now = world.clock_seconds;
    let tod = now % DAY_SECS;
    let dawn_at = if tod < DAWN_SECS {
        now - tod + DAWN_SECS
    } else {
        now - tod + DAY_SECS + DAWN_SECS
    };
    let secs_to_dawn = (dawn_at - now).min(HOURS_8_SECS) as u32;
    world.queue_multi_turn_raw(&[(ActionId::Sleep, secs_to_dawn)]);
    ExecuteOutcome::Done(format!("sleeping ({} game-min)...", secs_to_dawn / 60))
}

/// Phase 17 fishing (slice-1 flat). Requires a pond adjacent to the
/// player. Skill-less for slice 1: a flat 20% success roll. Cost is
/// 600 game-seconds per attempt. Success drops one Fish on the
/// player's cell. Slice-2 will add a Fishing skill, gear bonuses,
/// and a stream variant; the design card calls those out.
fn is_pond(t: TerrainKind) -> bool {
    matches!(t, TerrainKind::PondWater)
}

const FISHING_SUCCESS_PCT: u8 = 20;

fn eval_fishing(world: &World) -> Availability {
    Availability::from_has(
        find_adjacent_terrain(world, is_pond).is_some(),
        world.moves_to_seconds(ActionId::Fishing.move_cost()),
        "no pond adjacent",
    )
}

fn execute_fishing(world: &mut World) -> ExecuteOutcome {
    if find_adjacent_terrain(world, is_pond).is_none() {
        return ExecuteOutcome::Done("no pond adjacent".to_string());
    }
    let roll = world.rng.d100();
    world.spend_moves(ActionId::Fishing.move_cost());
    if roll <= FISHING_SUCCESS_PCT {
        let pos = world.player_pos();
        if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            cell.items.push(ItemInstance::stack(
                ItemKind::Fish,
                1,
                ItemKind::Fish.def().default_weight_g,
                None,
                ItemMetadata::None,
            ));
        }
        ExecuteOutcome::Done("caught a fish!".to_string())
    } else {
        ExecuteOutcome::Done("nothing biting".to_string())
    }
}

/// Phase-12 instant verb. Requires a lit fire on the player's cell and
/// at least one firewood in pack-or-3x3. Consumes one firewood and adds
/// FIRE_FUEL_SECONDS_PER_FEED to the lit fire's remaining burn. No
/// skill check (this isn't a Fire Making test) and no cap on stacked
/// fuel time — stockpile-driven long fires are fine for slice 1.
fn execute_feed_fire(world: &mut World) -> ExecuteOutcome {
    // Defensive re-checks: the menu's evaluate should have gated this,
    // but a debug command could have removed state between menu-open
    // and confirm.
    if !player_cell_has_lit_fire(world) {
        return ExecuteOutcome::Done("no fire here to feed".to_string());
    }
    if !consume_one_fire_material(world, FireMaterial::Fuel) {
        return ExecuteOutcome::Done("no firewood within reach".to_string());
    }

    // Bump the lit fire's remaining burn. There can be more than one
    // Firewood on the cell (the unburnt reserve from a chop pile), so
    // explicitly target the one with Lit metadata.
    let pos = world.player_pos();
    if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
        for item in cell.items.iter_mut() {
            if let ItemMetadata::Lit { fuel_seconds } = &mut item.metadata {
                *fuel_seconds = fuel_seconds.saturating_add(FIRE_FUEL_SECONDS_PER_FEED);
                break;
            }
        }
    }

    world.spend_moves(ActionId::FeedFire.move_cost());
    ExecuteOutcome::Done("fed the fire (+30m burn)".to_string())
}

#[derive(Clone, Copy)]
enum FireMaterial {
    Tinder,
    Kindling,
    #[allow(dead_code)] // success-path fuel consumption — see below
    Fuel,
}

/// Consume one unit of the given material. Tries the player's pack
/// first (cheap to mutate), then the 3x3 cell square in deterministic
/// order. Returns true if a unit was consumed.
fn consume_one_fire_material(world: &mut World, mat: FireMaterial) -> bool {
    let kinds: &[ItemKind] = match mat {
        FireMaterial::Tinder => &[ItemKind::Twig, ItemKind::GrassBlade],
        FireMaterial::Kindling => &[ItemKind::Stick],
        FireMaterial::Fuel => &[ItemKind::Firewood],
    };

    // Pack first.
    for &kind in kinds {
        if world.player_pack_mut().take_one_from_stack(kind) {
            return true;
        }
    }

    // Then the 3x3 cell square.
    let p = world.player_pos();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let wx = (p.x + dx) as i64;
            let wy = (p.y + dy) as i64;
            let Some(cell) = world.cell_at_mut(wx, wy) else {
                continue;
            };
            let Some(idx) = cell.items.iter().position(|i| {
                kinds.contains(&i.kind)
                    && i.count > 0
                    && !matches!(i.metadata, ItemMetadata::Lit { .. })
            }) else {
                continue;
            };
            cell.items[idx].count -= 1;
            if cell.items[idx].count == 0 {
                cell.items.remove(idx);
            }
            return true;
        }
    }
    false
}

/// Called by main.rs once per `ActionId` reported in
/// `MultiTurnTickResult.completed_steps`. This is where the verb's
/// post-completion side-effects fire (consume from pack, place
/// structure, etc.). Steps that interrupt or get cancelled DO NOT
/// reach this function — partial work is forfeited per the design
/// card.
pub fn complete_step(world: &mut World, id: ActionId) -> Option<String> {
    match id {
        ActionId::PitchTent => place_pitched_from_pack(world, ItemKind::Tent, 5_000, "tent pitched"),
        ActionId::UnrollBedroll => {
            place_pitched_from_pack(world, ItemKind::Bedroll, 2_000, "bedroll unrolled")
        }
        ActionId::Sleep => {
            // Sleeping through to dawn (or 8h) restores Sleep to max.
            // Other needs ticked normally during the queue's
            // advance_time_raw — those drops are real.
            let mut needs = world.player_needs();
            needs.restore(NeedKind::Sleep, crate::needs::NEED_MAX);
            world.set_player_needs(needs);
            Some("woke rested".to_string())
        }
        ActionId::PlacePan => complete_place_pan(world),
        ActionId::PickUpPan => complete_pick_up_pan(world),
        ActionId::CookFish => complete_cook_fish(world),
        ActionId::SeasonPan => complete_season_pan(world),
        ActionId::TakeFromPan => complete_take_from_pan(world),
        // SetupCamp expands into PitchTent + UnrollBedroll steps in the
        // queue; complete_step is never called with SetupCamp itself.
        // Other future multi-turn verbs land their finish effects here.
        _ => None,
    }
}

// ---- Crafting finishers ---------------------------------------------

fn complete_place_pan(world: &mut World) -> Option<String> {
    // Need both a CookingPan to consume AND a Lit fire to replace.
    let fire = find_adjacent_item(world, |i| matches!(i.metadata, ItemMetadata::Lit { .. }));
    let Some((fx, fy, idx)) = fire else {
        return Some("the fire died before the pan could land".to_string());
    };

    // Capture the fire's remaining fuel and remove it.
    let fuel_seconds = {
        let cell = world.cell_at_mut(fx as i64, fy as i64)?;
        let lit = cell.items.remove(idx);
        match lit.metadata {
            ItemMetadata::Lit { fuel_seconds } => fuel_seconds,
            _ => return Some("internal: removed item wasn't Lit".to_string()),
        }
    };

    // Consume a CookingPan from pack first (cleanest), then radius.
    let took = consume_one_reachable(world, ItemKind::CookingPan);
    if !took {
        // Pan vanished between menu confirm and complete — put the
        // fire back so we don't silently destroy fuel.
        if let Some(cell) = world.cell_at_mut(fx as i64, fy as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds },
            ));
        }
        return Some("no pan to place".to_string());
    }

    // Drop the pan-on-fire item on the fire's old cell.
    if let Some(cell) = world.cell_at_mut(fx as i64, fy as i64) {
        cell.items.push(ItemInstance::unique(
            ItemKind::CookingPan,
            1_000,
            None,
            ItemMetadata::PannedOnFire {
                contents: PanContents::Empty,
                fuel_seconds,
            },
        ));
    }
    world.recompute_fov();
    Some("pan placed on fire".to_string())
}

fn complete_pick_up_pan(world: &mut World) -> Option<String> {
    let Some((wx, wy, idx)) = find_reachable_pan(world, |c, _f| matches!(c, PanContents::Empty))
    else {
        return Some("pan wasn't empty anymore".to_string());
    };

    let fuel_seconds = {
        let cell = world.cell_at_mut(wx as i64, wy as i64)?;
        let pan = cell.items.remove(idx);
        match pan.metadata {
            ItemMetadata::PannedOnFire { fuel_seconds, .. } => fuel_seconds,
            _ => 0,
        }
    };

    // Put the remaining fuel back on the cell as a Lit firewood (if
    // there's any fuel left) so the player can place the pan back later
    // without losing the fire.
    if fuel_seconds > 0 {
        if let Some(cell) = world.cell_at_mut(wx as i64, wy as i64) {
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds },
            ));
        }
    }

    // Pan back to pack; if full, drop on player cell.
    let pan = ItemInstance::unique(ItemKind::CookingPan, 1_000, None, ItemMetadata::None);
    let add = world.player_pack_mut().try_add(pan);
    if let Err(pan) = add {
        let pos = world.player_pos();
        if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            cell.items.push(pan);
        }
    }
    world.recompute_fov();
    Some("picked up pan".to_string())
}

fn complete_cook_fish(world: &mut World) -> Option<String> {
    if !consume_one_reachable(world, ItemKind::Fish) {
        return Some("no fish to cook".to_string());
    }
    let Some((wx, wy, idx)) =
        find_reachable_pan(world, |c, f| matches!(c, PanContents::Empty) && f > 0)
    else {
        // Refund: dropping a raw Fish at the player's feet keeps the
        // verb honest if the pan dis-empties between confirm and finish.
        let pos = world.player_pos();
        if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            cell.items.push(ItemInstance::stack(
                ItemKind::Fish,
                1,
                ItemKind::Fish.def().default_weight_g,
                None,
                ItemMetadata::None,
            ));
        }
        return Some("pan no longer ready; fish set aside".to_string());
    };
    let cell = world.cell_at_mut(wx as i64, wy as i64)?;
    if let ItemMetadata::PannedOnFire {
        ref mut contents, ..
    } = cell.items[idx].metadata
    {
        *contents = PanContents::Cooking {
            input: CookableKind::Fish,
            elapsed_secs: 0,
            seasonings: Seasonings::empty(),
        };
    }
    Some("fish cooking in pan".to_string())
}

fn complete_season_pan(world: &mut World) -> Option<String> {
    let Some((wx, wy, idx)) = find_reachable_pan(world, |c, _f| match c {
        PanContents::Cooking { seasonings, .. } => !seasonings.has(ModifierTag::Herb),
        _ => false,
    }) else {
        return Some("nothing fresh in the pan to season".to_string());
    };
    if !consume_one_reachable(world, ItemKind::Herb) {
        return Some("no herb to add".to_string());
    }
    let cell = world.cell_at_mut(wx as i64, wy as i64)?;
    if let ItemMetadata::PannedOnFire {
        ref mut contents, ..
    } = cell.items[idx].metadata
    {
        if let PanContents::Cooking {
            ref mut seasonings, ..
        } = contents
        {
            seasonings.set(ModifierTag::Herb);
        }
    }
    Some("herb added to the pan".to_string())
}

fn complete_take_from_pan(world: &mut World) -> Option<String> {
    let Some((wx, wy, idx)) = find_reachable_pan(world, |c, _f| match c {
        PanContents::Cooking {
            input,
            elapsed_secs,
            ..
        } => !matches!(cook_progress(input, elapsed_secs), CookProgress::Raw),
        _ => false,
    }) else {
        return Some("nothing done in the pan".to_string());
    };

    // Pull the cooked-out triple, then reset the pan to empty.
    let (base, seasonings, state) = {
        let cell = world.cell_at_mut(wx as i64, wy as i64)?;
        let result = if let ItemMetadata::PannedOnFire {
            ref mut contents, ..
        } = cell.items[idx].metadata
        {
            let snapshot = *contents;
            *contents = PanContents::Empty;
            snapshot
        } else {
            return Some("internal: pan metadata corrupt".to_string());
        };
        match result {
            PanContents::Cooking {
                input,
                elapsed_secs,
                seasonings,
            } => {
                let state = match cook_progress(input, elapsed_secs) {
                    CookProgress::Raw => return Some("still raw".to_string()),
                    CookProgress::Ok => CookedState::Ok,
                    CookProgress::Burnt => CookedState::Burnt,
                };
                (input, seasonings, state)
            }
            _ => return Some("pan was empty".to_string()),
        }
    };

    // Build the cooked instance and try-add to pack; if it doesn't fit,
    // drop at the player's feet.
    let cooked = ItemInstance::unique(
        ItemKind::Cooked,
        ItemKind::Cooked.def().default_weight_g,
        None,
        ItemMetadata::Cooked {
            base,
            state,
            seasonings,
        },
    );
    let label = cooked.display_label();
    let added = world.player_pack_mut().try_add(cooked);
    if let Err(cooked) = added {
        let pos = world.player_pos();
        if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            cell.items.push(cooked);
        }
        return Some(format!("{} (dropped, pack full)", label));
    }
    Some(format!("took {}", label))
}

/// Shared "consume one of `kind` from the pack, drop a Pitched
/// ItemInstance on the player's cell" finisher. Used by PitchTent and
/// UnrollBedroll; future placement verbs in phase 12+ (e.g. place pan
/// on fire) will compose differently because the source/target shape
/// differs.
///
/// Returns the success message if the consume succeeded. If the pack
/// is empty (edge case: player's tent was removed via debug mid-action),
/// returns None and the world is unchanged.
fn place_pitched_from_pack(
    world: &mut World,
    kind: ItemKind,
    weight_g: u32,
    success_msg: &str,
) -> Option<String> {
    let took = world.player_pack_mut().take_one_from_stack(kind);
    if !took {
        return None;
    }
    let pos = world.player_pos();
    if let Some(cell) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
        cell.items.push(ItemInstance::unique(
            kind,
            weight_g,
            None,
            ItemMetadata::Pitched,
        ));
    }
    Some(success_msg.to_string())
}

/// Shape shared by every "consume one source unit, restore one need"
/// verb. `WaterskinCharge` is its own variant because waterskins decrement
/// `water_uses` (not `count`) and adjust weight.
enum ConsumeFrom {
    Stack(ItemKind),
    WaterskinCharge,
}

fn consume_and_restore(
    world: &mut World,
    source: ConsumeFrom,
    need: NeedKind,
    amount: u8,
    cost_game_seconds: u32,
    success_msg: &'static str,
    empty_msg: &'static str,
) -> ExecuteOutcome {
    let consumed = match source {
        ConsumeFrom::Stack(kind) => world.player_pack_mut().take_one_from_stack(kind),
        ConsumeFrom::WaterskinCharge => world.player_pack_mut().drink_one_water_use(),
    };
    if !consumed {
        return ExecuteOutcome::Done(empty_msg.to_string());
    }
    let mut needs = world.player_needs();
    needs.restore(need, amount);
    world.set_player_needs(needs);
    world.spend_action_time(cost_game_seconds);
    ExecuteOutcome::Done(success_msg.to_string())
}

// ---- Phase 11b: terrain-dependent verbs ----

// ---- Crafting / cookware helpers -------------------------------------
//
// Recipe rows in crafting.rs are UI-only; the source of truth for "is
// this verb available right now?" lives here. Tools and ingredients are
// scanned across the player's pack AND the 3x3 cells centered on the
// player — the user's "pack + 1-cell radius" rule.

/// Find a panned-on-fire cookware reachable by the player (own cell or
/// any of the 8 neighbors), returning its `(world_x, world_y, item_idx)`.
/// `match_contents` filters by current PanContents state.
fn find_reachable_pan<F: Fn(PanContents, u32) -> bool>(
    world: &World,
    match_contents: F,
) -> Option<(i32, i32, usize)> {
    let p = world.player_pos();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let wx = p.x + dx;
            let wy = p.y + dy;
            if let Some(cell) = world.cell_at(wx as i64, wy as i64) {
                if let Some(idx) = cell.items.iter().position(|i| match i.metadata {
                    ItemMetadata::PannedOnFire {
                        contents,
                        fuel_seconds,
                    } => i.kind == ItemKind::CookingPan && match_contents(contents, fuel_seconds),
                    _ => false,
                }) {
                    return Some((wx, wy, idx));
                }
            }
        }
    }
    None
}

/// True if there's at least one cookware reachable by `pred`.
fn any_reachable_pan<F: Fn(PanContents, u32) -> bool>(world: &World, pred: F) -> bool {
    find_reachable_pan(world, pred).is_some()
}

/// Count units of `kind` reachable across pack + 1-cell radius. Used by
/// crafting recipes whose ingredient scope is pack + adjacent ground.
/// Items carrying non-None metadata (Pitched, Lit, PannedOnFire, Cooked)
/// don't satisfy the ingredient — those represent placed structures or
/// crafted outputs, not consumable stock.
fn count_reachable_kind(world: &World, kind: ItemKind) -> u32 {
    let mut total: u32 = 0;
    for item in world.player_pack().contents.iter() {
        if item.kind == kind && matches!(item.metadata, ItemMetadata::None) {
            total = total.saturating_add(item.count as u32);
        }
    }
    let p = world.player_pos();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(cell) = world.cell_at((p.x + dx) as i64, (p.y + dy) as i64) else {
                continue;
            };
            for item in cell.items.iter() {
                if item.kind == kind && matches!(item.metadata, ItemMetadata::None) {
                    total = total.saturating_add(item.count as u32);
                }
            }
        }
    }
    total
}

/// Consume one unit of `kind` from pack first, then the 3x3 cells
/// centered on the player. Returns true if a unit was actually
/// consumed. Same scope rules as `count_reachable_kind`.
fn consume_one_reachable(world: &mut World, kind: ItemKind) -> bool {
    if world.player_pack_mut().take_one_from_stack(kind) {
        return true;
    }
    let p = world.player_pos();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let wx = (p.x + dx) as i64;
            let wy = (p.y + dy) as i64;
            let Some(cell) = world.cell_at_mut(wx, wy) else {
                continue;
            };
            let Some(idx) = cell.items.iter().position(|i| {
                i.kind == kind && i.count > 0 && matches!(i.metadata, ItemMetadata::None)
            }) else {
                continue;
            };
            cell.items[idx].count -= 1;
            if cell.items[idx].count == 0 {
                cell.items.remove(idx);
            }
            return true;
        }
    }
    false
}

fn eval_place_pan(world: &World) -> Availability {
    let pack = world.player_pack();
    if !pack.has_stack(ItemKind::CookingPan) && count_reachable_kind(world, ItemKind::CookingPan) == 0
    {
        return Availability::Unavailable {
            reason: "no cooking pan",
        };
    }
    if find_adjacent_item(world, |i| matches!(i.metadata, ItemMetadata::Lit { .. })).is_none() {
        return Availability::Unavailable {
            reason: "no lit fire adjacent",
        };
    }
    Availability::Available {
        cost_game_seconds: world.moves_to_seconds(ActionId::PlacePan.move_cost()),
    }
}

fn eval_pick_up_pan(world: &World) -> Availability {
    if any_reachable_pan(world, |c, _f| matches!(c, PanContents::Empty)) {
        Availability::Available {
            cost_game_seconds: world.moves_to_seconds(ActionId::PickUpPan.move_cost()),
        }
    } else {
        Availability::Unavailable {
            reason: "no empty pan-on-fire to lift",
        }
    }
}

fn eval_cook_fish(world: &World) -> Availability {
    if count_reachable_kind(world, ItemKind::Fish) == 0 {
        return Availability::Unavailable {
            reason: "no fish in reach",
        };
    }
    if !any_reachable_pan(world, |c, f| matches!(c, PanContents::Empty) && f > 0) {
        return Availability::Unavailable {
            reason: "need empty pan-on-fire",
        };
    }
    Availability::Available {
        cost_game_seconds: world.moves_to_seconds(ActionId::CookFish.move_cost()),
    }
}

fn eval_season_pan(world: &World) -> Availability {
    if count_reachable_kind(world, ItemKind::Herb) == 0 {
        return Availability::Unavailable {
            reason: "no herb in reach",
        };
    }
    let has_target = any_reachable_pan(world, |c, _f| match c {
        PanContents::Cooking { seasonings, .. } => !seasonings.has(ModifierTag::Herb),
        _ => false,
    });
    if !has_target {
        return Availability::Unavailable {
            reason: "no fresh cook to season",
        };
    }
    Availability::Available {
        cost_game_seconds: world.moves_to_seconds(ActionId::SeasonPan.move_cost()),
    }
}

fn eval_take_from_pan(world: &World) -> Availability {
    let ready = any_reachable_pan(world, |c, _f| match c {
        PanContents::Cooking {
            input,
            elapsed_secs,
            ..
        } => !matches!(cook_progress(input, elapsed_secs), CookProgress::Raw),
        _ => false,
    });
    if ready {
        Availability::Available {
            cost_game_seconds: world.moves_to_seconds(ActionId::TakeFromPan.move_cost()),
        }
    } else {
        Availability::Unavailable {
            reason: "nothing done in the pan",
        }
    }
}

/// Find the first adjacent cell (8-neighborhood, excluding the player's
/// own) whose terrain satisfies `pred`. Returns the world coords.
fn find_adjacent_terrain<F: Fn(TerrainKind) -> bool>(
    world: &World,
    pred: F,
) -> Option<(i32, i32)> {
    let p = world.player_pos();
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let wx = p.x + dx;
            let wy = p.y + dy;
            if pred(world.tile_at(wx as i64, wy as i64)) {
                return Some((wx, wy));
            }
        }
    }
    None
}

/// Find an adjacent cell holding an item that satisfies `pred`. Returns
/// the cell coords + the item index within that cell's items vec.
fn find_adjacent_item<F: Fn(&ItemInstance) -> bool>(
    world: &World,
    pred: F,
) -> Option<(i32, i32, usize)> {
    let p = world.player_pos();
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let wx = p.x + dx;
            let wy = p.y + dy;
            if let Some(cell) = world.cell_at(wx as i64, wy as i64) {
                if let Some(idx) = cell.items.iter().position(|i| pred(i)) {
                    return Some((wx, wy, idx));
                }
            }
        }
    }
    None
}

fn is_water(t: TerrainKind) -> bool {
    matches!(t, TerrainKind::StreamWater | TerrainKind::PondWater)
}

fn eval_drink_from_stream(world: &World) -> Availability {
    Availability::from_has(
        find_adjacent_terrain(world, is_water).is_some(),
        world.moves_to_seconds(ActionId::DrinkFromStream.move_cost()),
        "no water adjacent",
    )
}

fn execute_drink_from_stream(world: &mut World) -> ExecuteOutcome {
    if find_adjacent_terrain(world, is_water).is_none() {
        return ExecuteOutcome::Done("no water adjacent".to_string());
    }
    let mut needs = world.player_needs();
    needs.restore(NeedKind::Thirst, 20);
    world.set_player_needs(needs);
    world.spend_moves(ActionId::DrinkFromStream.move_cost());
    ExecuteOutcome::Done("drank from water (+20 thirst)".to_string())
}

fn eval_fill_waterskin(world: &World) -> Availability {
    if find_adjacent_terrain(world, is_water).is_none() {
        return Availability::Unavailable {
            reason: "no water adjacent",
        };
    }
    let has_partial = world.player_pack().contents.iter().any(|i| {
        i.kind == ItemKind::Waterskin
            && matches!(i.metadata, ItemMetadata::Waterskin { water_uses } if water_uses < 4)
    });
    Availability::from_has(
        has_partial,
        world.moves_to_seconds(ActionId::FillWaterskin.move_cost()),
        "waterskins already full",
    )
}

fn execute_fill_waterskin(world: &mut World) -> ExecuteOutcome {
    if find_adjacent_terrain(world, is_water).is_none() {
        return ExecuteOutcome::Done("no water adjacent".to_string());
    }
    let filled = {
        let mut pack = world.player_pack_mut();
        let Some(idx) = pack.contents.iter().position(|i| {
            i.kind == ItemKind::Waterskin
                && matches!(i.metadata, ItemMetadata::Waterskin { water_uses } if water_uses < 4)
        }) else {
            return ExecuteOutcome::Done("no empty waterskin".to_string());
        };
        pack.contents[idx].metadata = ItemMetadata::Waterskin { water_uses: 4 };
        pack.contents[idx].weight_g_each = 1_200;
        true
    };
    if filled {
        world.spend_moves(ActionId::FillWaterskin.move_cost());
        ExecuteOutcome::Done("waterskin filled (4 uses)".to_string())
    } else {
        ExecuteOutcome::Done("no empty waterskin".to_string())
    }
}

fn eval_chop_tree(world: &World) -> Availability {
    if !world.player_pack().has_stack(ItemKind::Axe) {
        return Availability::Unavailable {
            reason: "need an axe",
        };
    }
    Availability::from_has(
        find_adjacent_terrain(world, |t| t == TerrainKind::TreeTrunk).is_some(),
        world.moves_to_seconds(ActionId::ChopTree.move_cost()),
        "no tree adjacent",
    )
}

fn execute_chop_tree(world: &mut World) -> ExecuteOutcome {
    if !world.player_pack().has_stack(ItemKind::Axe) {
        return ExecuteOutcome::Done("need an axe".to_string());
    }
    let Some((wx, wy)) = find_adjacent_terrain(world, |t| t == TerrainKind::TreeTrunk) else {
        return ExecuteOutcome::Done("no tree to chop".to_string());
    };

    // Capture the species before clearing it so the sapling that
    // replaces this cell is the same species — the regrowth loop
    // expects "chop a Hazel, get a Hazel sapling, eventually grow a
    // Hazel back."
    let species = world
        .cell_at(wx as i64, wy as i64)
        .and_then(|c| c.tree_species);
    let planted_day = world.calendar_day;

    // Fell: terrain converts to BareDirt (Phase D: a chopped cell is a
    // stump-and-disturbed-dirt patch, not pristine grass); the species
    // tag clears; a sapling decoration takes over. FOV recomputes —
    // the tree no longer blocks sight, but the sapling doesn't block
    // either, so the new line-of-sight opens up immediately.
    world.set_terrain_at(wx as i64, wy as i64, TerrainKind::BareDirt);
    world.set_tree_species_at(wx as i64, wy as i64, None);
    if let Some(sp) = species {
        world.set_decoration_at(
            wx as i64,
            wy as i64,
            crate::flora::Decoration::Sapling {
                species: sp,
                planted_day,
            },
        );
    }
    let span = (FELLED_FIREWOOD_MAX - FELLED_FIREWOOD_MIN + 1) as u32;
    let count = FELLED_FIREWOOD_MIN as u16 + (world.rng.next_u32() % span) as u16;
    if let Some(cell) = world.cell_at_mut(wx as i64, wy as i64) {
        cell.items.push(ItemInstance::stack(
            ItemKind::Firewood,
            count,
            500,
            None,
            ItemMetadata::None,
        ));
    }
    world.spend_moves(ActionId::ChopTree.move_cost());
    world.recompute_fov();
    ExecuteOutcome::Done(format!("tree felled (+{} firewood on the ground)", count))
}

fn eval_pick_herb(world: &World) -> Availability {
    if !world.player_pack().has_stack(ItemKind::Knife) {
        return Availability::Unavailable {
            reason: "need a knife",
        };
    }
    Availability::from_has(
        find_adjacent_item(world, |i| i.kind == ItemKind::Herb).is_some(),
        world.moves_to_seconds(ActionId::PickHerb.move_cost()),
        "no herb adjacent",
    )
}

fn execute_pick_herb(world: &mut World) -> ExecuteOutcome {
    if !world.player_pack().has_stack(ItemKind::Knife) {
        return ExecuteOutcome::Done("need a knife".to_string());
    }
    let Some((wx, wy, idx)) = find_adjacent_item(world, |i| i.kind == ItemKind::Herb) else {
        return ExecuteOutcome::Done("no herb adjacent".to_string());
    };
    // Take the herb out of the cell.
    let herb = if let Some(cell) = world.cell_at_mut(wx as i64, wy as i64) {
        if idx < cell.items.len() {
            cell.items.remove(idx)
        } else {
            return ExecuteOutcome::Done("herb gone".to_string());
        }
    } else {
        return ExecuteOutcome::Done("cell gone".to_string());
    };
    // Pack-add. If it doesn't fit, put it back. Bind the result so the
    // RefMut from player_pack_mut drops before the next world borrow.
    let add_result = world.player_pack_mut().try_add(herb);
    match add_result {
        Ok(()) => {
            world.spend_moves(ActionId::PickHerb.move_cost());
            ExecuteOutcome::Done("picked herb".to_string())
        }
        Err(returned) => {
            if let Some(cell) = world.cell_at_mut(wx as i64, wy as i64) {
                cell.items.push(returned);
            }
            ExecuteOutcome::Done("pack too full for herb".to_string())
        }
    }
}

// ---- Phase D: undergrowth harvest verbs ----

/// Tool requirement for a harvest verb. Some verbs need a knife (most),
/// others need an axe (gorse is woody enough to require an axe).
#[derive(Clone, Copy, PartialEq, Eq)]
enum HarvestTool {
    Knife,
    Axe,
}
const HARVEST_REQUIRES_KNIFE: HarvestTool = HarvestTool::Knife;
const HARVEST_REQUIRES_AXE: HarvestTool = HarvestTool::Axe;

fn find_adjacent_decoration<F: Fn(crate::flora::Decoration) -> bool>(
    world: &World,
    pred: F,
) -> Option<(i32, i32)> {
    let p = world.player_pos();
    // Include the player's own cell — harvesting moss you're standing
    // on is valid even though "adjacent" implies 8-neighborhood. The
    // 3x3 catchment matches the existing fire-materials search radius.
    for dy in -1..=1 {
        for dx in -1..=1 {
            let wx = p.x + dx;
            let wy = p.y + dy;
            if let Some(cell) = world.cell_at(wx as i64, wy as i64) {
                if pred(cell.decoration) {
                    return Some((wx, wy));
                }
            }
        }
    }
    None
}

fn eval_harvest_decoration<F: Fn(crate::flora::Decoration) -> bool>(
    world: &World,
    pred: F,
    id: ActionId,
    tool: HarvestTool,
    missing_reason: &'static str,
) -> Availability {
    let pack = world.player_pack();
    let has_tool = match tool {
        HarvestTool::Knife => pack.has_stack(ItemKind::Knife),
        HarvestTool::Axe => pack.has_stack(ItemKind::Axe),
    };
    if !has_tool {
        return Availability::Unavailable {
            reason: match tool {
                HarvestTool::Knife => "need a knife",
                HarvestTool::Axe => "need an axe",
            },
        };
    }
    Availability::from_has(
        find_adjacent_decoration(world, pred).is_some(),
        world.moves_to_seconds(id.move_cost()),
        missing_reason,
    )
}

/// Helper: try to add a stack of `count` items into the pack. If the
/// stack doesn't fit, the items are dropped on the cell at `(wx, wy)`
/// instead — better to leave them on the ground than vanish them.
fn deliver_stack(world: &mut World, wx: i32, wy: i32, kind: ItemKind, count: u16) {
    if count == 0 {
        return;
    }
    let def = kind.def();
    let stack = ItemInstance::stack(
        kind,
        count,
        def.default_weight_g,
        None,
        ItemMetadata::None,
    );
    let add_result = world.player_pack_mut().try_add(stack);
    if let Err(returned) = add_result {
        if let Some(cell) = world.cell_at_mut(wx as i64, wy as i64) {
            cell.items.push(returned);
        }
    }
}

/// Award Foraging XP for a successful harvest. Mirrors the
/// FireMaking award path used by `execute_start_fire`. Daily cap is
/// honored; level-up logs but doesn't pop a UI yet.
fn award_foraging_xp(world: &mut World, success: bool) {
    let mut skills = world.player_skills();
    let _leveled = crate::skill::award_xp(skills.get_mut(crate::skill::SkillKind::Foraging), success);
    world.set_player_skills(skills);
}

fn execute_harvest_moss(world: &mut World) -> ExecuteOutcome {
    if !world.player_pack().has_stack(ItemKind::Knife) {
        return ExecuteOutcome::Done("need a knife".to_string());
    }
    let Some((wx, wy)) = find_adjacent_decoration(world, |d| {
        matches!(d, crate::flora::Decoration::Moss)
    }) else {
        return ExecuteOutcome::Done("no moss adjacent".to_string());
    };
    let count = 1 + (world.rng.next_u32() % 2) as u16; // 1..=2
    world.set_decoration_at(wx as i64, wy as i64, crate::flora::Decoration::None);
    deliver_stack(world, wx, wy, ItemKind::Moss, count);
    world.spend_moves(ActionId::HarvestMoss.move_cost());
    award_foraging_xp(world, true);
    ExecuteOutcome::Done(format!("harvested {} moss", count))
}

fn execute_cut_fern(world: &mut World) -> ExecuteOutcome {
    if !world.player_pack().has_stack(ItemKind::Knife) {
        return ExecuteOutcome::Done("need a knife".to_string());
    }
    let Some((wx, wy)) = find_adjacent_decoration(world, |d| {
        matches!(d, crate::flora::Decoration::Fern { .. })
    }) else {
        return ExecuteOutcome::Done("no fern adjacent".to_string());
    };
    world.set_decoration_at(wx as i64, wy as i64, crate::flora::Decoration::None);
    deliver_stack(world, wx, wy, ItemKind::FernFrond, 2);
    world.spend_moves(ActionId::CutFern.move_cost());
    // CutFern doesn't train Foraging — it's a brute-force cut, no skill.
    ExecuteOutcome::Done("cut fern (+2 fronds)".to_string())
}

fn execute_dig_fern(world: &mut World) -> ExecuteOutcome {
    if !world.player_pack().has_stack(ItemKind::Knife) {
        return ExecuteOutcome::Done("need a knife".to_string());
    }
    let Some((wx, wy)) = find_adjacent_decoration(world, |d| {
        matches!(d, crate::flora::Decoration::Fern { .. })
    }) else {
        return ExecuteOutcome::Done("no fern adjacent".to_string());
    };
    let frond_count = 1 + (world.rng.next_u32() % 2) as u16; // 1..=2
    world.set_decoration_at(wx as i64, wy as i64, crate::flora::Decoration::None);
    deliver_stack(world, wx, wy, ItemKind::FernRoot, 1);
    deliver_stack(world, wx, wy, ItemKind::FernFrond, frond_count);
    world.spend_moves(ActionId::DigFern.move_cost());
    award_foraging_xp(world, true);
    ExecuteOutcome::Done(format!("dug fern (+1 root, +{} fronds)", frond_count))
}

fn execute_cut_gorse(world: &mut World) -> ExecuteOutcome {
    if !world.player_pack().has_stack(ItemKind::Axe) {
        return ExecuteOutcome::Done("need an axe".to_string());
    }
    let Some((wx, wy)) = find_adjacent_decoration(world, |d| {
        matches!(d, crate::flora::Decoration::Gorse { .. })
    }) else {
        return ExecuteOutcome::Done("no gorse adjacent".to_string());
    };
    world.set_decoration_at(wx as i64, wy as i64, crate::flora::Decoration::None);
    deliver_stack(world, wx, wy, ItemKind::GorseFaggot, 1);
    world.spend_moves(ActionId::CutGorse.move_cost());
    world.recompute_fov();
    ExecuteOutcome::Done("cut gorse (+1 faggot)".to_string())
}

fn execute_cut_bracken(world: &mut World) -> ExecuteOutcome {
    if !world.player_pack().has_stack(ItemKind::Knife) {
        return ExecuteOutcome::Done("need a knife".to_string());
    }
    let Some((wx, wy)) = find_adjacent_decoration(world, |d| {
        matches!(d, crate::flora::Decoration::Bracken { .. })
    }) else {
        return ExecuteOutcome::Done("no bracken adjacent".to_string());
    };
    world.set_decoration_at(wx as i64, wy as i64, crate::flora::Decoration::None);
    deliver_stack(world, wx, wy, ItemKind::BrackenStraw, 3);
    world.spend_moves(ActionId::CutBracken.move_cost());
    ExecuteOutcome::Done("cut bracken (+3 straw)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{starting_pack, ItemInstance, ItemKind, ItemMetadata};
    use crate::world::{Position, World, CHUNK_H, CHUNK_W};

    #[test]
    fn catalog_listed_in_render_order() {
        assert_eq!(ALL_ACTIONS.first().map(|a| a.id), Some(ActionId::Pickup));
        // Reaches each declared verb at least once; guards against
        // dropping a Variant from ALL_ACTIONS when ActionId grows.
        let count = ALL_ACTIONS.len();
        assert!(count >= 13);
    }

    #[test]
    fn cut_fern_clears_decoration_and_drops_fronds() {
        use crate::flora::{Decoration, PlantState};
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let pos = world.player_pos();
        // Drop a fern on the cell east of the player. Player has a
        // knife in the starting pack.
        let east_x = pos.x + 1;
        let east_y = pos.y;
        world.set_decoration_at(
            east_x as i64,
            east_y as i64,
            Decoration::Fern {
                state: PlantState::Mature,
            },
        );
        assert!(
            matches!(evaluate(&world, ActionId::CutFern), Availability::Available { .. }),
            "CutFern should be available with knife + adjacent fern"
        );
        let outcome = execute(&mut world, ActionId::CutFern);
        match outcome {
            ExecuteOutcome::Done(ref msg) => assert!(msg.contains("fern"), "got {:?}", msg),
            other => panic!("expected Done, got {:?}", other),
        }
        // Decoration cleared.
        assert!(matches!(
            world.cell_at(east_x as i64, east_y as i64).map(|c| c.decoration),
            Some(Decoration::None)
        ));
        // Pack got 2 FernFronds.
        let pack = world.player_pack();
        let frond_count: u16 = pack
            .contents
            .iter()
            .filter(|i| i.kind == ItemKind::FernFrond)
            .map(|i| i.count)
            .sum();
        assert_eq!(frond_count, 2);
    }

    #[test]
    fn cut_gorse_requires_axe_not_knife() {
        use crate::flora::{Decoration, PlantState};
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let pos = world.player_pos();
        world.set_decoration_at(
            (pos.x + 1) as i64,
            pos.y as i64,
            Decoration::Gorse {
                state: PlantState::Mature,
            },
        );
        // Starting pack has knife + axe. Remove axe and re-check.
        world.player_pack_mut().contents.retain(|i| i.kind != ItemKind::Axe);
        assert!(
            matches!(
                evaluate(&world, ActionId::CutGorse),
                Availability::Unavailable { reason: "need an axe" }
            ),
            "CutGorse must demand an axe even with a knife"
        );
    }

    #[test]
    fn chop_tree_spawns_a_sapling_of_the_same_species() {
        use crate::flora::{Decoration, TreeSpecies};
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Force an Oak tree east of the player so we know the species
        // we're chopping.
        let pos = world.player_pos();
        let east_x = pos.x + 1;
        let east_y = pos.y;
        world.set_terrain_at(east_x as i64, east_y as i64, TerrainKind::TreeTrunk);
        world.set_tree_species_at(east_x as i64, east_y as i64, Some(TreeSpecies::Oak));
        // Starting pack has an axe.
        let outcome = execute(&mut world, ActionId::ChopTree);
        match outcome {
            ExecuteOutcome::Done(_) => {}
            other => panic!("expected Done, got {:?}", other),
        }
        assert_eq!(world.tile_at(east_x as i64, east_y as i64), TerrainKind::BareDirt);
        let cell = world.cell_at(east_x as i64, east_y as i64).expect("cell");
        assert!(matches!(
            cell.decoration,
            Decoration::Sapling { species: TreeSpecies::Oak, .. }
        ));
        assert_eq!(cell.tree_species, None);
    }

    #[test]
    fn pickup_available_on_seeded_debris_cell() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Place a deterministic stack on the spawn cell so the test
        // doesn't depend on chunkgen rolling debris at any specific
        // coord (which the debris-probability tweaks can shift).
        let pos = world.player_pos();
        if let Some(c) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            c.items.clear();
            c.items.push(ItemInstance::stack(
                ItemKind::Twig,
                3,
                5,
                None,
                ItemMetadata::None,
            ));
        }
        let avail = evaluate(&world, ActionId::Pickup);
        match avail {
            Availability::Available { cost_game_seconds } => {
                assert_eq!(
                    cost_game_seconds,
                    world.moves_to_seconds(ActionId::Pickup.move_cost())
                );
            }
            other => panic!("expected Available, got {:?}", other),
        }
    }

    #[test]
    fn pickup_unavailable_on_empty_cell() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Force the spawn cell empty so the assertion isolates the
        // empty-cell case from whatever chunkgen rolled.
        if let Some(c) = world.cell_at_mut(20, 15) {
            c.items.clear();
        }
        let avail = evaluate(&world, ActionId::Pickup);
        assert!(matches!(avail, Availability::Unavailable { reason: "nothing here" }));
    }

    #[test]
    fn pickup_unavailable_when_pack_full() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Replace the east cell with a single 10kg boulder, no room.
        let pos = world.player_pos();
        if let Some(c) = world.cell_at_mut((pos.x + 1) as i64, pos.y as i64) {
            c.items.clear();
            c.items.push(ItemInstance::stack(
                ItemKind::Stone,
                1,
                10_000,
                None,
                ItemMetadata::None,
            ));
        }
        world.try_move_player(1, 0);
        let avail = evaluate(&world, ActionId::Pickup);
        assert!(matches!(avail, Availability::Unavailable { reason: "pack too full" }));
    }

    #[test]
    fn every_verb_evaluates_to_a_real_reason_or_available() {
        // Phase 8 left behind a "stubs report phase XX" assertion that
        // outlived its purpose — every verb is now live. The remaining
        // useful coverage: each verb's evaluate() must return SOMETHING
        // (Available or a non-empty Unavailable reason) without
        // panicking. Catches dispatch table holes when a new ActionId
        // is added but its evaluate arm is forgotten.
        let world = World::new(CHUNK_W, CHUNK_H);
        for action in ALL_ACTIONS {
            match evaluate(&world, action.id) {
                Availability::Available { .. } => {}
                Availability::Unavailable { reason } => {
                    assert!(
                        !reason.is_empty(),
                        "{:?} returned an empty unavail reason",
                        action.id
                    );
                }
            }
        }
        // Tickle starting_pack + Position so they don't get pruned in
        // tests-only-builds; the resolver depends on World state.
        let _ = (Position { x: 0, y: 0 }, starting_pack());
    }

    #[test]
    fn eat_ration_available_at_spawn() {
        let world = World::new(CHUNK_W, CHUNK_H);
        match evaluate(&world, ActionId::EatRation) {
            Availability::Available { cost_game_seconds } => {
                assert_eq!(
                    cost_game_seconds,
                    world.moves_to_seconds(ActionId::EatRation.move_cost())
                );
            }
            other => panic!("expected Available, got {:?}", other),
        }
    }

    #[test]
    fn eat_ration_decrements_pack_and_restores_hunger() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Drop hunger first so the restore is observable.
        let mut n = world.player_needs();
        n.hunger = 50;
        world.set_player_needs(n);

        let rations_before = world
            .player_pack()
            .contents
            .iter()
            .find(|i| i.kind == ItemKind::Ration)
            .map(|i| i.count)
            .unwrap_or(0);

        let outcome = execute(&mut world, ActionId::EatRation);
        assert!(matches!(outcome, ExecuteOutcome::Done(_)));

        let rations_after = world
            .player_pack()
            .contents
            .iter()
            .find(|i| i.kind == ItemKind::Ration)
            .map(|i| i.count)
            .unwrap_or(0);
        assert_eq!(rations_after, rations_before - 1);
        assert_eq!(world.player_needs().hunger, 75);
    }

    #[test]
    fn drink_waterskin_uses_charge_and_restores_thirst() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Drop thirst so we can see the change.
        let mut n = world.player_needs();
        n.thirst = 50;
        world.set_player_needs(n);

        let outcome = execute(&mut world, ActionId::DrinkWaterskin);
        assert!(matches!(outcome, ExecuteOutcome::Done(_)));
        // After drinking, thirst is +20.
        assert_eq!(world.player_needs().thirst, 70);

        // First waterskin in starting_pack now has 3 water_uses and weighs
        // 950g instead of 1200g.
        let pack = world.player_pack();
        let drained = pack
            .contents
            .iter()
            .find(|i| i.kind == ItemKind::Waterskin)
            .expect("waterskin in starting pack");
        match drained.metadata {
            ItemMetadata::Waterskin { water_uses } => assert_eq!(water_uses, 3),
            other => panic!("expected Waterskin metadata, got {:?}", other),
        }
        assert_eq!(drained.weight_g_each, 950);
    }

    #[test]
    fn complete_pitch_tent_places_pitched_item_on_player_cell() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let pos = world.player_pos();
        // Force the spawn cell empty so the assertion isolates the new
        // pitched item (the spawn cell has no debris by default but be
        // explicit).
        let pre_count = world
            .cell_at(pos.x as i64, pos.y as i64)
            .map(|c| c.items.len())
            .unwrap_or(0);

        let msg = complete_step(&mut world, ActionId::PitchTent);
        assert_eq!(msg.as_deref(), Some("tent pitched"));

        let cell = world
            .cell_at(pos.x as i64, pos.y as i64)
            .expect("player's cell exists");
        assert_eq!(cell.items.len(), pre_count + 1);
        let pitched = cell
            .items
            .iter()
            .find(|i| i.kind == ItemKind::Tent)
            .expect("tent on cell");
        assert!(matches!(pitched.metadata, ItemMetadata::Pitched));

        // Pack lost one Tent in exchange.
        assert!(!world.player_pack().has_stack(ItemKind::Tent));
    }

    #[test]
    fn complete_pitch_tent_noop_when_pack_empty() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Drain all tents first.
        while world.player_pack_mut().take_one_from_stack(ItemKind::Tent) {}
        let pos = world.player_pos();
        let pre_items = world
            .cell_at(pos.x as i64, pos.y as i64)
            .map(|c| c.items.len())
            .unwrap_or(0);

        let msg = complete_step(&mut world, ActionId::PitchTent);
        assert!(msg.is_none(), "no pack tent -> no placement");
        let post_items = world
            .cell_at(pos.x as i64, pos.y as i64)
            .map(|c| c.items.len())
            .unwrap_or(0);
        assert_eq!(pre_items, post_items);
    }

    #[test]
    fn start_fire_unavailable_without_materials() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Clear the 3x3 around spawn so the test isolates the missing-
        // materials case from whatever chunkgen rolled.
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(c) = world.cell_at_mut((20 + dx) as i64, (15 + dy) as i64) {
                    c.items.clear();
                }
            }
        }
        match eval_start_fire(&world) {
            Availability::Unavailable { reason } => {
                // Should be the first-failing material check.
                assert!(!reason.is_empty());
            }
            other => panic!("expected Unavailable, got {:?}", other),
        }
    }

    #[test]
    fn start_fire_available_with_full_materials_and_flint() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Stuff the player's cell with the full requirement set.
        let pos = world.player_pos();
        if let Some(c) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            c.items.clear();
            c.items.push(ItemInstance::stack(
                ItemKind::Twig,
                3,
                5,
                None,
                ItemMetadata::None,
            ));
            c.items.push(ItemInstance::stack(
                ItemKind::Stick,
                5,
                50,
                None,
                ItemMetadata::None,
            ));
            c.items.push(ItemInstance::stack(
                ItemKind::Firewood,
                3,
                500,
                None,
                ItemMetadata::None,
            ));
        }
        match eval_start_fire(&world) {
            Availability::Available { cost_game_seconds } => {
                assert_eq!(
                    cost_game_seconds,
                    world.moves_to_seconds(ActionId::StartFire.move_cost())
                );
            }
            other => panic!("expected Available, got {:?}", other),
        }
    }

    #[test]
    fn execute_start_fire_success_places_lit_fire_and_awards_xp() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.rng = crate::skill::Rng::from_state(1);

        // Clear the entire 3x3 around the player so chunkgen rolls
        // don't add extra materials and confuse the post-execute
        // counts. Then fill the spawn cell with exactly the kit.
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(c) = world.cell_at_mut((20 + dx) as i64, (15 + dy) as i64) {
                    c.items.clear();
                }
            }
        }
        let pos = world.player_pos();
        if let Some(c) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            c.items.push(ItemInstance::stack(
                ItemKind::Twig,
                3,
                5,
                None,
                ItemMetadata::None,
            ));
            c.items.push(ItemInstance::stack(
                ItemKind::Stick,
                5,
                50,
                None,
                ItemMetadata::None,
            ));
            c.items.push(ItemInstance::stack(
                ItemKind::Firewood,
                3,
                500,
                None,
                ItemMetadata::None,
            ));
        }

        let xp_before = world.player_skills().fire_making.daily_xp;
        let outcome = execute(&mut world, ActionId::StartFire);
        assert!(matches!(outcome, ExecuteOutcome::Done(_)));

        // XP went up regardless of success/failure outcome (we don't
        // know which the RNG produced; assert >0).
        let xp_after = world.player_skills().fire_making.daily_xp;
        assert!(
            xp_after > xp_before,
            "xp must rise on any attempt: {} -> {}",
            xp_before,
            xp_after
        );

        // Tinder always consumed; pre = 3 twigs, post should be <= 2.
        let cell = world.cell_at(pos.x as i64, pos.y as i64).unwrap();
        let post_twigs: u32 = cell
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Twig)
            .map(|i| i.count as u32)
            .sum();
        assert!(post_twigs < 3, "at least 1 twig consumed");
    }

    #[test]
    fn feed_fire_consumes_one_firewood_and_extends_burn() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Clean the 3x3 around the player so chunkgen debris doesn't
        // bleed firewood into the pack/cells scan.
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(c) = world.cell_at_mut((20 + dx) as i64, (15 + dy) as i64) {
                    c.items.clear();
                }
            }
        }
        // Place a lit fire on the player's cell plus a feedable stack of
        // 2 firewood on top.
        let pos = world.player_pos();
        if let Some(c) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            c.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds: 600 },
            ));
            c.items.push(ItemInstance::stack(
                ItemKind::Firewood,
                2,
                500,
                None,
                ItemMetadata::None,
            ));
        }

        // Evaluator should report available.
        match evaluate(&world, ActionId::FeedFire) {
            Availability::Available { cost_game_seconds } => {
                assert_eq!(
                    cost_game_seconds,
                    world.moves_to_seconds(ActionId::FeedFire.move_cost())
                );
            }
            other => panic!("expected Available, got {:?}", other),
        }

        let outcome = execute(&mut world, ActionId::FeedFire);
        assert!(matches!(outcome, ExecuteOutcome::Done(_)));

        let cell = world.cell_at(pos.x as i64, pos.y as i64).unwrap();
        // Lit fire's fuel_seconds bumped by 1800.
        let lit_secs = cell
            .items
            .iter()
            .find_map(|i| match i.metadata {
                ItemMetadata::Lit { fuel_seconds } => Some(fuel_seconds),
                _ => None,
            })
            .expect("lit fire still present");
        // The world's per-second fire tick fires during spend_moves,
        // so the lit fire also burns down by that wall-clock during the
        // action. Net: starting 600s + FIRE_FUEL_SECONDS_PER_FEED bump
        // - elapsed-during-action burn.
        let elapsed = world.moves_to_seconds(ActionId::FeedFire.move_cost());
        assert_eq!(
            lit_secs,
            600 + FIRE_FUEL_SECONDS_PER_FEED - elapsed
        );
        // Reserve firewood stack went 2 -> 1.
        let reserve: u32 = cell
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Firewood && !matches!(i.metadata, ItemMetadata::Lit { .. }))
            .map(|i| i.count as u32)
            .sum();
        assert_eq!(reserve, 1);
    }

    #[test]
    fn feed_fire_unavailable_without_lit_fire_on_cell() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Even with firewood handy, missing the lit fire blocks the verb.
        let pos = world.player_pos();
        if let Some(c) = world.cell_at_mut(pos.x as i64, pos.y as i64) {
            c.items.clear();
            c.items.push(ItemInstance::stack(
                ItemKind::Firewood,
                3,
                500,
                None,
                ItemMetadata::None,
            ));
        }
        match evaluate(&world, ActionId::FeedFire) {
            Availability::Unavailable { reason } => {
                assert_eq!(reason, "no fire here to feed");
            }
            other => panic!("expected Unavailable, got {:?}", other),
        }
    }

    #[test]
    fn sleep_queues_duration_to_next_dawn_or_8h_min() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // 22:00 of day 1 (8h to next dawn). Sleep should target exactly
        // 8h since that's both the cap AND the to-dawn time.
        world.clock_seconds = 22 * 3600;
        let outcome = execute(&mut world, ActionId::Sleep);
        assert!(matches!(outcome, ExecuteOutcome::Done(_)));
        let queued = world
            .active_action
            .as_ref()
            .expect("sleep must queue a multi-turn step");
        let step = queued.steps.front().expect("at least one step");
        assert_eq!(step.id, ActionId::Sleep);
        assert_eq!(step.target_secs, 8 * 3600, "22:00 -> dawn = 8h");

        // 23:00 same day: only 7h to dawn. Target must be 7h (< 8h cap).
        world.cancel_multi_turn();
        world.clock_seconds = 23 * 3600;
        let _ = execute(&mut world, ActionId::Sleep);
        let step = world.active_action.as_ref().unwrap().steps.front().unwrap();
        assert_eq!(step.target_secs, 7 * 3600);

        // Noon: 18h to next dawn, capped to 8h.
        world.cancel_multi_turn();
        world.clock_seconds = 12 * 3600;
        let _ = execute(&mut world, ActionId::Sleep);
        let step = world.active_action.as_ref().unwrap().steps.front().unwrap();
        assert_eq!(step.target_secs, 8 * 3600, "noon caps at 8h");
    }

    #[test]
    fn fishing_unavailable_without_adjacent_pond() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Spawn is at (20, 15); the chunkgen pond is at (28, 22). No
        // pond should be within the 3x3 around spawn.
        match evaluate(&world, ActionId::Fishing) {
            Availability::Unavailable { reason } => assert_eq!(reason, "no pond adjacent"),
            other => panic!("expected Unavailable, got {:?}", other),
        }
    }

    #[test]
    fn fishing_with_pond_adjacent_advances_clock_and_sometimes_drops_fish() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let pos = world.player_pos();
        // Inject a pond cell one east of the player so eval/execute
        // both clear their "pond adjacent" gate.
        world.set_terrain_at((pos.x + 1) as i64, pos.y as i64, TerrainKind::PondWater);

        let clock_before = world.clock_seconds;
        let outcome = execute(&mut world, ActionId::Fishing);
        let msg = match outcome {
            ExecuteOutcome::Done(m) => m,
            other => panic!("expected Done, got {:?}", other),
        };
        // Clock must advance by Fishing's wall-clock cost (600s at
        // baseline speed) regardless of catch outcome — fishing takes
        // the same time whether you succeed.
        let expected = world.moves_to_seconds(ActionId::Fishing.move_cost()) as u64;
        assert!(world.clock_seconds >= clock_before + expected);
        // Either outcome message is acceptable; one of them must occur.
        assert!(
            msg == "caught a fish!" || msg == "nothing biting",
            "got: {}",
            msg
        );
        // If success, a Fish stack exists on the player's cell.
        let caught = world
            .cell_at(pos.x as i64, pos.y as i64)
            .map(|c| c.items.iter().any(|i| i.kind == ItemKind::Fish))
            .unwrap_or(false);
        assert_eq!(
            caught,
            msg == "caught a fish!",
            "fish presence must match the outcome msg"
        );
    }

    #[test]
    fn complete_sleep_restores_sleep_need_to_max() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let mut n = world.player_needs();
        n.sleep = 10;
        world.set_player_needs(n);

        let msg = complete_step(&mut world, ActionId::Sleep);
        assert_eq!(msg.as_deref(), Some("woke rested"));
        assert_eq!(world.player_needs().sleep, crate::needs::NEED_MAX);
    }

    /// Drive a 5-second multi-turn step to completion. The crafting
    /// verbs all queue exactly one step whose `target_secs` is
    /// `move_cost()` translated through the player's effective speed,
    /// so this tick-then-finish loop mirrors what the main frame loop
    /// does — keeps the tests honest about the real time flow.
    fn run_to_completion(world: &mut World) -> Vec<ActionId> {
        let mut completed = Vec::new();
        for _ in 0..200 {
            if world.active_action.is_none() {
                break;
            }
            let result = world.tick_multi_turn(1);
            for id in result.completed_steps {
                if complete_step(world, id).is_some() {
                    // Discard the message; tests assert on world state.
                }
                completed.push(id);
            }
            if result.interrupted {
                break;
            }
        }
        completed
    }

    fn place_lit_fire_east_of_player(world: &mut World, fuel_seconds: u32) -> (i32, i32) {
        let pos = world.player_pos();
        let (fx, fy) = (pos.x + 1, pos.y);
        if let Some(cell) = world.cell_at_mut(fx as i64, fy as i64) {
            cell.items.clear();
            cell.items.push(ItemInstance::unique(
                ItemKind::Firewood,
                500,
                None,
                ItemMetadata::Lit { fuel_seconds },
            ));
        }
        (fx, fy)
    }

    #[test]
    fn place_pan_then_pick_up_pan_round_trips_through_pack() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        let (fx, fy) = place_lit_fire_east_of_player(&mut world, 1_000);

        assert!(matches!(
            evaluate(&world, ActionId::PlacePan),
            Availability::Available { .. }
        ));
        execute(&mut world, ActionId::PlacePan);
        run_to_completion(&mut world);

        // East cell now holds a PannedOnFire CookingPan.
        let pan = world
            .cell_at(fx as i64, fy as i64)
            .and_then(|c| c.items.iter().find(|i| i.kind == ItemKind::CookingPan).cloned());
        let pan = pan.expect("pan placed on fire cell");
        assert!(matches!(
            pan.metadata,
            ItemMetadata::PannedOnFire { contents: PanContents::Empty, .. }
        ));

        // Pack lost the cooking pan it had at start.
        assert!(!world.player_pack().has_stack(ItemKind::CookingPan));

        // Now lift it back.
        execute(&mut world, ActionId::PickUpPan);
        run_to_completion(&mut world);

        // Cell no longer has a pan; fuel returned as Lit firewood.
        let cell = world.cell_at(fx as i64, fy as i64).unwrap();
        assert!(!cell.items.iter().any(|i| i.kind == ItemKind::CookingPan));
        assert!(cell.items.iter().any(|i| matches!(i.metadata, ItemMetadata::Lit { .. })));
        // Pack regained the cooking pan.
        assert!(world.player_pack().has_stack(ItemKind::CookingPan));
    }

    #[test]
    fn full_cook_fish_flow_with_herb_modifier_yields_herbed_cooked_fish() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        // Give the player a fish + a herb in pack.
        world.player_pack_mut().try_add(ItemInstance::stack(
            ItemKind::Fish,
            1,
            ItemKind::Fish.def().default_weight_g,
            None,
            ItemMetadata::None,
        )).ok();
        world.player_pack_mut().try_add(ItemInstance::unique(
            ItemKind::Herb,
            10,
            None,
            ItemMetadata::None,
        )).ok();
        let (fx, fy) = place_lit_fire_east_of_player(&mut world, 10_000);

        // 1. Place pan.
        execute(&mut world, ActionId::PlacePan);
        run_to_completion(&mut world);

        // 2. Start the cook.
        assert!(matches!(
            evaluate(&world, ActionId::CookFish),
            Availability::Available { .. }
        ));
        execute(&mut world, ActionId::CookFish);
        run_to_completion(&mut world);

        // Fish consumed; pan now Cooking.
        assert!(!world.player_pack().has_stack(ItemKind::Fish));
        let pan_meta = world
            .cell_at(fx as i64, fy as i64)
            .and_then(|c| c.items.iter().find_map(|i| match i.metadata {
                ItemMetadata::PannedOnFire { contents, .. } => Some(contents),
                _ => None,
            }))
            .expect("pan still on fire");
        assert!(matches!(pan_meta, PanContents::Cooking { input: CookableKind::Fish, .. }));

        // 3. Season pan while it's cooking.
        execute(&mut world, ActionId::SeasonPan);
        run_to_completion(&mut world);
        assert!(!world.player_pack().has_stack(ItemKind::Herb));

        // 4. Advance time until cooking completes (target 120s).
        world.advance_time_raw(125);

        // 5. Take from pan.
        assert!(matches!(
            evaluate(&world, ActionId::TakeFromPan),
            Availability::Available { .. }
        ));
        execute(&mut world, ActionId::TakeFromPan);
        run_to_completion(&mut world);

        // Pack should now contain a Cooked Fish with Herb seasoning, state Ok.
        let cooked = world
            .player_pack()
            .contents
            .iter()
            .find(|i| i.kind == ItemKind::Cooked)
            .cloned()
            .expect("cooked food in pack");
        match cooked.metadata {
            ItemMetadata::Cooked { base, state, seasonings } => {
                assert_eq!(base, CookableKind::Fish);
                assert_eq!(state, CookedState::Ok);
                assert!(seasonings.has(ModifierTag::Herb));
            }
            other => panic!("expected Cooked, got {:?}", other),
        }
        // Pan should be Empty again.
        let pan_meta = world
            .cell_at(fx as i64, fy as i64)
            .and_then(|c| c.items.iter().find_map(|i| match i.metadata {
                ItemMetadata::PannedOnFire { contents, .. } => Some(contents),
                _ => None,
            }))
            .unwrap();
        assert!(matches!(pan_meta, PanContents::Empty));
    }

    #[test]
    fn overcooking_past_burn_threshold_yields_burnt() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.player_pack_mut().try_add(ItemInstance::stack(
            ItemKind::Fish,
            1,
            ItemKind::Fish.def().default_weight_g,
            None,
            ItemMetadata::None,
        )).ok();
        place_lit_fire_east_of_player(&mut world, 10_000);
        execute(&mut world, ActionId::PlacePan);
        run_to_completion(&mut world);
        execute(&mut world, ActionId::CookFish);
        run_to_completion(&mut world);

        // Past 2x target = burn threshold (240s for fish).
        world.advance_time_raw(260);

        execute(&mut world, ActionId::TakeFromPan);
        run_to_completion(&mut world);
        let cooked = world
            .player_pack()
            .contents
            .iter()
            .find(|i| i.kind == ItemKind::Cooked)
            .cloned()
            .unwrap();
        match cooked.metadata {
            ItemMetadata::Cooked { state, .. } => assert_eq!(state, CookedState::Burnt),
            other => panic!("expected Cooked, got {:?}", other),
        }
    }

    #[test]
    fn fuel_exhaustion_during_cook_spills_food_onto_cell() {
        let mut world = World::new(CHUNK_W, CHUNK_H);
        world.player_pack_mut().try_add(ItemInstance::stack(
            ItemKind::Fish,
            1,
            ItemKind::Fish.def().default_weight_g,
            None,
            ItemMetadata::None,
        )).ok();
        // Only 30 seconds of fuel — fire will die mid-cook.
        let (fx, fy) = place_lit_fire_east_of_player(&mut world, 30);
        execute(&mut world, ActionId::PlacePan);
        run_to_completion(&mut world);
        execute(&mut world, ActionId::CookFish);
        run_to_completion(&mut world);

        // Drive the clock 60 seconds — well past the pan's fuel.
        world.advance_time_raw(60);

        // The PannedOnFire should be gone (back to plain pan, metadata
        // None) and a Cooked item should be on the cell.
        let cell = world.cell_at(fx as i64, fy as i64).unwrap();
        let pan = cell.items.iter().find(|i| i.kind == ItemKind::CookingPan).unwrap();
        assert!(matches!(pan.metadata, ItemMetadata::None));
        let cooked = cell.items.iter().find(|i| i.kind == ItemKind::Cooked);
        assert!(cooked.is_some(), "fish should spill onto cell as cooked");
    }

    #[test]
    fn eat_herb_unavailable_until_pickup_lands() {
        // Phase 11 introduces herbs in the world (and PickHerb). Until
        // then, EatHerb evaluates as "no herbs in pack" — the SAME message
        // that'll show even after phase 11 if the player just hasn't
        // picked any yet. So phase 11 doesn't need to revisit this verb.
        let world = World::new(CHUNK_W, CHUNK_H);
        match evaluate(&world, ActionId::EatHerb) {
            Availability::Unavailable { reason } => {
                assert_eq!(reason, "no herbs in pack");
            }
            other => panic!("got {:?}", other),
        }
    }
}
