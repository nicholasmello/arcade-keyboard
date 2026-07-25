use alloc::vec::Vec;
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Sender;
use futures_util::stream::{FuturesUnordered, StreamExt};

pub struct Key<'a> {
    button: Input<'a>,
    value: char,
}

impl Key<'_> {
    #[allow(unused)]
    pub async fn set_value(mut self, value: char) {
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

    pub fn add_key(&mut self, button: Input<'d>, value: char) -> Result<(), KeyboardError> {
        let key = self
            .keys
            .get_mut(self.num_keys)
            .ok_or(KeyboardError::MaxKeys)?;
        *key = Some(Key { button, value });

        self.num_keys += 1;

        Ok(())
    }

    pub async fn process<'ch, M, const N: usize>(mut self, sender: Sender<'ch, M, Vec<char>, N>)
    where
        M: RawMutex,
    {
        loop {
            self.keys
                .iter_mut()
                .filter_map(|opt| opt.as_mut().map(|s| s.button.wait_for_any_edge()))
                .collect::<FuturesUnordered<_>>()
                .next()
                .await;

            let mut pressed: Vec<char> = Vec::new();
            for key in self.keys.iter_mut().filter_map(|opt| opt.as_mut()) {
                if key.button.is_low() {
                    pressed.push(key.value);
                }
            }
            sender.send(pressed).await;
        }
    }
}
