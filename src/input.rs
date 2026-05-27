#[cfg(feature = "gilrs")]
use gilrs::{Button, EventType as GilrsEvent, Gilrs};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L,
    R,
    /// Outer-left shoulder ("L2"). Bound on desktop (X key) for future use;
    /// not currently mapped by the Miyoo kernel keymap.
    L2,
    /// Outer-right shoulder ("R2"). Bound on desktop (V key); not mapped on
    /// Miyoo.
    R2,
    Start,
    Select,
    /// Toggle the full-screen Cornwall overmap. Bound to `M` on desktop;
    /// the Miyoo binding lands later as a Select+R chord.
    OpenOvermap,
    /// PR B Wait card: explicit pass-turn binding. Bound to `.` (period)
    /// on desktop. Miyoo lacks a dedicated wait key — the open-world
    /// input handler interprets `B` (a no-op in the open world today,
    /// since menu-close uses `B` only after higher-priority blocks
    /// already short-circuited) as Wait instead.
    Wait,
}

const INITIAL_DELAY: Duration = Duration::from_millis(250);
const REPEAT_PERIOD: Duration = Duration::from_millis(100);

pub struct Input {
    #[cfg(feature = "gilrs")]
    gilrs: Option<Gilrs>,
    held: HashSet<Action>,
    next_repeat: Option<Instant>,
    queued: Vec<Action>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "gilrs")]
            gilrs: Gilrs::new().ok(),
            held: HashSet::new(),
            next_repeat: None,
            queued: Vec::new(),
        }
    }

    pub fn handle_sdl_event(&mut self, event: &Event) {
        match event {
            Event::KeyDown { keycode: Some(kc), repeat: false, .. } => {
                match keycode_to_action(*kc) {
                    Some(action) => self.press(action),
                    None => crate::log_debug!("input: unmapped keycode {:?}", kc),
                }
            }
            Event::KeyUp { keycode: Some(kc), .. } => {
                if let Some(action) = keycode_to_action(*kc) {
                    self.release(action);
                }
            }
            _ => {}
        }
    }

    #[cfg(feature = "gilrs")]
    pub fn poll_gamepad(&mut self) {
        let mut transitions: Vec<(Action, bool)> = Vec::new();
        if let Some(gilrs) = self.gilrs.as_mut() {
            while let Some(ev) = gilrs.next_event() {
                match ev.event {
                    GilrsEvent::ButtonPressed(b, _) => {
                        if let Some(a) = button_to_action(b) {
                            transitions.push((a, true));
                        }
                    }
                    GilrsEvent::ButtonReleased(b, _) => {
                        if let Some(a) = button_to_action(b) {
                            transitions.push((a, false));
                        }
                    }
                    _ => {}
                }
            }
        }
        for (a, is_press) in transitions {
            if is_press {
                self.press(a);
            } else {
                self.release(a);
            }
        }
    }

    #[cfg(not(feature = "gilrs"))]
    pub fn poll_gamepad(&mut self) {}

    fn press(&mut self, a: Action) {
        if self.held.insert(a) {
            self.queued.push(a);
            if is_movement(a) {
                self.next_repeat = Some(Instant::now() + INITIAL_DELAY);
            }
        }
    }

    fn release(&mut self, a: Action) {
        self.held.remove(&a);
        if is_movement(a) && !self.held.iter().any(|h| is_movement(*h)) {
            self.next_repeat = None;
        }
    }

    /// Is `a` currently held? Used for press-vs-tap-vs-hold detection
    /// in main.rs (phase 15 hold-Y radial).
    pub fn is_held(&self, a: Action) -> bool {
        self.held.contains(&a)
    }

    pub fn drain(&mut self) -> Vec<Action> {
        let mut out = std::mem::take(&mut self.queued);
        let now = Instant::now();
        if let Some(when) = self.next_repeat {
            if now >= when {
                for a in [Action::Up, Action::Down, Action::Left, Action::Right] {
                    if self.held.contains(&a) {
                        out.push(a);
                    }
                }
                self.next_repeat = Some(now + REPEAT_PERIOD);
            }
        }
        out
    }
}

fn is_movement(a: Action) -> bool {
    matches!(a, Action::Up | Action::Down | Action::Left | Action::Right)
}

/// Desktop keyboard mapping (per user-locked spec):
///   Arrow keys -> dpad (Up/Down/Left/Right)
///   A S D F   -> face buttons A B X Y
///   Z X C V   -> shoulder buttons L L2 R R2
///   Escape    -> Start (quit-current-game)
///   L/R Shift -> Select (manual save)
///
/// The desktop layout intentionally does NOT inherit the Miyoo kernel
/// keymap (Space, LCtrl, LShift, etc.) because LShift means X-face on
/// Miyoo and Select on desktop — keeping them merged would be confusing.
#[cfg(not(target_arch = "arm"))]
fn keycode_to_action(kc: Keycode) -> Option<Action> {
    Some(match kc {
        Keycode::Up => Action::Up,
        Keycode::Down => Action::Down,
        Keycode::Left => Action::Left,
        Keycode::Right => Action::Right,
        Keycode::A => Action::A,
        Keycode::S => Action::B,
        Keycode::D => Action::X,
        Keycode::F => Action::Y,
        Keycode::Z => Action::L,
        Keycode::X => Action::L2,
        Keycode::C => Action::R,
        Keycode::V => Action::R2,
        Keycode::Escape => Action::Start,
        Keycode::LShift | Keycode::RShift => Action::Select,
        Keycode::M => Action::OpenOvermap,
        Keycode::Period => Action::Wait,
        _ => return None,
    })
}

/// Miyoo Mini Plus / Onion kernel keymap. The kernel emits these keycodes
/// when the physical buttons are pressed (per AGENTS.md): A=Space,
/// B=LCtrl, X=LShift, Y=LAlt, L=Tab, R=Backspace, Start=Enter,
/// Select=RCtrl, Menu=Esc. The dpad emits arrow keycodes. Raw keycode 116
/// (SDL's T) has also been observed for the menu/shutdown path and is
/// treated as a clean quit.
#[cfg(target_arch = "arm")]
fn keycode_to_action(kc: Keycode) -> Option<Action> {
    Some(match kc {
        Keycode::Up => Action::Up,
        Keycode::Down => Action::Down,
        Keycode::Left => Action::Left,
        Keycode::Right => Action::Right,
        Keycode::Space => Action::A,
        Keycode::LCtrl => Action::B,
        Keycode::LShift => Action::X,
        Keycode::LAlt => Action::Y,
        Keycode::Tab => Action::L,
        Keycode::Backspace => Action::R,
        Keycode::Return | Keycode::T => Action::Start,
        Keycode::RCtrl => Action::Select,
        Keycode::Escape => Action::Start,
        _ => return None,
    })
}

#[cfg(feature = "gilrs")]
fn button_to_action(b: Button) -> Option<Action> {
    Some(match b {
        Button::DPadUp => Action::Up,
        Button::DPadDown => Action::Down,
        Button::DPadLeft => Action::Left,
        Button::DPadRight => Action::Right,
        Button::South => Action::A,
        Button::East => Action::B,
        Button::West => Action::X,
        Button::North => Action::Y,
        Button::LeftTrigger => Action::L,
        Button::RightTrigger => Action::R,
        Button::LeftTrigger2 => Action::L2,
        Button::RightTrigger2 => Action::R2,
        Button::Start => Action::Start,
        Button::Select => Action::Select,
        _ => return None,
    })
}
