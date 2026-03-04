// use core::sync::atomic::{AtomicBool, Ordering};
use core::fmt::Write;
use dacx578::{Address, AsyncFunctions as _, DacX578, ResetMode};
use defmt::*;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
// use embassy_futures::select::{Either, select};
use embassy_rp::{
    bind_interrupts,
    gpio::Output,
    i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cIrqHandler},
    peripherals::{I2C0, USB},
    usb::{Driver as UsbDriver, InterruptHandler as UsbIrqHandler},
    // watchdog::Watchdog,
};
use embassy_sync::mutex::Mutex;
// use embassy_time::{Duration, Timer};
use embassy_usb::{
    Builder, Config as UsbConfig, UsbDevice,
    class::cdc_acm::{CdcAcmClass, State as CdcAcmState},
};
use heapless::String;
use kmdparse::parse;
use static_cell::StaticCell;
use uom::{
    ConstZero,
    si::{electric_potential::millivolt, f32::ElectricPotential},
};

use crate::{
    MeasurementReceiver,
    commands::{Commands, Dacs},
    resources::{StaticI2cBus, UsbDacDev},
};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbIrqHandler<USB>;
    I2C0_IRQ => I2cIrqHandler<I2C0>;
});

// /// Signal to sensor tasks to shut down
// pub(crate) static SHUTDOWN: AtomicBool = AtomicBool::new(false);

type CdcAcmDevice = CdcAcmClass<'static, UsbDriver<'static, USB>>;
type UsbDeviceDriver = UsbDevice<'static, UsbDriver<'static, USB>>;
// type SharedI2cBus = I2cDevice<'static, NoopRawMutex, I2c<'static, I2C0, I2cAsync>>;
// type StaticInput = Input<'static>;
type StaticOutput = Output<'static>;
// type StaticInputRef = &'static mut Input<'static>;
// type StaticOutputRef = &'static mut Output<'static>;

pub fn usb_task(spawner: &Spawner, dev: UsbDacDev, receiver: MeasurementReceiver) {
    // Allocate static memory for the USB device and related state
    static USB_DEVICE: StaticCell<UsbDeviceDriver> = StaticCell::new();
    static CDC_CONF_STATE: StaticCell<CdcAcmState> = StaticCell::new();
    static CDC_TLM_STATE: StaticCell<CdcAcmState> = StaticCell::new();
    static CDC_LOG_STATE: StaticCell<CdcAcmState> = StaticCell::new();
    static CDC_DEVICE: StaticCell<CdcAcmDevice> = StaticCell::new();
    static TLM_DEVICE: StaticCell<CdcAcmDevice> = StaticCell::new();
    static CONF_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    // Create the USB driver and attach interrupts
    let driver = UsbDriver::new(dev.usb, Irqs);
    trace!("USB driver created");
    // Create the USB device configuration
    let mut config = UsbConfig::new(0xc001, 0xfee1);
    config.manufacturer = Some("LoCSST/PIC-D");
    config.product = Some("PIC-D Heater DAC Rev.0");
    config.serial_number = Some("2026-0001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    trace!("USB configuration created");

    // Allocate static buffers for USB descriptors and control transfers
    let conf_desc = CONF_DESC.init([0; 256]);
    let bos_desc = BOS_DESC.init([0; 256]);
    let ctrl_buf = CTRL_BUF.init([0; 64]);

    // Initialize the CDC ACM class state for both interfaces
    let state_conf = CDC_CONF_STATE.init(CdcAcmState::new());
    let state_tlm = CDC_TLM_STATE.init(CdcAcmState::new());
    let state_log = CDC_LOG_STATE.init(CdcAcmState::new());

    // USB builder to construct the device with the specified configuration and classes
    let mut usb_builder = Builder::new(driver, config, conf_desc, bos_desc, &mut [], ctrl_buf);
    trace!("USB builder created");

    // Initialize the CDC ACM classes for both the configuration and telemetry interfaces
    let cdc_conf = CDC_DEVICE.init(CdcAcmClass::new(&mut usb_builder, state_conf, 64));
    let cdc_tlm = TLM_DEVICE.init(CdcAcmClass::new(&mut usb_builder, state_tlm, 64));

    // Set up USB logging
    let cdc_log = CdcAcmClass::new(&mut usb_builder, state_log, 64);
    if let Err(e) = spawner.spawn(cdc_log_task(cdc_log)) {
        error!("Failed to spawn CDC log task: {:?}", e);
    } else {
        trace!("CDC log task spawned");
    }

    // Build the USB device
    let usb: UsbDevice<'_, UsbDriver<'_, USB>> = usb_builder.build();

    // Store the USB device in a static cell for handoff to the USB task in embassy executor
    let usb_dev = USB_DEVICE.init(usb);

    // Create DAC devices
    static I2C_BUS: StaticCell<StaticI2cBus<I2C0>> = StaticCell::new();
    static AMP_EN: StaticCell<StaticOutput> = StaticCell::new();

    // Initialize the shared I2C bus and DAC control pins
    let i2c_config = I2cConfig::default();
    let i2c = Mutex::new(I2c::new_async(dev.i2c, dev.scl, dev.sda, Irqs, i2c_config));
    let amp_en = Output::new(dev.en, embassy_rp::gpio::Level::Low);

    // Store the I2C bus and control pins in static cells for use in the USB task
    let i2c_bus = I2C_BUS.init(i2c);
    let amp_en_pin = AMP_EN.init(amp_en);

    // Spawn the USB device task and CDC ACM tasks in the embassy executor
    if let Err(e) = spawner.spawn(usb_device_task(usb_dev)) {
        error!("Failed to spawn USB device task: {:?}", e);
        log::error!("Failed to spawn USB device task: {:?}", e);
    } else {
        trace!("USB device task spawned");
    }
    if let Err(e) = spawner.spawn(cdc_conf_task(cdc_conf, i2c_bus, amp_en_pin)) {
        error!("Failed to spawn CDC config task: {:?}", e);
        log::error!("Failed to spawn CDC config task: {:?}", e);
    } else {
        trace!("CDC configuration input task spawned");
    }
    if let Err(e) = spawner.spawn(cdc_tlm_task(cdc_tlm, receiver)) {
        error!("Failed to spawn CDC telemetry task: {:?}", e);
        log::error!("Failed to spawn CDC telemetry task: {:?}", e);
    } else {
        trace!("CDC telemetry task spawned");
    }
}

#[embassy_executor::task]
pub async fn cdc_conf_task(
    usb: &'static mut CdcAcmDevice,
    i2c: &'static mut StaticI2cBus<I2C0>,
    dac_en: &'static mut StaticOutput,
) {
    {
        const ADDR_LEN: usize = dacx578::Address::address_range().len();
        let mut addrs = heapless::Vec::<u8, ADDR_LEN>::new();
        {
            let mut i2c = i2c.lock().await;
            for addr in dacx578::Address::address_range() {
                i2c.blocking_read(addr, &mut [0; 1])
                    .is_ok()
                    .then(|| addrs.push(addr).ok());
            }
        }
        trace!("I2C 0> Found {} devices: {:#02x}", addrs.len(), addrs);
    }
    let mut dac0 = DacX578::new(
        I2cDevice::new(i2c),
        Address::PinHigh,
        ElectricPotential::new::<millivolt>(2048.0),
    );
    let mut dac1 = DacX578::new(
        I2cDevice::new(i2c),
        Address::PinLow,
        ElectricPotential::new::<millivolt>(2048.0),
    );
    if let Err(e) = dac0.reset(ResetMode::Por).await {
        log::error!("Failed to reset DAC0: {:?}", e);
        error!("Failed to reset DAC0: {:?}", e);
        return;
    }
    if let Err(e) = dac1.reset(ResetMode::Por).await {
        log::error!("Failed to reset DAC1: {:?}", e);
        error!("Failed to reset DAC1: {:?}", e);
        return;
    }

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
                        Commands::ReadDac { dac, channel } => {
                            match dac {
                                Dacs::Dac0 => {
                                    let value =
                                        dac0.read(dacx578::Register::ChannelDac(channel)).await;
                                    usb.respond("Read from DAC0", value).await;
                                }
                                Dacs::Dac1 => {
                                    let value =
                                        dac1.read(dacx578::Register::ChannelDac(channel)).await;
                                    usb.respond("Read from DAC1", value).await;
                                }
                                Dacs::Dac2 => {
                                    // DAC2 is not implemented in this example, but you could add it similarly to DAC0 and DAC1
                                    trace!("DAC2 read not implemented");
                                    usb.report_err("DAC2", "Not implemented").await;
                                }
                            }
                        }
                        Commands::WriteDac {
                            dac,
                            channel,
                            value,
                        } => match dac {
                            Dacs::Dac0 => {
                                let res = dac0
                                    .write_and_update(
                                        channel,
                                        ElectricPotential::new::<millivolt>(value),
                                    )
                                    .await;
                                usb.respond("Write to DAC0", res).await;
                            }
                            Dacs::Dac1 => {
                                let res = dac1
                                    .write_and_update(
                                        channel,
                                        ElectricPotential::new::<millivolt>(value),
                                    )
                                    .await;
                                usb.respond("Write to DAC1", res).await;
                            }
                            Dacs::Dac2 => {
                                // DAC2 is not implemented in this example, but you could add it similarly to DAC0 and DAC1
                                trace!("DAC2 write not implemented");
                                usb.report_err("DAC2", "Not implemented").await;
                            }
                        },
                        Commands::EnableOutputs => {
                            dac_en.set_high();
                            usb.report_ok("Enable AMP EN pin", ()).await;
                        }
                        Commands::DisableOutputs => {
                            dac_en.set_low();
                            usb.report_ok("Disable AMP EN pin", ()).await;
                        }
                        Commands::AllOff => {
                            dac_en.set_low();
                            let r1 = dac0
                                .write_and_update(dacx578::Channel::All, ElectricPotential::ZERO)
                                .await
                                .err();
                            let r2 = dac1
                                .write_and_update(dacx578::Channel::All, ElectricPotential::ZERO)
                                .await
                                .err();
                            usb.report_ok("All outputs off", (r1.is_none(), r2.is_none()))
                                .await;
                        }
                        Commands::GetReportCadence => {
                            let cadence = crate::reporter::UPDATE_CADENCE_MS
                                .load(core::sync::atomic::Ordering::Relaxed);
                            usb.report_ok("Current report cadence (ms)", cadence).await;
                        }
                        Commands::SetReportCadence(new_cadence) => {
                            crate::reporter::UPDATE_CADENCE_MS
                                .store(new_cadence, core::sync::atomic::Ordering::Relaxed);
                            usb.report_ok("Updated report cadence (ms)", new_cadence)
                                .await;
                        }
                        Commands::Help => {
                            let help_message = "Available commands:\r\n\
                            \t- read-dac <dac> <channel>: Read the value from the specified DAC and channel\r\n\
                            \t- write-dac <dac> <channel> <value>: Write the specified value to the specified DAC and channel.\r\n\
                            \t\tValue is an unsigned integer in millivolts.
                            \t- enable-outputs: Enable the DAC outputs\r\n\
                            \t- disable-outputs: Disable the DAC outputs\r\n\
                            \t- all-off: Disable outputs and set all DAC channels to 0\r\n\
                            \t- help: Show this help message\r\n\
                            Note: \r\n
                            \t<dac> can be dac0, dac1, or dac2 (dac2 is not implemented)\r\n\
                            \t<channel> can be a, b, c, d, e, f, g, h, and all\r\n\
                            \t<value> should be a 16-bit decimal value (e.g. 32767)\r\n";
                            usb.write_message(help_message.as_bytes()).await;
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
                "{},{},{},{}\r\n",
                measurement.source,
                measurement.voltage,
                measurement.current,
                measurement.power
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
pub async fn usb_device_task(dev: &'static mut UsbDeviceDriver) {
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
