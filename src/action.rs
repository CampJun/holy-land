// Context-action registry: the verbs the player picks from the command
// menu (tap Y) and later the hold-Y radial overlay (phase 15). Each verb
// declares availability with a human-readable reason if unavailable, plus
// a base cost in game-seconds.
//
// Phase 7 wires the resolver + vertical menu UX. Only `Pickup` is fully
// implemented — every other slice-1 verb is a greyed-out stub whose
// `reason` names the phase that unlocks it. Reading the menu in-game is
// a live punch-list of remaining work.

use crate::items::{ItemInstance, ItemKind, ItemMetadata};
use crate::needs::NeedKind;
use crate::skill::{self, SkillKind};
use crate::world::{TerrainKind, World};

// ---- Per-verb design constants ---------------------------------------
//
// STYLE.md §2: source of truth for verb tuning lives in this module,
// not in world.rs. World.rs only owns engine-level constants (movement
// cost, day-length, FOV radii). Verb costs are inlined in
// `ActionId::base_cost()` so there's literally one match arm per verb
// to edit when rebalancing. Other verb tuning (materials, modifiers,
// output) lives as `const`s below, grouped by verb.

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
}

impl ActionId {
    /// Base cost in game-seconds before need-penalty amplification.
    /// Single source of truth for verb tuning — evaluators consult
    /// this for the menu's "Xs" readout; executors pass it into
    /// `world.spend_action_time`. Change a value here and every call
    /// site picks it up.
    pub fn base_cost(self) -> u32 {
        match self {
            ActionId::Pickup => 3,
            ActionId::EatRation => 10,
            ActionId::EatHerb => 5,
            ActionId::DrinkWaterskin => 5,
            ActionId::DrinkFromStream => 10,
            ActionId::FillWaterskin => 20,
            ActionId::PitchTent => 300,
            ActionId::UnrollBedroll => 30,
            // SetupCamp queues PitchTent + UnrollBedroll; the menu's
            // surfaced cost is the sum so the player sees the total.
            ActionId::SetupCamp => Self::PitchTent.base_cost() + Self::UnrollBedroll.base_cost(),
            ActionId::StartFire => 60,
            ActionId::FeedFire => 15,
            ActionId::ChopTree => 120,
            ActionId::PickHerb => 10,
            // Sleep jumps the clock; the "60s" cost surfacing isn't
            // meaningful. Phase-16 will replace this with an explicit
            // "Sleep until..." input.
            ActionId::Sleep => 0,
            ActionId::Fishing => 600,
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
    let cost = id.base_cost();
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
        ActionId::Sleep => Availability::Unavailable {
            reason: "phase 16: sleep",
        },
        ActionId::Fishing => Availability::Unavailable {
            reason: "phase 17: fishing",
        },
    }
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
        cost_game_seconds: ActionId::StartFire.base_cost(),
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
        cost_game_seconds: ActionId::FeedFire.base_cost(),
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
        cost_game_seconds: ActionId::Pickup.base_cost(),
    }
}

#[derive(Debug)]
pub enum ExecuteOutcome {
    /// The action ran; the inner message is suitable for log_info.
    Done(String),
    /// The action's implementation lives in a future phase. The menu
    /// keeps it greyed-out via `evaluate`, but execute is defensive in
    /// case wiring drifts.
    NotImplemented,
}

pub fn execute(world: &mut World, id: ActionId) -> ExecuteOutcome {
    match id {
        ActionId::Pickup => {
            let picked = world.try_pickup_all_at_player();
            if picked > 0 {
                world.spend_action_time(ActionId::Pickup.base_cost());
            }
            ExecuteOutcome::Done(format!("picked up {} stack(s)", picked))
        }
        ActionId::EatRation => consume_and_restore(
            world,
            ConsumeFrom::Stack(ItemKind::Ration),
            NeedKind::Hunger,
            25,
            ActionId::EatRation.base_cost(),
            "ate a ration (+25 hunger)",
            "no rations to eat",
        ),
        ActionId::EatHerb => consume_and_restore(
            world,
            ConsumeFrom::Stack(ItemKind::Herb),
            NeedKind::Hunger,
            5,
            ActionId::EatHerb.base_cost(),
            "ate a herb (+5 hunger)",
            "no herbs to eat",
        ),
        ActionId::DrinkWaterskin => consume_and_restore(
            world,
            ConsumeFrom::WaterskinCharge,
            NeedKind::Thirst,
            20,
            ActionId::DrinkWaterskin.base_cost(),
            "drank from waterskin (+20 thirst)",
            "no water to drink",
        ),
        ActionId::PitchTent => {
            world.queue_multi_turn(&[(ActionId::PitchTent, ActionId::PitchTent.base_cost())]);
            ExecuteOutcome::Done("pitching tent...".to_string())
        }
        ActionId::UnrollBedroll => {
            world.queue_multi_turn(&[(
                ActionId::UnrollBedroll,
                ActionId::UnrollBedroll.base_cost(),
            )]);
            ExecuteOutcome::Done("unrolling bedroll...".to_string())
        }
        ActionId::SetupCamp => {
            world.queue_multi_turn(&[
                (ActionId::PitchTent, ActionId::PitchTent.base_cost()),
                (ActionId::UnrollBedroll, ActionId::UnrollBedroll.base_cost()),
            ]);
            ExecuteOutcome::Done("setting up camp...".to_string())
        }
        ActionId::StartFire => execute_start_fire(world),
        ActionId::FeedFire => execute_feed_fire(world),
        ActionId::ChopTree => execute_chop_tree(world),
        ActionId::PickHerb => execute_pick_herb(world),
        ActionId::DrinkFromStream => execute_drink_from_stream(world),
        ActionId::FillWaterskin => execute_fill_waterskin(world),
        _ => ExecuteOutcome::NotImplemented,
    }
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

    world.spend_action_time(ActionId::StartFire.base_cost());
    // Phase 13b: a freshly lit fire bumps night FOV radius — recompute
    // so the extended sight shows on the same frame as the success log,
    // not after the next move.
    if success {
        world.recompute_fov();
    }
    ExecuteOutcome::Done(msg)
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

    world.spend_action_time(ActionId::FeedFire.base_cost());
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
        // SetupCamp expands into PitchTent + UnrollBedroll steps in the
        // queue; complete_step is never called with SetupCamp itself.
        // Other future multi-turn verbs land their finish effects here.
        _ => None,
    }
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
        ActionId::DrinkFromStream.base_cost(),
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
    world.spend_action_time(ActionId::DrinkFromStream.base_cost());
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
        ActionId::FillWaterskin.base_cost(),
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
        world.spend_action_time(ActionId::FillWaterskin.base_cost());
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
        ActionId::ChopTree.base_cost(),
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

    // Fell: terrain converts; FOV recomputes (the tree no longer blocks
    // sight). Drop a random pile of firewood on the now-grass cell so
    // the player can carry it back for fires.
    world.set_terrain_at(wx as i64, wy as i64, TerrainKind::Grass);
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
    world.spend_action_time(ActionId::ChopTree.base_cost());
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
        ActionId::PickHerb.base_cost(),
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
            world.spend_action_time(ActionId::PickHerb.base_cost());
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
                assert_eq!(cost_game_seconds, ActionId::Pickup.base_cost());
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
    fn unimplemented_verbs_report_phase_in_reason() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Verbs that are LIVE in phase 8 should report a real reason, not
        // a phase number; verbs that are still stubs should name their
        // unlocking phase.
        let live = [
            ActionId::Pickup,
            ActionId::EatRation,
            ActionId::EatHerb,
            ActionId::DrinkWaterskin,
            ActionId::DrinkFromStream,
            ActionId::FillWaterskin,
            ActionId::PitchTent,
            ActionId::UnrollBedroll,
            ActionId::SetupCamp,
            ActionId::StartFire,
            ActionId::FeedFire,
            ActionId::ChopTree,
            ActionId::PickHerb,
        ];
        for action in ALL_ACTIONS {
            if live.contains(&action.id) {
                continue;
            }
            match evaluate(&world, action.id) {
                Availability::Unavailable { reason } => {
                    assert!(
                        reason.starts_with("phase"),
                        "{:?} reason should name its phase, got '{}'",
                        action.id,
                        reason
                    );
                }
                other => panic!("{:?} expected stub Unavailable, got {:?}", action.id, other),
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
                assert_eq!(cost_game_seconds, ActionId::EatRation.base_cost());
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
                assert_eq!(cost_game_seconds, ActionId::StartFire.base_cost());
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
                assert_eq!(cost_game_seconds, ActionId::FeedFire.base_cost());
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
        // The world's per-second fire tick fires during spend_action_time,
        // so the lit fire also burns down by base_cost during the action.
        // Net: starting 600s + FIRE_FUEL_SECONDS_PER_FEED bump - base_cost burn.
        assert_eq!(
            lit_secs,
            600 + FIRE_FUEL_SECONDS_PER_FEED - ActionId::FeedFire.base_cost()
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
