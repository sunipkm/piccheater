#![no_std]
#![warn(missing_docs)]
//! [`embedded-hal`] driver for the INA233 power monitor.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
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
pub struct Ina233<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C> Ina233<I2C> {
    /// Create a new INA233 driver instance.
    ///
    /// # Arguments
    /// - `i2c` - I2C peripheral implementing the embedded-hal I2C traits.
    /// - `a0` - INA233 address pin A0 configuration.
    /// - `a1` - INA233 address pin A1 configuration.
    pub fn new(i2c: I2C, a0: AddrPin, a1: AddrPin) -> Self {
        let base_address = 0x40;
        let address = base_address | (a1 as u8) << 2 | (a0 as u8);
        Self { i2c, address }
    }
}

mod registers;
mod blocking_impl;
mod async_impl;