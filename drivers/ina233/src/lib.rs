#![no_std]
#![warn(missing_docs)]
//! [`embedded-hal`] driver for the INA233 power monitor.

use crate::interface::I2cInterface;
pub use uom::si::f32::ElectricCurrent;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// INA233 I2C address pin configuration
pub enum AddrPin {
    /// Address pin grounded.
    #[default]
    Gnd = 0b00,
    /// Address pin connected to Vdd.
    Vdd = 0b01,
    /// Address pin connected to SDA.
    Sda = 0b10,
    /// Address pin connected to SCL.
    Scl = 0b11,
}

/// INA233 I2C driver
pub struct Ina233<I2C, D> {
    i2c: I2cInterface<I2C>,
    delay: D,
    current_lsb: ElectricCurrent,
}

/// INA233 Errors
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<I2CError> {
    /// I2C bus error
    I2C(I2CError),
    /// Device identification error
    DeviceId,
}

impl<I2CError> From<I2CError> for Error<I2CError> {
    fn from(err: I2CError) -> Self {
        Error::I2C(err)
    }
}

#[cfg(feature = "async")]
pub use crate::async_impl::AsyncInterface;
#[cfg(feature = "sync")]
pub use crate::blocking_impl::SyncInterface;
pub use crate::configuration::{Configuration, ConfigurationBuilder};

mod async_impl;
mod blocking_impl;
mod configuration;
mod interface;
mod registers;
