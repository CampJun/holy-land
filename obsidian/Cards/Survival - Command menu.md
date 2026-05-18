Context-aware command menu on the Y button. Three modes: tap-vertical, hold-radial, multi-turn-toggle.

## Tap Y (release within ~200 ms with no dpad held)
Opens a vertical list of **all** context actions for current cell + 8 adjacent cells + player inventory.
- Each row: action name, short description, greyed-out reason if unavailable.
- Example: standing next to a tree with no axe → "Chop tree (needs an axe)" greyed with red tint.
- D-pad navigates; A confirms; B closes.

## Hold Y (≥ 200 ms held)
4-direction radial overlay with **top 4** priority actions.
- Priority order:
  1. Tool-in-hand + adjacent-target match (axe + tree → chop tree)
  2. Survival-critical (drink if Thirst < 25; eat if Hunger < 25)
  3. Recently-used (rolling LRU)
  4. Alphabetical fallback for the slot
- D-pad direction selects the corresponding action; release commits.
- Release with no dpad held → close, no action.

## Multi-turn-action toggle
While a multi-turn action is in progress, holding **Select** switches the HUD between **progress bar** (watch turns tick) and **time-skip** (jumps to end-of-action, simulating every elapsed turn so interrupt conditions can fire).

## Implementation
- `src/action.rs` carries a `ContextAction` registry; each entry declares `is_available(world, player) -> Result<(), Reason>` and `cost_game_seconds(world, player) -> u64` and `execute(world, player) -> Outcome`.
- Resolver runs every frame the menu is open (cheap; small action set).
- Reasons enum surfaces in the greyed-out tooltip.

## Input plumbing
- `src/input.rs`: add tap-vs-hold-Y discrimination. New `Action` variants: `Action::YTap`, `Action::YHoldStart`, `Action::YHoldRelease(Option<Dir>)`.
- 200 ms threshold; resettable on dpad-during-hold.

Reference: `[[Survival - Multi-turn action queue]]`.
