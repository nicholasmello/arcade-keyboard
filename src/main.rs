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
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embedded_alloc::LlffHeap as Heap;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    static mut HEAP_MEM: [u8; 4096] = [0; 4096];
    unsafe {
        HEAP.init(
            core::ptr::addr_of!(HEAP_MEM) as usize,
            core::mem::size_of::<[u8; 4096]>(),
        );
    }

    let keyboard_events: Channel<ThreadModeRawMutex, usb::KeyboardEvent, 64> = Channel::new();

    let p = embassy_rp::init(Default::default());

    // Create the driver, from the HAL.
    let driver = Driver::new(p.USB, Irqs);
    let mut usb_buf = usb::UsbKeyboardBuf::new();
    let usbkey = usb::UsbKeyboard::create_usb(&mut usb_buf, driver);

    // Set up the signal pin that will be used to trigger the keyboard.
    let mut signal_pin = Input::new(p.PIN_16, Pull::None);

    // Enable the schmitt trigger to slightly debounce.
    signal_pin.set_schmitt(true);

    let mut keyboard = Keyboard::<1>::new();
    match keyboard.add_key(signal_pin, 'a') {
        Ok(()) => info!("Key 'a' registered"),
        Err(_) => {
            defmt::panic!("Failed to register key!");
        }
    }

    let led = Output::new(p.PIN_25, Level::Low);

    join(
        join(usbkey.run(keyboard_events.receiver()), heartbeat(led)),
        keyboard.process(keyboard_events.sender()),
    )
    .await;
}
