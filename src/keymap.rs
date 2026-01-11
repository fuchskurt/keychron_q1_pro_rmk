use rmk::{
    a,
    encoder,
    k,
    layer,
    types::action::{EncoderAction, KeyAction},
};

pub(crate) const COL: usize = 16;
pub(crate) const ROW: usize = 6;
pub(crate) const NUM_LAYER: usize = 1;

pub(crate) const NUM_ENCODER: usize = 1;

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        layer!(
        // Layer 0: WIN_BASE
        [
            [k!(Escape), k!(F1), k!(F2), k!(F3), k!(F4), k!(F5), k!(F6), k!(F7), k!(F8), k!(F9), k!(F10), k!(F11), k!(F12), k!(Delete), a!(No), k!(AudioMute)],
            [k!(Grave),  k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), k!(Kc7), k!(Kc8), k!(Kc9), k!(Kc0),  k!(Minus), k!(Equal), k!(Backspace), a!(No), k!(PageUp)],
            [k!(Tab),    k!(Q),   k!(W),   k!(E),   k!(R),   k!(T),   k!(Y),   k!(U),   k!(I),   k!(O),   k!(P),    k!(LeftBracket), k!(RightBracket), k!(Enter), a!(No), k!(PageDown)],
            [k!(CapsLock),k!(A),   k!(S),   k!(D),   k!(F),   k!(G),   k!(H),   k!(J),   k!(K),   k!(L),   k!(Semicolon), k!(Quote), a!(No), k!(Backslash), a!(No), k!(Home)],
            [k!(LShift),  k!(NonusBackslash), k!(Z), k!(X), k!(C), k!(V), k!(B), k!(N), k!(M), k!(Comma), k!(Dot), k!(Slash), a!(No), k!(RShift), k!(Up), a!(No)],
            [k!(LCtrl),   k!(LGui), k!(LAlt), a!(No), a!(No), a!(No), k!(Space), a!(No), a!(No), a!(No), k!(RAlt), k!(RGui), k!(RCtrl), k!(Left), k!(Down), k!(Right)]
        ]),
    ]
}

#[rustfmt::skip]
pub const fn get_default_encoder_map() -> [[EncoderAction; NUM_ENCODER]; NUM_LAYER] {
    [
        // Layer 0
        [
            // Encoder 0: (Clockwise, Counter-Clockwise)
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
        ],
    ]
}
