#![no_std]
#![warn(missing_docs)]
//! [`embedded-hal`] driver for the INA233 power monitor.

use crate::interface::I2cInterface;
use crate::registers::Calibration;
pub use uom::si::f32::ElectricCurrent;
pub use uom::si::f32::ElectricalResistance;

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

impl AddrPin {
    /// Returns the valid I2C address range for the INA233 based on the address pin configuration.
    pub const fn address_range() -> core::ops::RangeInclusive<u8> {
        0x40..=0x4f
    }
}

impl From<u8> for AddrPin {
    fn from(addr: u8) -> Self {
        match addr & 0b11 {
            0 => AddrPin::Gnd,
            1 => AddrPin::Vdd,
            2 => AddrPin::Sda,
            _ => AddrPin::Scl,
        }
    }
}

/// INA233 I2C driver
pub struct Ina233<I2C, D> {
    i2c: I2cInterface<I2C>,
    delay: D,
    lsb: ElectricCurrent,
    calibration: Calibration,
}

impl<I2C, D> Ina233<I2C, D> {
    /// Get the I2C address of the INA233 device.
    pub const fn address(&self) -> u8 {
        self.i2c.address
    }
}

/// INA233 Errors
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<I2CError> {
    /// I2C bus error
    I2C(I2CError),
    /// Device identification error
    DeviceId,
    /// Calibration read-back did not match the written value
    CalibrationMismatch,
    /// ADC configuration read-back did not match the written value
    ConfigurationMismatch,
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
pub use crate::registers::{AdcConfig, AdcMode, Averages, ConversionTime};

#[cfg(feature = "async")]
mod async_impl;
#[cfg(feature = "sync")]
mod blocking_impl;
mod configuration;
mod interface;
mod registers;
