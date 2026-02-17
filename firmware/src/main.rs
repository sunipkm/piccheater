#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::{Executor, Spawner};
use embassy_rp::{
    gpio::Output,
    multicore::{Stack, spawn_core1},
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

mod commands;
mod reporter;
mod resources;
mod usb;

use crate::{
    reporter::report_spawner,
    resources::{AssignedResources, I2cSnsDev, LedDev, UsbDacDev},
    usb::usb_task,
};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let resources = split_resources!(p);
    info!("Main core started");
    // Set up data transmission channel
    static DATA_CHANNEL: StaticCell<MeasurementChannel> = StaticCell::new();
    let channel = DATA_CHANNEL.init(Channel::new());
    let sender = channel.sender();
    let receiver = channel.receiver();
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
                if let Err(e) = spawner.spawn(led_task(resources.led)) {
                    log::error!("Failed to spawn LED task: {:?}", e);
                    error!("Failed to spawn LED task: {:?}", e);
                }
                report_spawner(&spawner, resources.i2csns, sender);
            }
        })
    });
    info!("Core 1 started and tasks spawned");
    // Spawn the USB task on the main core
    usb_task(&spawner, resources.usbdac, receiver);
    info!("USB task spawned on main core");
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
    pub source: u8,
    pub voltage: u32,
    pub current: i32,
    pub power: u32,
}

pub type MeasurementChannel = Channel<CriticalSectionRawMutex, Measurement, 8>;
pub type MeasurementSender = Sender<'static, CriticalSectionRawMutex, Measurement, 8>;
pub type MeasurementReceiver = Receiver<'static, CriticalSectionRawMutex, Measurement, 8>;
