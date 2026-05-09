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
    Start,
    Select,
}

const INITIAL_DELAY: Duration = Duration::from_millis(250);
const REPEAT_PERIOD: Duration = Duration::from_millis(100);

pub struct Input {
    gilrs: Option<Gilrs>,
    held: HashSet<Action>,
    next_repeat: Option<Instant>,
    queued: Vec<Action>,
}

impl Input {
    pub fn new() -> Self {
        let gilrs = Gilrs::new().ok();
        Self {
            gilrs,
            held: HashSet::new(),
            next_repeat: None,
            queued: Vec::new(),
        }
    }

    pub fn handle_sdl_event(&mut self, event: &Event) {
        match event {
            Event::KeyDown { keycode: Some(kc), repeat: false, .. } => {
                if let Some(action) = keycode_to_action(*kc) {
                    self.press(action);
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

fn keycode_to_action(kc: Keycode) -> Option<Action> {
    Some(match kc {
        Keycode::Up | Keycode::W | Keycode::K => Action::Up,
        Keycode::Down | Keycode::S | Keycode::J => Action::Down,
        Keycode::Left | Keycode::A | Keycode::H => Action::Left,
        Keycode::Right | Keycode::D | Keycode::L => Action::Right,
        Keycode::Z => Action::A,
        Keycode::X => Action::B,
        Keycode::C => Action::X,
        Keycode::V => Action::Y,
        Keycode::Q => Action::L,
        Keycode::E => Action::R,
        Keycode::Return => Action::Start,
        Keycode::Escape => Action::Start,
        Keycode::Backspace | Keycode::RShift => Action::Select,
        _ => return None,
    })
}

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
        Button::Start => Action::Start,
        Button::Select => Action::Select,
        _ => return None,
    })
}
