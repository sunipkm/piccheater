use core::sync::atomic::AtomicU32;

use defmt::{debug, error, trace};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    i2c::{Config as I2cConfig, I2c, InterruptHandler as I2cIrqHandler},
    peripherals::I2C1,
};
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Duration, Ticker};
use ina233::{AdcConfig, AsyncInterface, ConfigurationBuilder, Ina233};
use static_cell::StaticCell;
use uom::si::{
    electric_current::{ampere, microampere, milliampere},
    electric_potential::{microvolt, millivolt, volt},
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
    trace!("Spawning I2C report task");
    if let Err(e) = spawner.spawn(i2c_report_task(i2c_bus, sender)) {
        log::error!("Failed to spawn I2C report task: {:?}", e);
        trace!("Failed to spawn I2C report task: {:?}", e);
    } else {
        trace!("I2C report task spawned successfully");
    }
}

#[embassy_executor::task]
pub async fn i2c_report_task(i2c_bus: &'static StaticI2cBus<I2C1>, sender: MeasurementSender) {
    trace!("I2C report task started");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut addrs = heapless::Vec::<u8, 16>::new();
    let mut sensors = heapless::Vec::<Ina233<_, _>, 16>::new();
    'main: loop {
        addrs.clear();
        sensors.clear();
        while addrs.is_empty() {
            let mut i2c = i2c_bus.lock().await;
            for addr in ina233::AddrPin::address_range() {
                i2c.blocking_read(addr, &mut [0; 1])
                    // .inspect_err(|e| {
                    //     error!("I2C read error at address {:#02x}: {:?}", addr, e);
                    // })
                    .is_ok()
                    .then(|| addrs.push(addr).ok());
            }
            trace!(
                "[INA233] Found {} devices at addresses: {:?}",
                addrs.len(),
                addrs.as_slice(),
            );
            log::trace!(
                "[INA233] Found {} devices at addresses: {:?}",
                addrs.len(),
                addrs.as_slice(),
            );
            ticker.next().await;
        }
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
                    debug!("Initialized INA233 at address {:#02x}", addr);
                    sensors.push(s).ok()
                }
                Err(e) => {
                    log::error!("[INA233@{:#02x}] Failed to initialize: {:?}", addr, e);
                    error!("[INA233@{:#02x}] Failed to initialize: {:?}", addr, e);
                    continue 'main;
                }
            };
        }
        loop {
            for sensor in sensors.iter_mut() {
                match sensor.read().await {
                    Ok((current, voltage)) => {
                        let shunt = sensor.read_shunt().await.ok();
                        debug!(
                            "[INA233@{:#02x}] Current = {} A, Voltage = {} V, Power = {} W, Shunt = {} uV",
                            sensor.address(),
                            current.get::<ampere>(),
                            voltage.get::<volt>(),
                            (current * voltage).get::<watt>(),
                            shunt.map(|v| v.get::<microvolt>()).unwrap_or(-f32::NAN),
                        );
                        let power = (current.abs() * voltage).get::<milliwatt>() as u32;
                        let current = current.get::<milliampere>() as i32;
                        let voltage = voltage.get::<millivolt>() as u32;
                        let shunt = shunt.map(|v| v.get::<microvolt>() as i32).unwrap_or(0);
                        if sender
                            .try_send(Measurement {
                                source: sensor.address(),
                                voltage,
                                current,
                                power,
                                shunt,
                            })
                            .is_err()
                        {
                            trace!(
                                "[INA233@{:#02x}] Failed to send measurement: Channel full",
                                sensor.address()
                            );
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[INA233@{:#02x}] Failed to read from sensor: {:?}",
                            sensor.address(),
                            e
                        );
                        error!(
                            "[INA233@{:#02x}] Failed to read from sensor: {:?}",
                            sensor.address(),
                            e
                        );
                        continue 'main;
                    }
                }
            }
            ticker.next().await;
        }
    }
}
