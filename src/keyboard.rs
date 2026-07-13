use crate::usb::{KeyboardAction, KeyboardEvent};
use defmt::*;
use embassy_rp::gpio::Input;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};

pub struct Key<'a> {
    button: Input<'a>,
    value: char,
}

impl Key<'_> {
    pub async fn process<'ch, M, const N: usize>(
        &mut self,
        sender: Sender<'ch, M, KeyboardEvent, N>,
    ) where
        M: RawMutex,
    {
        loop {
            self.button.wait_for_high().await;
            info!("Button {} pressed", self.value);

            sender
                .send(KeyboardEvent {
                    key: self.value,
                    action: KeyboardAction::Press,
                })
                .await;

            // Debounce
            Timer::after(Duration::from_millis(50)).await;

            self.button.wait_for_low().await;
            info!("Button {} unpressed", self.value);

            sender
                .send(KeyboardEvent {
                    key: self.value,
                    action: KeyboardAction::Depress,
                })
                .await;

            // Debounce
            Timer::after(Duration::from_millis(50)).await;
        }
    }
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

    pub async fn process<'ch, M, const N: usize>(mut self, sender: Sender<'ch, M, KeyboardEvent, N>)
    where
        M: RawMutex,
    {
        // TODO: Make this a loop that is concatinated together
        self.keys[0].as_mut().unwrap().process(sender).await;
    }
}
