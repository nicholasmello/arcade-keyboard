use alloc::vec::Vec;
use core::cell::OnceCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use defmt::*;
use embassy_futures::join::join;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Receiver;
use embassy_usb::UsbDevice;
use embassy_usb::class::hid::{
    HidBootProtocol, HidProtocolMode, HidReader, HidReaderWriter, HidSubclass, HidWriter, ReportId,
    RequestHandler, State,
};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder, Config, Handler};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

pub static HID_PROTOCOL_MODE: AtomicU8 = AtomicU8::new(HidProtocolMode::Boot as u8);

pub struct UsbKeyboardBuf<'d> {
    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    config_descriptor: [u8; 256],
    bos_descriptor: [u8; 256],
    // You can also add a Microsoft OS descriptor.
    msos_descriptor: [u8; 256],
    control_buf: [u8; 64],

    state: OnceCell<State<'d>>,
    device_handler: OnceCell<MyDeviceHandler>,
}

impl<'d> UsbKeyboardBuf<'d> {
    pub const fn new() -> Self {
        Self {
            config_descriptor: [0; 256],
            bos_descriptor: [0; 256],
            msos_descriptor: [0; 256],
            control_buf: [0; 64],
            state: OnceCell::new(),
            device_handler: OnceCell::new(),
        }
    }
}

pub struct UsbKeyboard<'d> {
    usb: UsbDevice<'d, Driver<'d, USB>>,
    reader: HidReader<'d, Driver<'d, USB>, 1>,
    writer: HidWriter<'d, Driver<'d, USB>, 8>,
}

impl<'d> UsbKeyboard<'d> {
    // HidWriter<'_, Driver<'_, USB>, 8>
    pub fn create_usb(buf: &'d mut UsbKeyboardBuf<'d>, driver: Driver<'d, USB>) -> Self {
        // Create embassy-usb Config
        let mut config = Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Nicholas Mello");
        config.product = Some("Pi Pico Arcade Keyboard");
        config.serial_number = Some("00000001");
        config.max_power = 100;
        config.max_packet_size_0 = 64;
        config.composite_with_iads = false;
        config.device_class = 0;
        config.device_sub_class = 0;
        config.device_protocol = 0;

        if buf.state.set(State::new()).is_err() {
            defmt::panic!("USB State already initalized");
        }

        let mut builder = Builder::new(
            driver,
            config,
            &mut buf.config_descriptor,
            &mut buf.bos_descriptor,
            &mut buf.msos_descriptor,
            &mut buf.control_buf,
        );

        if buf.device_handler.set(MyDeviceHandler::new()).is_err() {
            defmt::panic!("USB Device Handler already initialized");
        }

        builder.handler(buf.device_handler.get_mut().unwrap());

        // Create classes on the builder.
        let config = embassy_usb::class::hid::Config {
            report_descriptor: KeyboardReport::desc(),
            request_handler: None,
            poll_ms: 60,
            max_packet_size: 64,
            hid_subclass: HidSubclass::Boot,
            hid_boot_protocol: HidBootProtocol::Keyboard,
        };
        let hid =
            HidReaderWriter::<_, 1, 8>::new(&mut builder, buf.state.get_mut().unwrap(), config);

        let (reader, writer) = hid.split();

        let usb = builder.build();

        Self {
            usb,
            reader,
            writer,
        }
    }

    pub async fn run<'ch, M, const N: usize>(self, receiver: Receiver<'ch, M, Vec<char>, N>)
    where
        M: RawMutex,
    {
        let mut request_handler = MyRequestHandler {};

        let Self {
            mut usb,
            reader,
            mut writer,
        } = self;

        let reader_fut = async {
            reader.run(false, &mut request_handler).await;
        };

        let writer_fut = async {
            loop {
                let event = receiver.receive().await;

                let mut boot_keys = [0, 0, 0, 0, 0, 0, 0, 0];

                for (key, idx) in event.iter().zip(2usize..) {
                    let boot_key = match key {
                        'a' => 4,
                        _ => defmt::panic!("Not supported key: {}", key),
                    };
                    boot_keys[idx] = boot_key;
                }

                if HID_PROTOCOL_MODE.load(Ordering::Relaxed) == HidProtocolMode::Boot as u8 {
                    match writer.write(&boot_keys).await {
                        Ok(()) => {}
                        Err(e) => warn!("Failed to send boot report: {:?}", e),
                    };
                } else {
                    defmt::unimplemented!("Only supports boot mode keyboards!");
                    // // Create a report with the A key pressed. (no shift modifier)
                    // let report = KeyboardReport {
                    //     keycodes: [4, 0, 0, 0, 0, 0],
                    //     leds: 0,
                    //     modifier: 0,
                    //     reserved: 0,
                    // };
                    // // Send the report.
                    // match writer.write_serialize(&report).await {
                    //     Ok(()) => {}
                    //     Err(e) => warn!("Failed to send report: {:?}", e),
                    // };
                }
            }
        };

        join(usb.run(), join(reader_fut, writer_fut)).await;
    }
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {=[u8]}", id, data);
        OutResponse::Accepted
    }

    fn get_protocol(&self) -> HidProtocolMode {
        let protocol = HidProtocolMode::from(HID_PROTOCOL_MODE.load(Ordering::Relaxed));
        info!("The current HID protocol mode is: {}", protocol);
        protocol
    }

    fn set_protocol(&mut self, protocol: HidProtocolMode) -> OutResponse {
        info!("Switching to HID protocol mode: {}", protocol);
        HID_PROTOCOL_MODE.store(protocol as u8, Ordering::Relaxed);
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}

struct MyDeviceHandler {
    configured: AtomicBool,
}

impl MyDeviceHandler {
    fn new() -> Self {
        MyDeviceHandler {
            configured: AtomicBool::new(false),
        }
    }
}

impl Handler for MyDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        self.configured.store(false, Ordering::Relaxed);
        if enabled {
            info!("Device enabled");
        } else {
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        self.configured.store(false, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        self.configured.store(false, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        if configured {
            info!(
                "Device configured, it may now draw up to the configured current limit from Vbus."
            )
        } else {
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }
}
