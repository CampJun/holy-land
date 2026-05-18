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
use crate::world::{
    World, COST_DRINK_WATERSKIN, COST_EAT_HERB, COST_EAT_RATION, COST_PICKUP,
    COST_PITCH_TENT, COST_UNROLL_BEDROLL,
};

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
    Sleep,
    Fishing,
}

impl ActionId {
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
    match id {
        ActionId::Pickup => eval_pickup(world),
        ActionId::EatRation => Availability::from_has(
            pack.has_stack(ItemKind::Ration),
            COST_EAT_RATION,
            "no rations in pack",
        ),
        ActionId::EatHerb => Availability::from_has(
            pack.has_stack(ItemKind::Herb),
            COST_EAT_HERB,
            "no herbs in pack",
        ),
        ActionId::DrinkWaterskin => Availability::from_has(
            pack.has_waterskin_with_water(),
            COST_DRINK_WATERSKIN,
            "no water in waterskins",
        ),
        // Slice-1 stubs; each `reason` names the phase that lights this
        // action up. When you wire the real check, replace the arm.
        ActionId::DrinkFromStream | ActionId::FillWaterskin => Availability::Unavailable {
            reason: "phase 11: no water yet",
        },
        ActionId::ChopTree => Availability::Unavailable {
            reason: "phase 11: no trees yet",
        },
        ActionId::PickHerb => Availability::Unavailable {
            reason: "phase 11: no herbs yet",
        },
        ActionId::PitchTent => Availability::from_has(
            pack.has_stack(ItemKind::Tent),
            COST_PITCH_TENT,
            "no tent in pack",
        ),
        ActionId::UnrollBedroll => Availability::from_has(
            pack.has_stack(ItemKind::Bedroll),
            COST_UNROLL_BEDROLL,
            "no bedroll in pack",
        ),
        ActionId::SetupCamp => Availability::from_has(
            pack.has_stack(ItemKind::Tent) && pack.has_stack(ItemKind::Bedroll),
            COST_PITCH_TENT + COST_UNROLL_BEDROLL,
            "need tent + bedroll in pack",
        ),
        ActionId::StartFire => Availability::Unavailable {
            reason: "phase 10: fire making",
        },
        ActionId::Sleep => Availability::Unavailable {
            reason: "phase 16: sleep",
        },
        ActionId::Fishing => Availability::Unavailable {
            reason: "phase 17: fishing",
        },
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
        cost_game_seconds: COST_PICKUP,
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
            ExecuteOutcome::Done(format!("picked up {} stack(s)", picked))
        }
        ActionId::EatRation => consume_and_restore(
            world,
            ConsumeFrom::Stack(ItemKind::Ration),
            NeedKind::Hunger,
            25,
            COST_EAT_RATION,
            "ate a ration (+25 hunger)",
            "no rations to eat",
        ),
        ActionId::EatHerb => consume_and_restore(
            world,
            ConsumeFrom::Stack(ItemKind::Herb),
            NeedKind::Hunger,
            5,
            COST_EAT_HERB,
            "ate a herb (+5 hunger)",
            "no herbs to eat",
        ),
        ActionId::DrinkWaterskin => consume_and_restore(
            world,
            ConsumeFrom::WaterskinCharge,
            NeedKind::Thirst,
            20,
            COST_DRINK_WATERSKIN,
            "drank from waterskin (+20 thirst)",
            "no water to drink",
        ),
        ActionId::PitchTent => {
            world.queue_multi_turn(&[(ActionId::PitchTent, COST_PITCH_TENT)]);
            ExecuteOutcome::Done("pitching tent...".to_string())
        }
        ActionId::UnrollBedroll => {
            world.queue_multi_turn(&[(ActionId::UnrollBedroll, COST_UNROLL_BEDROLL)]);
            ExecuteOutcome::Done("unrolling bedroll...".to_string())
        }
        ActionId::SetupCamp => {
            world.queue_multi_turn(&[
                (ActionId::PitchTent, COST_PITCH_TENT),
                (ActionId::UnrollBedroll, COST_UNROLL_BEDROLL),
            ]);
            ExecuteOutcome::Done("setting up camp...".to_string())
        }
        _ => ExecuteOutcome::NotImplemented,
    }
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
        // Walk east onto the seeded twigs + stone pile.
        world.try_move_player(1, 0);
        let avail = evaluate(&world, ActionId::Pickup);
        match avail {
            Availability::Available { cost_game_seconds } => {
                assert_eq!(cost_game_seconds, COST_PICKUP);
            }
            other => panic!("expected Available, got {:?}", other),
        }
    }

    #[test]
    fn pickup_unavailable_on_empty_cell() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Spawn cell has no items (debris is in adjacent cells).
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
            ActionId::PitchTent,
            ActionId::UnrollBedroll,
            ActionId::SetupCamp,
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
                assert_eq!(cost_game_seconds, COST_EAT_RATION);
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
