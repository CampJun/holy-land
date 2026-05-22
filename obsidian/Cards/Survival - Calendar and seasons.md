In-game date + season state. Foundational for every other card in the [[Survival - Seasons and flora - Brainstorm]] cluster.

## Shape

- 365-day Gregorian, no leap year. Sarum liturgical labels. Reference: Cornwall-Pilgrim.md §5.2.
- **Start date: 21 March 1300** (Easter, Spring). Reference: Cornwall-Pilgrim.md §5.1.
- Solar season boundaries (Cornwall-Pilgrim.md §5.3):

| Season | Range |
|---|---|
| Spring | 21 Mar – 20 Jun |
| Summer | 21 Jun – 22 Sep |
| Autumn | 23 Sep – 20 Dec |
| Winter | 21 Dec – 20 Mar |

## State

- New `pub calendar_day: u32` on `World` and on `RunSave`. Counts days since **1 Jan 1300** (so start of game = day 80). Wraps year boundary as a plain u32 increment.
- `clock_seconds` keeps today's-time-of-day semantics (0..86_400). When `clock_seconds` is advanced past 86_400, the overflow increments `calendar_day` and `clock_seconds %= 86_400`.
- Existing dawn-crossing hook in `main.rs:862–887` (the auto-save trigger) is the natural integration point: at every dawn, advance `calendar_day` and fire season-change effects if applicable.

## Module

`src/calendar.rs` (new, name reserved by Cornwall-Pilgrim.md §6.10). Contents:

```rust
pub const EPOCH_YEAR: u32 = 1300;
pub const START_DAY: u32 = 80; // 21 Mar 1300, zero-indexed from 1 Jan

pub enum Season { Spring, Summer, Autumn, Winter }
pub enum Month { Jan, Feb, Mar, Apr, May, Jun, Jul, Aug, Sep, Oct, Nov, Dec }

pub fn season_of(day: u32) -> Season;
pub fn date_of(day: u32) -> (u32 /*year*/, Month, u8 /*day_of_month*/);
pub fn day_of_year(day: u32) -> u16; // 0..365
pub fn day_of_week(day: u32) -> DayOfWeek; // future: Sarum feast lookup
```

`season_of` returns by `day_of_year` band: `[0, 79]` Winter, `[80, 171]` Spring, `[172, 264]` Summer, `[265, 354]` Autumn, `[355, 364]` Winter.

## Display

Status-bar entry: top-right corner of HUD shows `21 Mar · Spring` style label. Updated on dawn cross, not per-frame. One small CP437 block; reuses existing HUD render path.

## Save

- `RunSave` gains `pub calendar_day: u32` with `#[serde(default = "default_calendar_day")]` → `START_DAY` (80). Ships under the schema-v2 bump already drafted in [[Survival - Save schema v2]]. No new SCHEMA_VERSION beyond v2.
- Round-trip test: save → load preserves `calendar_day`.
- Season-change determinism test: save at day 79 23:30, fast-forward sleep across midnight to day 80, assert `season_of(day) == Spring`.

## Hooks fired on season change

Other cards consume the `SeasonChanged(old, new)` event at dawn:

- [[Survival - Seasonal ground cover and palette]] — re-evaluates ground cover for all loaded cells (lays snow at Winter start, clears at Spring start, etc.).
- [[Survival - Plant lifecycle and seasonal foraging]] — drives the lifecycle state machine (deciduous tints flip; berry plants enter Fruiting).
- Cornwall-World.md §369 — per-biome ambient temperature offset reads `season_of(calendar_day)` for the warmth model.

## Out of scope

- Feast-day banner (Cornwall-Pilgrim.md §5.5) — deferred polish.
- Moveable feasts (Easter, Pentecost) — algorithm exists (Computus), but no slice-1 mechanic reads them yet.
- Day-length variance / longer summer days — defer (today's flat 06:00–20:00 day window is fine for v1).
- Leap years — explicitly skipped per Cornwall-Pilgrim.md §5.2.
- Sleep-until-date verb — extends [[Survival - Multi-turn action queue]] Sleep; not blocking.

References: [[Cornwall-Pilgrim]] §5, [[Survival - Day night cycle]] (the time-of-day half — this card adds the date half), [[Survival - Save schema v2]], [[Survival - Multi-turn action queue]] (dawn-crossing already wired).
