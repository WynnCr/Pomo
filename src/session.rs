//! The three kinds of session a pomodoro cycle alternates between.

use crate::theme::hex;
use gpui_kit::Hsla;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Session {
    Work,
    ShortBreak,
    LongBreak,
}

impl Session {
    pub const ALL: [Session; 3] = [Session::Work, Session::ShortBreak, Session::LongBreak];

    pub fn label(self) -> &'static str {
        match self {
            Session::Work => "Focus",
            Session::ShortBreak => "Short Break",
            Session::LongBreak => "Long Break",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Session::Work => "Focus",
            Session::ShortBreak => "Short",
            Session::LongBreak => "Long",
        }
    }

    pub fn accent(self) -> Hsla {
        match self {
            Session::Work => hex(0xe5484d),
            Session::ShortBreak => hex(0x4dabf7),
            Session::LongBreak => hex(0x69db7c),
        }
    }

    pub fn index(self) -> usize {
        match self {
            Session::Work => 0,
            Session::ShortBreak => 1,
            Session::LongBreak => 2,
        }
    }

    pub fn is_break(self) -> bool {
        !matches!(self, Session::Work)
    }

    pub fn completion_message(self) -> &'static str {
        match self {
            Session::Work => "Focus session complete. Time for a break.",
            Session::ShortBreak | Session::LongBreak => "Break is over. Ready to focus?",
        }
    }
}
