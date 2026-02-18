use core::sync::atomic::AtomicU32;

use defmt::{error, info};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cIrqHandler},
    peripherals::I2C1,
};
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Duration, Ticker};
use ina233::{AdcConfig, AsyncInterface as _, ConfigurationBuilder, Ina233};
use static_cell::StaticCell;
use uom::si::{
    electric_current::{ampere, microampere, milliampere},
    electric_potential::{millivolt, volt},
    electrical_resistance::milliohm,
    f32::{ElectricCurrent, ElectricalResistance},
    power::{milliwatt, watt},
};

use crate::{
    Measurement, MeasurementSender,
    resources::{I2cSnsDev, StaticI2cBus},
};

bind_interrupts!(struct Irqs {
    I2C1_IRQ => I2cIrqHandler<I2C1>;
});

pub static UPDATE_CADENCE_MS: AtomicU32 = AtomicU32::new(1000);

pub fn report_spawner(spawner: &Spawner, dev: I2cSnsDev, sender: MeasurementSender) {
    static I2C_BUS: StaticCell<StaticI2cBus<I2C1>> = StaticCell::new();
    let i2c_config = I2cConfig::default();
    let i2c_bus = I2C_BUS.init(Mutex::new(I2c::new_async(
        dev.i2c, dev.scl, dev.sda, Irqs, i2c_config,
    )));
    info!("Spawning I2C report task");
    if let Err(e) = spawner.spawn(i2c_report_task(i2c_bus, sender)) {
        log::error!("Failed to spawn I2C report task: {:?}", e);
        info!("Failed to spawn I2C report task: {:?}", e);
    } else {
        info!("I2C report task spawned successfully");
    }
}

#[embassy_executor::task]
pub async fn i2c_report_task(i2c_bus: &'static StaticI2cBus<I2C1>, sender: MeasurementSender) {
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut addrs = heapless::Vec::<u8, 16>::new();
    {
        let mut i2c = i2c_bus.lock().await;
        for addr in ina233::AddrPin::address_range() {
            i2c.blocking_read(addr, &mut [0; 1])
                .is_ok()
                .then(|| addrs.push(addr).ok());
        }
    }
    let mut sensors = heapless::Vec::<Ina233<_, _>, 16>::new();
    for addr in addrs.iter() {
        let config = ConfigurationBuilder::default()
            .address(*addr)
            .current_lsb(ElectricCurrent::new::<microampere>(96.0))
            .adc_config(
                AdcConfig::default()
                    .with_vbus_conv_time(ina233::ConversionTime::Ms4_156)
                    .with_vshunt_conv_time(ina233::ConversionTime::Ms4_156),
            )
            .build(ElectricalResistance::new::<milliohm>(20.0));
        let i2c_bus = I2cDevice::new(i2c_bus);
        match Ina233::new_async(i2c_bus, Delay, config).await {
            Ok(s) => {
                info!("Initialized INA233 at address {:#02x}", addr);
                sensors.push(s).ok()
            }
            Err(e) => {
                log::error!(
                    "Failed to initialize INA233 at address {:#02x}: {:?}",
                    addr,
                    e
                );
                error!(
                    "Failed to initialize INA233 at address {:#02x}: {:?}",
                    addr, e
                );
                continue;
            }
        };
    }
    loop {
        for sensor in sensors.iter_mut() {
            match sensor.read().await {
                Ok((current, voltage)) => {
                    info!(
                        "Sensor at {:#02x}: Current = {} A, Voltage = {} V, Power = {} W",
                        sensor.address(),
                        current.get::<ampere>(),
                        voltage.get::<volt>(),
                        (current * voltage).get::<watt>()
                    );
                    let power = (current.abs() * voltage).get::<milliwatt>() as u32;
                    let current = current.get::<milliampere>() as i32;
                    let voltage = voltage.get::<millivolt>() as u32;
                    if sender
                        .try_send(Measurement {
                            source: sensor.address(),
                            voltage,
                            current,
                            power,
                        })
                        .is_err()
                    {
                        error!(
                            "Failed to send measurement from sensor at {:#02x}: Channel full",
                            sensor.address()
                        );
                    }
                }
                Err(e) => {
                    log::error!(
                        "Failed to read from sensor at {:#02x}: {:?}",
                        sensor.address(),
                        e
                    );
                    error!(
                        "Failed to read from sensor at {:#02x}: {:?}",
                        sensor.address(),
                        e
                    );
                }
            }
        }
        ticker.next().await;
    }
}
