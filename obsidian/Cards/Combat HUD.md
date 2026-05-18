Build the combat overlay. Default view is uncluttered; power UI reveals only on modifier hold.

## Default elements
- **AP bar** — turn economy. Visible top or bottom strip; drains as the player acts this turn.
- **Target info** — currently auto-targeted enemy's name, HP bar, status effects. Updates as the player cycles targets.
- **"In striking range" alert** — fires when any enemy can reach the player in their next turn. Visual + log message.

## Modifier-hold overlay (NOT in default view)
- The 4 power slots reveal only while the power modifier (L or R on Miyoo) is held.
- Each slot shows: glyph/icon, name, cost (AP / mana / cooldown), greyed if unavailable.
- Once a slot is picked + modifier released → enter targeting flow (see [[Power system]]).

## Status / log
- Existing currency HUD stays.
- Add HP + mana bar near AP bar (top-left cluster). Existing HP indicator may already cover this.
- Combat log: damage dealt/taken, misses, crits, blessing procs.

## Alert visual treatment (TBD)
Pick during implementation: glyph blink on threatening enemy, reverse-video glyph, corner-of-screen icon, log message, or combination.

## Open
- Power-overlay placement: full-screen radial, edge strip, or center popup?
- Combat log: lines visible at once, fade timing.
- Damage-number floaters yes/no (CDDA-style log-only vs. typical RPG floaters).

Source: design plan dated 2026-05-14.
