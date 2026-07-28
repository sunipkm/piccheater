#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::{Executor, Spawner};
use embassy_rp::{
    gpio::Output,
    multicore::{Stack, spawn_core1},
};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Duration, Timer};
use heapless::String;
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

mod commands;
mod dac;
mod reporter;
mod resources;
mod usb;

use crate::{
    commands::Commands,
    dac::dac_task,
    reporter::report_spawner,
    resources::{AssignedResources, DacDev, I2cSnsDev, LedDev, UsbDev},
    usb::usb_task,
};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let resources = split_resources!(p);
    info!("Main core started");
    // Set up data transmission channel
    static RPT_CHANNEL: StaticCell<MeasurementChannel> = StaticCell::new();
    let rpt_channel = RPT_CHANNEL.init(Channel::new());
    let (rpt_sender, rpt_receiver) = (rpt_channel.sender(), rpt_channel.receiver());
    // Set up DAC and USB channels
    static CMD_CHANNEL: StaticCell<CommandChannel> = StaticCell::new();
    static RESP_CHANNEL: StaticCell<ResponseChannel> = StaticCell::new();
    let cmd_channel = CMD_CHANNEL.init(Channel::new());
    let resp_channel = RESP_CHANNEL.init(Channel::new());
    let (cmd_sender, cmd_receiver) = (cmd_channel.sender(), cmd_channel.receiver());
    let (resp_sender, resp_receiver) = (resp_channel.sender(), resp_channel.receiver());

    // Spawn the USB task on the main core
    usb_task(
        &spawner,
        resources.usb,
        rpt_receiver,
        cmd_sender,
        resp_receiver,
    );
    trace!("USB task spawned on main core");

    Timer::after_secs(2).await; // Wait for the USB task to initialize and enumerate

    // Set up stack and executor for core 1
    const STACK_SIZE: usize = 128 * 1024; // 128 KB stack for core 1
    static CORE1_STACK: StaticCell<Stack<STACK_SIZE>> = StaticCell::new();
    static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();
    let stack = CORE1_STACK.init(Stack::new());
    // Spawn the LED task and the I2C reporting task on core 1
    spawn_core1(p.CORE1, stack, move || {
        let exec = CORE1_EXECUTOR.init(Executor::new());
        exec.run({
            move |spawner| {
                // Spawn the LED task
                match led_task(resources.led) {
                    Ok(t) => {
                        spawner.spawn(t);
                        trace!("LED task spawned");
                    }
                    Err(e) => {
                        log::error!("Failed to spawn LED task: {:?}", e);
                        error!("Failed to spawn LED task: {:?}", e);
                    }
                }
                report_spawner(&spawner, resources.i2csns, rpt_sender);
            }
        })
    });
    trace!("Core 1 started and tasks spawned");

    // Spawn the DAC control task, which will handle commands from the USB configuration interface and control the DAC outputs accordingly
    match dac_task(resources.dac, cmd_receiver, resp_sender) {
        Ok(t) => {
            spawner.spawn(t);
            trace!("DAC control task spawned");
        }
        Err(e) => {
            error!("Failed to spawn DAC control task: {:?}", e);
            log::error!("Failed to spawn DAC control task: {:?}", e);
        }
    }
}

#[embassy_executor::task]
async fn led_task(led: LedDev) {
    let mut pin = Output::new(led.pin, embassy_rp::gpio::Level::Low);
    loop {
        pin.set_high();
        Timer::after(Duration::from_millis(500)).await;
        pin.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}

pub struct Measurement {
    /// I2C address of the sensor
    pub source: u8,
    /// Voltage in millivolts
    pub voltage: u32,
    /// Current in milliamps, can be negative
    pub current: i32,
    /// Power in milliwatts
    pub power: u32,
    /// Shunt voltage in microvolts, can be negative
    pub shunt: i32,
}

pub type MeasurementChannel = Channel<CriticalSectionRawMutex, Measurement, 8>;
pub type MeasurementSender = Sender<'static, CriticalSectionRawMutex, Measurement, 8>;
pub type MeasurementReceiver = Receiver<'static, CriticalSectionRawMutex, Measurement, 8>;

pub type CommandChannel = Channel<NoopRawMutex, Commands, 1>;
pub type CommandSender = Sender<'static, NoopRawMutex, Commands, 1>;
pub type CommandReceiver = Receiver<'static, NoopRawMutex, Commands, 1>;

pub type Response = (&'static str, Result<String<64>, String<256>>);
pub type ResponseChannel = Channel<NoopRawMutex, Response, 1>;
pub type ResponseSender = Sender<'static, NoopRawMutex, Response, 1>;
pub type ResponseReceiver = Receiver<'static, NoopRawMutex, Response, 1>;
