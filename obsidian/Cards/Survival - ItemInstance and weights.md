Item identity, weight, and pack capacity.

## Model
- Replace the Holy Land `Inventory { stacks: Vec<(Item, u32)> }` with `Vec<ItemInstance>`.
- `ItemInstance { kind: ItemKind, weight_g: u32, durability: Option<u16>, charges: Option<u16>, metadata: ItemMetadata }`.
- Player has a `Pack { capacity_g: u32, contents: Vec<ItemInstance> }`. Pickup fails if total weight > capacity.

## Slice-1 starting load (15 kg pack)
| Item | Weight | Verbs |
|---|---|---|
| Axe | 1.0 kg | chop tree, chop log |
| Knife | 0.2 | pick herb |
| Pack | 1.0 (worn) | container |
| Rations × 3 | 1.5 | eat → +25 Hunger |
| Waterskin × 2 (full) | up to 2.4 | drink, fill |
| Flint + steel | 0.1 | fire-making attempt |
| Tent | 5.0 | pitch (300s) |
| Bedroll | 2.0 | unroll → sleep target |
| Pan | 1.0 | cook on lit fire |

Starting load: ~13.2 kg of 15 kg → ~1.8 kg slack for gathered wood/herbs. Forces drop-or-drag for big hauls.

## Items that exist but can't be carried (drag-only)
- Felled tree (~200 kg).
- Future: large stones, corpses.
These exist as world entities the player can drag (see `[[Survival - Drag mechanic]]`) but never enter the pack.

## Why stacks → instances
Survival needs per-instance state: a half-empty waterskin, a worn-out axe, a damp twig. Stacks can't carry that. Slight memory cost (each pickup is a Vec entry) is fine on Miyoo at slice-1 inventory sizes.
