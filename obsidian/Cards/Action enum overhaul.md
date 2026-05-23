Extend `src/input.rs::Action` to support the new combat input flow without painting out the future analog-stick wheel.

## New variants needed
- Power-modifier hold/release (state, not a one-shot action).
- `Action::PowerSlot(0..=3)` — slot selection while modifier held.
- `Action::CycleTarget(Direction)` — dpad cycling between visible targets.
- `Action::ToggleFreeAim` — Select-button toggle.
- `Action::FreeAimMove(Direction)` — dpad moves the aim reticle.
- `Action::ConfirmCast` — confirm targeted power.
- `Action::CarefulStep(Direction)` — toggleable / modifier-held move that costs 50 AP instead of 25 + opportunity attack.
- `Action::Brace` — sword-and-board active block.

## Constraint
- Tier 2 analog-stick action wheels (Retroid-class) will eventually emit through the same `Action` enum. Don't bake "modifier+dpad" into slot semantics; the wheel will pick slots by stick direction.
- The wheel is a UI layer above the input layer (per `holyland-PLAN.md` portability rules).

## Implementation notes
- Existing `Action` enum is in `src/input.rs`. Repeat-on-hold logic for dpad already exists.
- Modifier-hold is a new shape — needs an explicit "modifier-down / modifier-up" pair in the key/button mapping. Both desktop and Miyoo kernel keymap need entries.
- On Miyoo: L is likely the power modifier; confirm L doesn't conflict with anything in `keycode_to_action`.

Source: design plan dated 2026-05-14.
