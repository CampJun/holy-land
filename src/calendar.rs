// In-game calendar. 365-day Gregorian (no leap year), Sarum liturgical
// labels. Player spawns 21 March 1300 (Easter, Spring) — see
// Cornwall-Pilgrim.md §5.
//
// `calendar_day: u32` on World/RunSave counts days since 1 January 1300,
// 1-indexed (so day 1 = 1 Jan, day 80 = 21 Mar). Wraps year boundary as
// a plain u32 increment.
//
// Solar season boundaries: Mar 21 / Jun 21 / Sep 23 / Dec 21.

use serde::{Deserialize, Serialize};

pub const EPOCH_YEAR: u32 = 1300;
/// 21 March 1300, expressed as days-since-1-Jan (1-indexed).
pub const START_DAY: u32 = 80;
pub const DAYS_PER_YEAR: u32 = 365;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    /// Short label for HUD/dialog.
    pub fn label(self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Month {
    Jan,
    Feb,
    Mar,
    Apr,
    May,
    Jun,
    Jul,
    Aug,
    Sep,
    Oct,
    Nov,
    Dec,
}

impl Month {
    pub fn short_label(self) -> &'static str {
        match self {
            Month::Jan => "Jan",
            Month::Feb => "Feb",
            Month::Mar => "Mar",
            Month::Apr => "Apr",
            Month::May => "May",
            Month::Jun => "Jun",
            Month::Jul => "Jul",
            Month::Aug => "Aug",
            Month::Sep => "Sep",
            Month::Oct => "Oct",
            Month::Nov => "Nov",
            Month::Dec => "Dec",
        }
    }
}

/// Days-in-month for a non-leap year, indexed by Month-as-usize.
const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Day-of-year (1..=365) for the given calendar_day. Year-cycles via mod.
pub fn day_of_year(calendar_day: u32) -> u16 {
    let d = if calendar_day == 0 {
        1
    } else {
        ((calendar_day - 1) % DAYS_PER_YEAR) + 1
    };
    d as u16
}

/// (year, month, day-of-month) for the given calendar_day.
pub fn date_of(calendar_day: u32) -> (u32, Month, u8) {
    let d = if calendar_day == 0 { 1 } else { calendar_day };
    let year = EPOCH_YEAR + (d - 1) / DAYS_PER_YEAR;
    let mut doy = ((d - 1) % DAYS_PER_YEAR) + 1; // 1..=365
    for (i, &len) in DAYS_IN_MONTH.iter().enumerate() {
        if doy <= len {
            let month = match i {
                0 => Month::Jan,
                1 => Month::Feb,
                2 => Month::Mar,
                3 => Month::Apr,
                4 => Month::May,
                5 => Month::Jun,
                6 => Month::Jul,
                7 => Month::Aug,
                8 => Month::Sep,
                9 => Month::Oct,
                10 => Month::Nov,
                _ => Month::Dec,
            };
            return (year, month, doy as u8);
        }
        doy -= len;
    }
    // Unreachable: doy is bounded to 1..=365 and DAYS_IN_MONTH sums to 365.
    (year, Month::Dec, 31)
}

/// Season for the given calendar_day. Solar boundaries Mar 21 / Jun 21
/// / Sep 23 / Dec 21 — encoded as bands on `day_of_year` (1-indexed).
pub fn season_of(calendar_day: u32) -> Season {
    let doy = day_of_year(calendar_day);
    // Winter: [1, 79] + [355, 365]
    // Spring: [80, 171]   (Mar 21 – Jun 20)
    // Summer: [172, 265]  (Jun 21 – Sep 22)
    // Autumn: [266, 354]  (Sep 23 – Dec 20)
    if doy <= 79 || doy >= 355 {
        Season::Winter
    } else if doy <= 171 {
        Season::Spring
    } else if doy <= 265 {
        Season::Summer
    } else {
        Season::Autumn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_day_is_21_mar_1300_spring() {
        let (y, m, d) = date_of(START_DAY);
        assert_eq!(y, 1300);
        assert_eq!(m, Month::Mar);
        assert_eq!(d, 21);
        assert_eq!(season_of(START_DAY), Season::Spring);
    }

    #[test]
    fn season_of_known_dates() {
        // Mar 20 = day 79 = last day of Winter.
        assert_eq!(season_of(79), Season::Winter);
        // Mar 21 = day 80 = first Spring.
        assert_eq!(season_of(80), Season::Spring);
        // Jun 20 = day 171 = last Spring.
        assert_eq!(season_of(171), Season::Spring);
        // Jun 21 = day 172 = first Summer.
        assert_eq!(season_of(172), Season::Summer);
        // Sep 22 = day 265 = last Summer.
        assert_eq!(season_of(265), Season::Summer);
        // Sep 23 = day 266 = first Autumn.
        assert_eq!(season_of(266), Season::Autumn);
        // Dec 20 = day 354 = last Autumn.
        assert_eq!(season_of(354), Season::Autumn);
        // Dec 21 = day 355 = first Winter (year-end).
        assert_eq!(season_of(355), Season::Winter);
        // Jan 1 = day 1 = Winter.
        assert_eq!(season_of(1), Season::Winter);
    }

    #[test]
    fn date_of_known_dates() {
        assert_eq!(date_of(1), (1300, Month::Jan, 1));
        assert_eq!(date_of(32), (1300, Month::Feb, 1));
        assert_eq!(date_of(60), (1300, Month::Mar, 1));
        assert_eq!(date_of(80), (1300, Month::Mar, 21));
        assert_eq!(date_of(365), (1300, Month::Dec, 31));
        // Year wraps.
        assert_eq!(date_of(366), (1301, Month::Jan, 1));
        assert_eq!(date_of(365 + 80), (1301, Month::Mar, 21));
    }

    #[test]
    fn day_of_year_wraps_at_year_boundary() {
        assert_eq!(day_of_year(1), 1);
        assert_eq!(day_of_year(365), 365);
        assert_eq!(day_of_year(366), 1);
        assert_eq!(day_of_year(365 * 2 + 80), 80);
    }
}
