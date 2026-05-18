// Stdin-driven debug console for desktop QA. Excluded from the Miyoo build
// (target_arch = "arm") because the handheld has no keyboard / stdin TTY.
//
// Architecture: spawn a worker thread that blocks on `stdin.read_line`,
// parses each line into a `DebugCommand`, and ships it across an mpsc
// channel. The main thread drains the channel each frame and applies the
// commands inline, so the World is only ever touched from the main thread.
//
// Adding a new command: extend `DebugCommand`, the match arm in
// `parse_command`, and the match arm in `apply_debug_command`. The 'help'
// printer documents each command for the QA user.

use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::needs::NeedKind;
use crate::world::{Position, World, DAY_LENGTH_SECONDS};

#[derive(Debug)]
pub enum DebugCommand {
    Help,
    SetTime(u8, u8),
    AdvanceSecs(u32),
    SetNeed(NeedKind, u8),
    Teleport(i32, i32),
    Unknown(String),
}

pub struct DebugConsole {
    rx: Receiver<DebugCommand>,
}

impl DebugConsole {
    /// Spawn the stdin reader thread and return a console handle. The thread
    /// lives until EOF on stdin or until the receiver is dropped. The OS
    /// reaps it on process exit.
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            crate::log_info!("[debug] console ready. Type 'help' for commands.");
            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let cmd = parse_command(line.trim());
                        if tx.send(cmd).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Self { rx }
    }

    /// Pull every queued command and feed it to `f`. Non-blocking.
    pub fn drain<F: FnMut(DebugCommand)>(&self, mut f: F) {
        while let Ok(cmd) = self.rx.try_recv() {
            f(cmd);
        }
    }
}

fn parse_command(raw: &str) -> DebugCommand {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    match parts.as_slice() {
        [] => DebugCommand::Unknown(String::new()),
        ["help"] | ["?"] | ["h"] => DebugCommand::Help,

        ["time", hm] => parse_hm(hm)
            .map(|(h, m)| DebugCommand::SetTime(h, m))
            .unwrap_or_else(|| DebugCommand::Unknown(raw.to_string())),

        ["advance", secs] => secs
            .parse::<u32>()
            .map(DebugCommand::AdvanceSecs)
            .unwrap_or_else(|_| DebugCommand::Unknown(raw.to_string())),

        ["need", which, value] => {
            let kind = match *which {
                "thirst" | "t" => NeedKind::Thirst,
                "hunger" | "h" => NeedKind::Hunger,
                "sleep" | "s" => NeedKind::Sleep,
                "warmth" | "w" => NeedKind::Warmth,
                _ => return DebugCommand::Unknown(raw.to_string()),
            };
            match value.parse::<u8>() {
                Ok(v) => DebugCommand::SetNeed(kind, v.min(100)),
                Err(_) => DebugCommand::Unknown(raw.to_string()),
            }
        }

        ["tp", x, y] => match (x.parse::<i32>(), y.parse::<i32>()) {
            (Ok(x), Ok(y)) => DebugCommand::Teleport(x, y),
            _ => DebugCommand::Unknown(raw.to_string()),
        },

        _ => DebugCommand::Unknown(raw.to_string()),
    }
}

fn parse_hm(s: &str) -> Option<(u8, u8)> {
    let (h, m) = s.split_once(':')?;
    let h: u8 = h.parse().ok()?;
    let m: u8 = m.parse().ok()?;
    if h < 24 && m < 60 {
        Some((h, m))
    } else {
        None
    }
}

/// Apply a debug command directly to the world. Caller is responsible for
/// re-syncing any side trackers (e.g. last_dawn_idx) — clock-rewinding
/// commands intentionally don't trigger an auto-save, only forward
/// crossings detected by the regular dawn check do.
pub fn apply_debug_command(world: &mut World, cmd: DebugCommand) {
    match cmd {
        DebugCommand::Help => print_help(),

        DebugCommand::Unknown(s) => {
            if !s.is_empty() {
                crate::log_info!("[debug] unknown command: '{}'. type 'help'.", s);
            }
        }

        DebugCommand::SetTime(h, m) => {
            // Preserve which day the player is in; only the in-day offset
            // changes. This means `time 06:00` doesn't advance to "next
            // dawn" — it sets the clock to 06:00 of the *current* day,
            // which can rewind if the current time is already past 06:00.
            let day_offset = world.clock_seconds / DAY_LENGTH_SECONDS * DAY_LENGTH_SECONDS;
            world.clock_seconds = day_offset + h as u64 * 3600 + m as u64 * 60;
            // Day/night radius may have changed; refresh FOV.
            world.recompute_fov();
            crate::log_info!(
                "[debug] time set to {:02}:{:02} day {} (clock {}s)",
                h,
                m,
                world.day_count(),
                world.clock_seconds
            );
        }

        DebugCommand::AdvanceSecs(secs) => {
            world.clock_seconds = world.clock_seconds.saturating_add(secs as u64);
            world.recompute_fov();
            let (h, m) = world.clock_hm();
            crate::log_info!(
                "[debug] advanced {}s -> {:02}:{:02} day {}",
                secs,
                h,
                m,
                world.day_count()
            );
        }

        DebugCommand::SetNeed(kind, value) => {
            let mut n = world.player_needs();
            n.set(kind, value);
            world.set_player_needs(n);
            crate::log_info!("[debug] need {:?} = {}", kind, value);
        }

        DebugCommand::Teleport(x, y) => {
            world.set_player_pos(Position { x, y });
            world.recompute_fov();
            crate::log_info!("[debug] teleported to ({}, {})", x, y);
        }
    }
}

fn print_help() {
    crate::log_info!("[debug] commands:");
    crate::log_info!("  time HH:MM        set clock to HH:MM today (e.g. 'time 19:45')");
    crate::log_info!("  advance SECS      advance clock by SECS game-seconds");
    crate::log_info!("  need NAME VAL     set thirst|hunger|sleep|warmth to 0-100 (alias t|h|s|w)");
    crate::log_info!("  tp X Y            teleport player to world coords (X, Y)");
    crate::log_info!("  help | ? | h      show this");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_keyword_recognized() {
        assert!(matches!(parse_command("help"), DebugCommand::Help));
        assert!(matches!(parse_command("?"), DebugCommand::Help));
        assert!(matches!(parse_command("h"), DebugCommand::Help));
    }

    #[test]
    fn empty_input_is_silent_unknown() {
        match parse_command("") {
            DebugCommand::Unknown(s) => assert!(s.is_empty()),
            _ => panic!("expected Unknown('')"),
        }
    }

    #[test]
    fn time_hh_mm_parses() {
        match parse_command("time 19:45") {
            DebugCommand::SetTime(19, 45) => {}
            other => panic!("expected SetTime(19, 45), got {:?}", other),
        }
        // Out of range minutes -> Unknown.
        assert!(matches!(parse_command("time 19:99"), DebugCommand::Unknown(_)));
        // Out of range hours -> Unknown.
        assert!(matches!(parse_command("time 24:00"), DebugCommand::Unknown(_)));
        // Missing colon -> Unknown.
        assert!(matches!(parse_command("time 1945"), DebugCommand::Unknown(_)));
    }

    #[test]
    fn advance_secs_parses() {
        assert!(matches!(
            parse_command("advance 3600"),
            DebugCommand::AdvanceSecs(3600)
        ));
        assert!(matches!(
            parse_command("advance not_a_number"),
            DebugCommand::Unknown(_)
        ));
    }

    #[test]
    fn need_command_accepts_aliases_and_clamps() {
        match parse_command("need thirst 50") {
            DebugCommand::SetNeed(NeedKind::Thirst, 50) => {}
            other => panic!("got {:?}", other),
        }
        match parse_command("need w 200") {
            // Clamps to 100.
            DebugCommand::SetNeed(NeedKind::Warmth, 100) => {}
            other => panic!("got {:?}", other),
        }
        assert!(matches!(
            parse_command("need nope 50"),
            DebugCommand::Unknown(_)
        ));
    }

    #[test]
    fn tp_command_parses_negatives() {
        assert!(matches!(
            parse_command("tp -5 30"),
            DebugCommand::Teleport(-5, 30)
        ));
        assert!(matches!(parse_command("tp foo bar"), DebugCommand::Unknown(_)));
    }

    #[test]
    fn whitespace_tolerant() {
        assert!(matches!(
            parse_command("  time  19:45  "),
            DebugCommand::SetTime(19, 45)
        ));
    }
}
