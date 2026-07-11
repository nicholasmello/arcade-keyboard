use defmt::*;
use embassy_rp::gpio::Output;
use embassy_time::{Duration, Timer};

pub async fn heartbeat(mut led: Output<'_>) {
    loop {
        info!("Alive!");
        led.toggle();
        Timer::after(Duration::from_millis(1000)).await;
    }
}
