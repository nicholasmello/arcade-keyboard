use core::sync::atomic::Ordering;

use defmt::*;
use embassy_rp::gpio::Input;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use embassy_usb::{
    class::hid::{HidProtocolMode, HidWriter},
    driver::Driver,
};
use usbd_hid::descriptor::KeyboardReport;

use crate::HID_PROTOCOL_MODE;

const WRITE_N: usize = 8;

pub struct Key<'a> {
    button: Input<'a>,
    value: char,
}

impl Key<'_> {
    pub async fn process<'d, D: Driver<'d>>(
        &mut self,
        mut writer: Mutex<CriticalSectionRawMutex, HidWriter<'d, D, WRITE_N>>,
    ) {
        loop {
            self.button.wait_for_high().await;
            info!("Button {} pressed", self.value);

            if HID_PROTOCOL_MODE.load(Ordering::Relaxed) == HidProtocolMode::Boot as u8 {
                match writer.get_mut().write(&[0, 0, 4, 0, 0, 0, 0, 0]).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send boot report: {:?}", e),
                };
            } else {
                // Create a report with the A key pressed. (no shift modifier)
                let report = KeyboardReport {
                    keycodes: [4, 0, 0, 0, 0, 0],
                    leds: 0,
                    modifier: 0,
                    reserved: 0,
                };
                // Send the report.
                match writer.get_mut().write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };
            }

            // Debounce
            Timer::after(Duration::from_millis(50)).await;

            self.button.wait_for_low().await;
            info!("Button {} unpressed", self.value);

            if HID_PROTOCOL_MODE.load(Ordering::Relaxed) == HidProtocolMode::Boot as u8 {
                match writer.get_mut().write(&[0, 0, 0, 0, 0, 0, 0, 0]).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send boot report: {:?}", e),
                };
            } else {
                let report = KeyboardReport {
                    keycodes: [0, 0, 0, 0, 0, 0],
                    leds: 0,
                    modifier: 0,
                    reserved: 0,
                };
                match writer.get_mut().write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => warn!("Failed to send report: {:?}", e),
                };
            }

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

pub struct Keyboard<'d, D: Driver<'d>, const KEY_N: usize> {
    writer: Mutex<CriticalSectionRawMutex, HidWriter<'d, D, WRITE_N>>,
    keys: [Option<Key<'d>>; KEY_N],
    num_keys: usize,
}

pub enum KeyboardError {
    MaxKeys,
}

impl<'d, D: Driver<'d>, const KEY_N: usize> Keyboard<'d, D, KEY_N> {
    pub fn new(writer: HidWriter<'d, D, WRITE_N>) -> Self {
        Self {
            writer: Mutex::new(writer),
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

    pub async fn process(mut self) {
        // TODO: Make this a loop that is concatinated together
        self.keys[0].as_mut().unwrap().process(self.writer).await;
    }
}
