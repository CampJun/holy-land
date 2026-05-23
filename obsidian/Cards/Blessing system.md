Blessings are multiplicative modifiers on a god's power — not standalone passive buffs.

## Core model
- Each of the 16 gods grants **one power AND one blessing**. The blessing modifies that god's power specifically.
- Blessings stack **multiplicatively** on the same power. Compound interest creates the "ridiculously juiced" late-run state the design is aiming for.

## Example (Baal Hadad)
- Power: Chain Lightning — bounces 5 times by default.
- Blessing: +20% bounces.
- One blessing: 5 × 1.2 = 6 bounces.
- Two blessings: 6 × 1.2 = 7.2 → 7 bounces.
- Three blessings: 7 × 1.2 = 8.4 → 8 bounces.
- Diminishing-return rounding is part of the design — keeps blessings always-useful while avoiding runaway scaling.

## Run-start free blessing
- At run start, **one of all 16 powers** (regardless of unlock state) is randomly rolled to receive a free **level-1 blessing**.
- Player starts with 1 power unlocked → 1/16 chance the free blessing lands on a usable power.
- Unlocking more powers monotonically raises that probability (max 1.0 when all 16 unlocked).
- **Late-game meta-progression** unlocks extra starting blessings up to **4 total**: a full pre-blessed hand. Endgame state.

## In-run blessing acquisition
- Standard: offer blessings via deity events / altars during a run (specific mechanism TBD).

## Replaces old model
- Old roadmap passives ("Ishtar +25% damage", "Hathor +1 HP regen") are dropped.
- Every god now follows the **power + power-modifier blessing** shape.
- `holyland-PLAN.md` and `holyland-ROADMAP.md` need updating to reflect this.

## Open
- In-run blessing acquisition mechanism (random pickups? deity favor? combat events?).
- Diminishing-return rounding policy at high stack counts (round-down keeps it simple; could explore floor/ceiling per-blessing-type).

Source: design plan dated 2026-05-14.
