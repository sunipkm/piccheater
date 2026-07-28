// use core::sync::atomic::{AtomicBool, Ordering};
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    peripherals::USB,
    usb::{Driver as UsbDriver, InterruptHandler as UsbIrqHandler},
    // watchdog::Watchdog,
};
use embassy_usb::{
    Config as UsbConfig, UsbDevice,
    class::cdc_acm::{CdcAcmClass, State as CdcAcmState},
};
use heapless::String;
use kmdparse::parse;
use static_cell::StaticCell;

use crate::{
    CommandSender, MeasurementReceiver, ResponseReceiver, commands::Commands, resources::UsbDev,
};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbIrqHandler<USB>;
});

// /// Signal to sensor tasks to shut down
// pub(crate) static SHUTDOWN: AtomicBool = AtomicBool::new(false);

type CdcAcmDevice = CdcAcmClass<'static, UsbDriver<'static, USB>>;
type UsbDeviceDriver = UsbDevice<'static, UsbDriver<'static, USB>>;

pub static DAC_READY: AtomicBool = AtomicBool::new(false);

pub fn usb_task(
    spawner: &Spawner,
    usb: UsbDev,
    report: MeasurementReceiver,
    command: CommandSender,
    response: ResponseReceiver,
) {
    // Allocate static memory for the USB device and related state
    static CDC_CONF_STATE: StaticCell<CdcAcmState> = StaticCell::new();
    static CDC_TLM_STATE: StaticCell<CdcAcmState> = StaticCell::new();
    static CDC_LOG_STATE: StaticCell<CdcAcmState> = StaticCell::new();
    static CDC_DEVICE: StaticCell<CdcAcmDevice> = StaticCell::new();
    static TLM_DEVICE: StaticCell<CdcAcmDevice> = StaticCell::new();

    // Create the USB driver and attach interrupts
    let driver = UsbDriver::new(usb.usb, Irqs);
    trace!("USB driver created");
    // static CONFIG: StaticCell<UsbConfig> = StaticCell::new();
    // Create the USB device configuration
    let mut config = UsbConfig::new(0xc001, 0xfee1);
    config.manufacturer = Some("LoCSST/PIC-D");
    config.product = Some("PIC-D Heater DAC Rev.0");
    config.serial_number = Some("2026-0001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    trace!("USB configuration created");

    // Initialize the CDC ACM class state for both interfaces
    let state_conf = CDC_CONF_STATE.init(CdcAcmState::new());
    let state_tlm = CDC_TLM_STATE.init(CdcAcmState::new());
    let state_log = CDC_LOG_STATE.init(CdcAcmState::new());

    // USB builder to construct the device with the specified configuration and classes
    let mut usb_builder = rp_usb_reset::build_usb_builder!(driver, config);

    // Initialize the CDC ACM classes for both the configuration and telemetry interfaces
    let cdc_conf = CDC_DEVICE.init(CdcAcmClass::new(&mut usb_builder, state_conf, 64));
    let cdc_tlm = TLM_DEVICE.init(CdcAcmClass::new(&mut usb_builder, state_tlm, 64));

    // Set up USB logging
    let cdc_log = CdcAcmClass::new(&mut usb_builder, state_log, 64);
    match cdc_log_task(cdc_log) {
        Ok(_) => trace!("CDC log task initialized"),
        Err(e) => {
            error!("Failed to initialize CDC log task: {:?}", e);
            log::error!("Failed to initialize CDC log task: {:?}", e);
        }
    }

    // Build the USB device
    let usb = usb_builder.build();

    // Spawn the USB device task and CDC ACM tasks in the embassy executor
    match usb_device_task(usb) {
        Ok(t) => {
            spawner.spawn(t);
            trace!("USB device task spawned")
        }
        Err(e) => {
            error!("Failed to spawn USB device task: {:?}", e);
            log::error!("Failed to spawn USB device task: {:?}", e);
        }
    }
    match cdc_conf_task(cdc_conf, command, response) {
        Ok(t) => {
            spawner.spawn(t);
            trace!("CDC configuration input task spawned")
        }
        Err(e) => {
            error!("Failed to spawn CDC config task: {:?}", e);
            log::error!("Failed to spawn CDC config task: {:?}", e);
        }
    }
    match cdc_tlm_task(cdc_tlm, report) {
        Ok(t) => {
            spawner.spawn(t);
            trace!("CDC telemetry task spawned")
        }
        Err(e) => {
            error!("Failed to spawn CDC telemetry task: {:?}", e);
            log::error!("Failed to spawn CDC telemetry task: {:?}", e);
        }
    }
}

#[embassy_executor::task]
pub async fn cdc_conf_task(
    usb: &'static mut CdcAcmDevice,
    sender: CommandSender,
    receiver: ResponseReceiver,
) {
    let mut data = [0u8; 256];
    let mut msg = String::<256>::new();

    loop {
        usb.wait_connection().await;
        trace!("USB connected");
        while let Ok(n) = usb.read_packet(&mut data).await {
            if let Ok(s) = core::str::from_utf8(&data[..n])
                && usb.process_input(&mut msg, s).await.is_none()
            {
                continue;
            }
            match parse::<(), Commands>(&msg, ()) {
                Ok(cmd) => {
                    match cmd {
                        Commands::GetReportCadence => {
                            let cadence =
                                crate::reporter::UPDATE_CADENCE_MS.load(Ordering::Relaxed);
                            usb.report_ok("Current report cadence (ms)", cadence).await;
                        }
                        Commands::SetReportCadence(new_cadence) => {
                            crate::reporter::UPDATE_CADENCE_MS
                                .store(new_cadence, Ordering::Relaxed);
                            usb.report_ok("Updated report cadence (ms)", new_cadence)
                                .await;
                        }
                        Commands::Help => {
                            let help_message = "Available commands:\r\n\
                            \t- read-dac <dac> <channel>: Read the value from the specified DAC and channel\r\n\
                            \t- write-dac <dac> <channel> <value>: Write the specified value to the specified DAC and channel.\r\n\
                            \t\tValue is an unsigned integer in millivolts.\r\n\
                            \t- enable-outputs: Enable the DAC outputs\r\n\
                            \t- disable-outputs: Disable the DAC outputs\r\n\
                            \t- all-off: Disable outputs and set all DAC channels to 0\r\n\
                            \t- help: relm-async-componenthow this help message\r\n\
                            Note: \r\n\
                            \t<dac> can be dac0, dac1, or dac2 (dac2 is not implemented)\r\n\
                            \t<channel> can be a, b, c, d, e, f, g, h, and all\r\n\
                            \t<value> should be a 16-bit decimal value (e.g. 32767)\r\n";
                            usb.write_message(help_message.as_bytes()).await;
                        }
                        _ => {
                            if !DAC_READY.load(Ordering::SeqCst) {
                                usb.report_err("DAC not ready", "Unable to process command at this time, please try again later").await;
                            } else if sender.try_send(cmd).is_err() {
                                usb.report_err("Command channel full", "Unable to process command at this time, please try again later").await;
                            } else {
                                let (message, value) = receiver.receive().await;
                                usb.respond(message, value).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    let mut err = String::<512>::new();
                    core::write!(&mut err, "{:?}\r\n", e).ok();
                    usb.report_err("Failed to parse command", err.as_str())
                        .await;
                }
            }
            msg.clear();
            usb.write_message(b"\r\n> ").await;
        }
    }
}

#[embassy_executor::task]
pub async fn cdc_tlm_task(usb: &'static mut CdcAcmDevice, receiver: MeasurementReceiver) {
    'main: loop {
        usb.wait_connection().await;
        loop {
            let measurement = receiver.receive().await;
            if !usb.rts() {
                trace!("USB disconnected");
                continue;
            }
            let mut output = String::<256>::new();
            core::write!(
                &mut output,
                "{},{},{},{},{}\r\n",
                measurement.source,
                measurement.voltage,
                measurement.current,
                measurement.power,
                measurement.shunt,
            )
            .ok();
            if let Err(e) = usb.write_packet(output.as_bytes()).await {
                match e {
                    embassy_usb::driver::EndpointError::BufferOverflow => {
                        error!("Failed to send telemetry: Buffer overflow");
                        log::error!("Failed to send telemetry: Buffer overflow");
                    }
                    embassy_usb::driver::EndpointError::Disabled => continue 'main,
                }
            }
        }
    }
}

#[embassy_executor::task]
pub async fn usb_device_task(dev: UsbDeviceDriver) {
    let mut dev = dev;
    dev.run().await;
}

#[embassy_executor::task]
pub async fn cdc_log_task(cdc: CdcAcmDevice) {
    embassy_usb_logger::with_class!(1024, log::LevelFilter::Info, cdc).await;
}

trait AcmDeviceFunctions {
    async fn write_message(&mut self, bytes: &[u8]);
    async fn process_input<const N: usize>(
        &mut self,
        msg: &mut String<N>,
        input: &str,
    ) -> Option<()>;
    async fn respond<T: core::fmt::Debug + defmt::Format, E: core::fmt::Debug + defmt::Format>(
        &mut self,
        message: &str,
        value: Result<T, E>,
    );
    async fn report_ok<T: core::fmt::Debug + defmt::Format>(&mut self, message: &str, value: T);
    async fn report_err<E: core::fmt::Debug + defmt::Format>(&mut self, message: &str, error: E);
}

impl AcmDeviceFunctions for CdcAcmDevice {
    async fn write_message(&mut self, s: &[u8]) {
        let iter = s.chunks(32);
        for chunk in iter {
            let _ = self.write_packet(chunk).await;
        }
    }

    async fn process_input<const N: usize>(
        &mut self,
        msg: &mut String<N>,
        input: &str,
    ) -> Option<()> {
        let mut skip = 0;
        for c in input.chars() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            if c.is_control() {
                if c == '\x08' || c == '\x7f' {
                    // backspace or delete
                    if msg.pop().is_some() {
                        // Move cursor back, print space, move cursor back again
                        self.write_message(b"\x08 \x08").await;
                    }
                } else if c == '\n' || c == '\r' {
                    // end of command
                    if !msg.is_empty() {
                        trace!("Received command: {}", msg);
                        self.write_message(b"\r\n").await; // extra newline after echo for readability
                        return Some(());
                    } else {
                        self.write_message(b"\r\n> ").await; // prompt for next command
                    }
                } else if c == '\x1b' {
                    // Ignore other control characters
                    skip = 2; // rudimentary way to skip ANSI escape sequences
                    continue;
                }
            } else if msg.push(c).is_err() {
                self.write_message(("Error: message too large, discarding.\r\n").as_bytes())
                    .await;
                msg.clear();
                self.write_message(b"> ").await; // extra newline after echo for readability
            } else {
                let mut buf = [0u8; 4];
                self.write_message(c.encode_utf8(&mut buf).as_bytes()).await; // echo back to tty0
            }
        }
        None
    }

    async fn respond<T: core::fmt::Debug + defmt::Format, E: core::fmt::Debug + defmt::Format>(
        &mut self,
        message: &str,
        value: Result<T, E>,
    ) {
        let mut output = heapless::String::<4096>::new();
        match value {
            Ok(v) => {
                log::trace!("{}: {:?}", message, v);
                trace!("{}: {:?}", message, v);
                core::write!(&mut output, "OK : {}: {:?}\r\n", message, v).ok();
            }
            Err(e) => {
                log::error!("{}: {:?}", message, e);
                error!("{}: {:?}", message, e);
                core::write!(&mut output, "ERR: {}: {:?}\r\n", message, e).ok();
            }
        }
        self.write_message(output.as_bytes()).await;
    }

    async fn report_ok<T: core::fmt::Debug + defmt::Format>(&mut self, message: &str, value: T) {
        self.respond::<_, ()>(message, Ok(value)).await;
    }

    async fn report_err<E: core::fmt::Debug + defmt::Format>(&mut self, message: &str, error: E) {
        self.respond::<(), E>(message, Err(error)).await;
    }
}
