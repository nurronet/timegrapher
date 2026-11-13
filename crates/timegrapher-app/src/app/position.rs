//! The positions the watch can be put in from the Current Position card,
//! the keys that pick them, and the 6 | 10 choice of how many the card
//! offers (#1473). Every name and code is core's
//! ([`timegrapher_core::session::Position`]); this only orders them for
//! the card and maps them to keys.

use eframe::egui;
use timegrapher_core::session::Position as Core;

/// A position on the Current Position card: the six standard test
/// positions, and the four 45° stops between the upright ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    DialUp,
    DialDown,
    CrownUp,
    CrownDown,
    CrownLeft,
    CrownRight,
    Up1h30,
    Up4h30,
    Up7h30,
    Up10h30,
}

impl Position {
    /// The six standard ones, in the card's order.
    pub const ALL: [Position; 6] = [
        Position::DialUp,
        Position::DialDown,
        Position::CrownUp,
        Position::CrownDown,
        Position::CrownLeft,
        Position::CrownRight,
    ];

    /// The four 45° stops, in hour order: the card's second row at 10.
    pub const HALF_HOURS: [Position; 4] = [
        Position::Up1h30,
        Position::Up4h30,
        Position::Up7h30,
        Position::Up10h30,
    ];

    /// All ten, the six then the four.
    pub const TEN: [Position; 10] = [
        Position::DialUp,
        Position::DialDown,
        Position::CrownUp,
        Position::CrownDown,
        Position::CrownLeft,
        Position::CrownRight,
        Position::Up1h30,
        Position::Up4h30,
        Position::Up7h30,
        Position::Up10h30,
    ];

    /// The six, or all ten, as the card offers them.
    pub fn offered(ten: bool) -> &'static [Position] {
        if ten {
            &Position::TEN
        } else {
            &Position::ALL
        }
    }

    /// A position by any name the session files take: DU, CH, "dial up",
    /// 1H30…
    pub(crate) fn from_name(s: &str) -> Option<Position> {
        Self::of_core(Core::parse(s)?)
    }

    /// The window's position for core's.
    pub(crate) fn of_core(p: Core) -> Option<Position> {
        Position::TEN.into_iter().find(|q| q.core() == p)
    }

    /// Core's position, which names it.
    pub fn core(self) -> Core {
        match self {
            Position::DialUp => Core::CH,
            Position::DialDown => Core::CB,
            Position::CrownUp => Core::H3,
            Position::CrownDown => Core::H9,
            Position::CrownLeft => Core::H6,
            Position::CrownRight => Core::H12,
            Position::Up1h30 => Core::H1h30,
            Position::Up4h30 => Core::H4h30,
            Position::Up7h30 => Core::H7h30,
            Position::Up10h30 => Core::H10h30,
        }
    }

    /// The short code on its button: DU … CR, and 1H30 … for the 45°
    /// stops, which have no other.
    pub fn code(self) -> &'static str {
        self.core().common()
    }

    /// Witschi's name: the hour that points up standing, CH and CB (cadran
    /// haut, cadran bas) lying flat.
    pub fn witschi(self) -> &'static str {
        self.core().code()
    }

    pub fn name(self) -> &'static str {
        self.core().name()
    }

    /// "Crown Up (CU, 3H)", "Dial Up (DU)", "1:30 Up (1H30)": core's
    /// display label, made once so the strip's bands can keep it.
    pub fn label(self) -> &'static str {
        static LABELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let labels =
            LABELS.get_or_init(|| Position::TEN.map(|p| p.core().display_label()).to_vec());
        let i = Position::TEN.iter().position(|p| *p == self).unwrap_or(0);
        &labels[i]
    }

    /// Whether this is one of the four 45° stops, shown only at 10.
    pub fn is_half_hour(self) -> bool {
        self.core().is_half_hour()
    }

    /// How the watch lies, for its picture on the Current Position card.
    pub(super) fn lying(self) -> crate::icons::Lying {
        use crate::icons::Lying;
        match (self, self.core().top_deg()) {
            (Position::DialDown, _) => Lying::DialDown,
            (_, None) => Lying::DialUp,
            (_, Some(deg)) => Lying::Top(deg as f32),
        }
    }

    /// The key that puts the watch here (Peter's pick, 12 Nov): U and D
    /// lying, and standing the last digit of the hour at the top, so 3H is
    /// 3, 12H is 2, 4H30 is 4 and 10H30 is 0.
    pub fn key_name(self) -> &'static str {
        match self {
            Position::DialUp => "U",
            Position::DialDown => "D",
            p => {
                let code = p.witschi();
                let hour = code.split('H').next().unwrap_or(code);
                &hour[hour.len() - 1..]
            }
        }
    }

    pub fn key(self) -> Option<egui::Key> {
        egui::Key::from_name(self.key_name())
    }

    /// The position whose key this is, if any.
    pub fn of_key(key: egui::Key) -> Option<Position> {
        Position::TEN.into_iter().find(|p| p.key() == Some(key))
    }
}

impl super::TimegrapherApp {
    /// The position key pressed this frame, alone: never with a modifier
    /// held, nor in the Watches view, nor while a field is being typed in,
    /// nor behind a dialog. The modifiers are the key event's own, as a
    /// shortcut's are: one pressed and let go within a frame is gone from
    /// the frame's by the time it is read. A 45° stop's key turns the card
    /// to 10 ([`super::App::change_position`]).
    pub(super) fn position_keys(&mut self, ctx: &egui::Context) {
        let dialog = ctx.memory(|m| m.top_modal_layer().is_some());
        if self.view != super::View::Bench || ctx.wants_keyboard_input() || dialog {
            return;
        }
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.is_none() => Position::of_key(*key),
                _ => None,
            })
        });
        if let Some(p) = pressed.filter(|_| self.position_editable()) {
            if p != self.position {
                self.change_position(p);
            }
        }
    }

    /// Whether the card offers ten: set by its 6 | 10 switch, and always
    /// while it shows a 45° stop, so it never shows one it has no button
    /// for.
    pub(super) fn card_ten(&self) -> bool {
        self.ten_positions || self.shown_position().is_half_hour()
    }
}
