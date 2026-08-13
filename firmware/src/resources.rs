use assign_resources::assign_resources;
use embassy_rp::{
    Peri,
    i2c::{Async as I2cAsync, I2c},
    peripherals,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};

assign_resources! {
    usb: UsbDev {
        usb: USB,
        // ldac: PIN_18,
        // clr: PIN_19,
    }
    dac: DacDev {
        i2c: I2C0,
        scl: PIN_21,
        sda: PIN_20,
        en: PIN_17,
        // ldac: PIN_18,
        // clr: PIN_19,
    }
    i2csns: I2cSnsDev {
        i2c: I2C1,
        scl: PIN_3,
        sda: PIN_2,
    }
    led: LedDev {
        pin: PIN_25,
    }
}

pub type StaticI2cBus<T> = Mutex<NoopRawMutex, I2c<'static, T, I2cAsync>>;
