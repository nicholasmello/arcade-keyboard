use alloc::vec::Vec;
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Sender;
use futures_util::stream::{FuturesUnordered, StreamExt};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum UsbKeycodes {
    A = 4,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Zero,
    Enter,
    Escape,
    BackSpace,
    Tab,
    Space,
    Hyphen,
    Equal,
    SquareBracketLeft,
    SquareBracketRight,
    BackSlash,

    Semicolon = 51,
    SingleQuote,
    Backtick,
    Comma,
    Period,
    ForwardSlash,
    CapsLock,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    PrintScreen,
    ScrollLock,
    Pause,
    Insert,
    Home,
    PageUp,
    Delete,
    End,
    PageDown,
    Right,
    Left,
    Down,
    Up,
    NumLock,

    F13 = 104,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    Execute,
    Help,
    Menu,
    Select,
    Again,
    Undo,
    Cut,
    Copy,
    Paste,
    Find,
    Mute,
    VolumeUp,
    VolumeDown,

    ControlLeft = 224,
    ShiftLeft,
    AltLeft,
    GUILeft,
    ControlRight,
    ShiftRight,
    AltRight,
    GUIRight,
}

pub struct Key<'a> {
    button: Input<'a>,
    value: UsbKeycodes,
}

impl Key<'_> {
    #[allow(unused)]
    pub async fn set_value(mut self, value: UsbKeycodes) {
        self.value = value;
    }
}

pub struct Keyboard<'d, const KEY_N: usize> {
    keys: [Option<Key<'d>>; KEY_N],
    num_keys: usize,
}

#[derive(Debug, defmt::Format)]
pub enum KeyboardError {
    MaxKeys,
}

impl<'d, const KEY_N: usize> Keyboard<'d, KEY_N> {
    pub fn new() -> Self {
        Self {
            keys: [const { None }; KEY_N],
            num_keys: 0,
        }
    }

    pub fn add_key(&mut self, button: Input<'d>, value: UsbKeycodes) -> Result<(), KeyboardError> {
        let key = self
            .keys
            .get_mut(self.num_keys)
            .ok_or(KeyboardError::MaxKeys)?;
        *key = Some(Key { button, value });

        self.num_keys += 1;

        Ok(())
    }

    pub async fn process<'ch, M, const N: usize>(
        mut self,
        sender: Sender<'ch, M, Vec<UsbKeycodes>, N>,
    ) where
        M: RawMutex,
    {
        loop {
            self.keys
                .iter_mut()
                .filter_map(|opt| opt.as_mut().map(|s| s.button.wait_for_any_edge()))
                .collect::<FuturesUnordered<_>>()
                .next()
                .await;

            let mut pressed: Vec<UsbKeycodes> = Vec::new();
            for key in self.keys.iter_mut().filter_map(|opt| opt.as_mut()) {
                if key.button.is_low() {
                    pressed.push(key.value);
                }
            }
            sender.send(pressed).await;
        }
    }
}
