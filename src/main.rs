#![no_std]
#![no_main]

mod heartbeat;
mod keyboard;
mod usb;

use crate::heartbeat::heartbeat;
use crate::keyboard::Keyboard;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs);
    let mut usb_buf = usb::UsbKeyboardBuf::new();
    let (usbkey, writer) = usb::UsbKeyboard::create_usb(&mut usb_buf, driver);

    // Set up the signal pin that will be used to trigger the keyboard.
    let mut signal_pin = Input::new(p.PIN_16, Pull::None);

    // Enable the schmitt trigger to slightly debounce.
    signal_pin.set_schmitt(true);

    let mut keyboard = Keyboard::<_, 1>::new(writer);
    match keyboard.add_key(signal_pin, 'a') {
        Ok(()) => info!("Key 'a' registered"),
        Err(_) => {
            defmt::panic!("Failed to register key!");
        }
    }

    let led = Output::new(p.PIN_25, Level::Low);

    join(join(usbkey.run(), heartbeat(led)), keyboard.process()).await;
}
