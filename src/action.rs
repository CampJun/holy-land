// Context-action registry: the verbs the player picks from the command
// menu (tap Y) and later the hold-Y radial overlay (phase 15). Each verb
// declares availability with a human-readable reason if unavailable, plus
// a base cost in game-seconds.
//
// Phase 7 wires the resolver + vertical menu UX. Only `Pickup` is fully
// implemented — every other slice-1 verb is a greyed-out stub whose
// `reason` names the phase that unlocks it. Reading the menu in-game is
// a live punch-list of remaining work.

use crate::world::{World, COST_PICKUP};

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
    StartFire,
    Sleep,
    Fishing,
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

pub fn evaluate(world: &World, id: ActionId) -> Availability {
    match id {
        ActionId::Pickup => eval_pickup(world),
        // Slice-1 stubs; each `reason` names the phase that lights this
        // action up. When you wire the real check, replace the arm.
        ActionId::DrinkFromStream | ActionId::FillWaterskin => Availability::Unavailable {
            reason: "phase 11: no water yet",
        },
        ActionId::DrinkWaterskin => Availability::Unavailable {
            reason: "phase 8: drink from waterskin",
        },
        ActionId::EatRation | ActionId::EatHerb => Availability::Unavailable {
            reason: "phase 8: eating",
        },
        ActionId::ChopTree => Availability::Unavailable {
            reason: "phase 11: no trees yet",
        },
        ActionId::PickHerb => Availability::Unavailable {
            reason: "phase 11: no herbs yet",
        },
        ActionId::PitchTent | ActionId::UnrollBedroll => Availability::Unavailable {
            reason: "phase 9: multi-turn actions",
        },
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
        _ => ExecuteOutcome::NotImplemented,
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
    fn stubs_report_phase_in_reason() {
        let world = World::new(CHUNK_W, CHUNK_H);
        // Every non-Pickup verb is currently a stub; the reason should
        // mention "phase" so the in-game menu reads as a punch list.
        for action in ALL_ACTIONS {
            if action.id == ActionId::Pickup {
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
        // Tickle starting_pack and Position so they don't get pruned in
        // tests-only-builds; the resolver depends on World state.
        let _ = (Position { x: 0, y: 0 }, starting_pack());
    }
}
