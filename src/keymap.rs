use rmk::types::action::KeyAction;
use rmk::{a, k, mo};

pub(crate) const COL: usize = 16;
pub(crate) const ROW: usize = 6;
pub(crate) const NUM_LAYER: usize = 2;

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        // Layer 0: WIN_BASE
        [
            [k!(Escape), k!(F1), k!(F2), k!(F3), k!(F4), k!(F5), k!(F6), k!(F7), k!(F8), k!(F9), k!(F10), k!(F11), k!(F12), k!(Delete), a!(No), k!(AudioToggle)],
            [k!(Grave),  k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), k!(Kc7), k!(Kc8), k!(Kc9), k!(Kc0),  k!(Minus), k!(Equal), k!(Backspace), a!(No), k!(PageUp)],
            [k!(Tab),    k!(Q),   k!(W),   k!(E),   k!(R),   k!(T),   k!(Y),   k!(U),   k!(I),   k!(O),   k!(P),    k!(LeftBracket), k!(RightBracket), k!(Enter), a!(No), k!(PageDown)],
            [k!(CapsLock),k!(A),   k!(S),   k!(D),   k!(F),   k!(G),   k!(H),   k!(J),   k!(K),   k!(L),   k!(Semicolon), k!(Quote), a!(No), k!(Backslash), a!(No), k!(Home)],
            [k!(LShift),  k!(NonusBackslash), k!(Z), k!(X), k!(C), k!(V), k!(B), k!(N), k!(M), k!(Comma), k!(Dot), k!(Slash), a!(No), k!(RShift), k!(Up), a!(No)],
            [k!(LCtrl),   k!(LGui), k!(LAlt), a!(No), a!(No), a!(No), k!(Space), a!(No), a!(No), a!(No), k!(RAlt), mo!(1), k!(RCtrl), k!(Left), k!(Down), k!(Right)],
        ],
        // Layer 1: WIN_FN (fill later)
        [
            [a!(No); COL],
            [a!(No); COL],
            [a!(No); COL],
            [a!(No); COL],
            [a!(No); COL],
            [a!(No); COL],
        ],
    ]
}
