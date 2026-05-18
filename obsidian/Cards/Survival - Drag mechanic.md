Whole-object drag for items too heavy to carry. Slice 1 use case: felled tree → chop-at-destination.

## Why this exists
A felled tree weighs ~200 kg; nowhere near pack capacity (15 kg). Survival needs a way to move large objects without pretending the player can lift them.

## Model
- A `DraggableEntity` component on world objects that can't be picked up.
- `Action::DragStart(EntityId)` and `Action::DragEnd` toggle drag mode.
- While dragging:
  - Player movement cost doubles: 10 game-sec per tile instead of 5.
  - The dragged entity tracks the player's previous tile each step.
  - Drag breaks if the path between player and dragged entity is blocked.
  - Drag breaks if the player attempts an action incompatible with dragging (e.g. drink waterskin is fine; chop tree is not).

## Slice-1 drag targets
- Felled tree (the only one). Created when "Chop tree" succeeds — the tree-trunk cell becomes a `Log` draggable entity on the cell, the original cell becomes forest-floor.

## Chop-at-destination
At the desired fire site:
- Drop drag (or move adjacent and stop).
- "Chop log → firewood" action available with axe in hand.
- 60 game-seconds; produces 3–6 firewood items in the player's current or adjacent cells.

## Out of scope (slice 2+)
- Carts and vehicles for heavier hauling.
- Multi-actor drag (two NPCs lifting a stone).
- Drag-and-stack (multiple logs in a pile).

Reference: `[[Survival - ItemInstance and weights]]`, `[[Survival - Command menu]]`.
