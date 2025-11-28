#![no_std]
#![deny(missing_docs)]

//! Driver for the DACx578 series Digital to Analog Converters (DACs) over I2C.
//! DAC5578 (8-bit), DAC6578 (10-bit), and DAC7578 (12-bit) devices are supported.

use bitfield_struct::bitfield;

mod async_impl;
mod blocking_impl;
mod details;

pub use async_impl::AsyncFunctions;
pub use blocking_impl::SyncFunctions;

/// user_address can be set by pulling the ADDR0 pin high/low or leave it floating
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Address {
    /// ADDR0 is low
    PinLow = 0x48,
    /// ADDR0 is high
    PinHigh = 0x4a,
    /// ADDR0 is floating
    PinFloat = 0x4c,
}

/// Defines the output channel to set the voltage for
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Channel {
    /// DAC output channel A
    A,
    /// DAC output channel B
    B,
    /// DAC output channel C
    C,
    /// DAC output channel D
    D,
    /// DAC output channel E
    E,
    /// DAC output channel F
    F,
    /// DAC output channel G
    G,
    /// DAC output channel H
    H,
    /// Targets all DAC output channels
    All = 0xf,
}

#[bitfield(u8)]
/// Struct to represent the state of all channels
pub struct Channels {
    #[allow(non_snake_case)]
    /// DAC output channel A
    pub A: bool,
    #[allow(non_snake_case)]
    /// DAC output channel B
    pub B: bool,
    #[allow(non_snake_case)]
    /// DAC output channel C
    pub C: bool,
    #[allow(non_snake_case)]
    /// DAC output channel D
    pub D: bool,
    #[allow(non_snake_case)]
    /// DAC output channel E
    pub E: bool,
    #[allow(non_snake_case)]
    /// DAC output channel F
    pub F: bool,
    #[allow(non_snake_case)]
    /// DAC output channel G
    pub G: bool,
    #[allow(non_snake_case)]
    /// DAC output channel H
    pub H: bool,
}

/// The type of the command to send for a Command
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CommandType {
    /// Write to the channel's DAC input register
    Write = 0x0,
    /// Selects DAC channel to be updated
    Update = 0x10,
    /// Write to DAC input register for a channel and update channel DAC register
    WriteUpdate = 0x30,
    /// Write to Selected DAC Input Register and Update All DAC Registers (Global Software LDAC)
    WriteUpdateAll = 0x20,
}

/// Read from the specified register
#[derive(Debug, Clone, Copy)]
pub enum Register {
    /// Read the value from the specified channel input register
    ChannelInput(Channel),
    /// Read the value from the specified channel DAC register
    ChannelDac(Channel),
    /// Read the value from the power-down register
    PowerDown,
    /// Read the value from the reference register
    ClearCode,
    /// Read the value from the LDAC register
    Ldac,
}

/// Readout from the specified register
#[derive(Debug, Clone, Copy)]
pub enum Configuration {
    /// Value read from the specified channel input register
    ChannelInput {
        /// The channel from which the value was read
        channel: Channel,
        /// The value read from the channel input register
        value: u16,
    },
    /// Value read from the specified channel DAC register
    ChannelDac {
        /// The channel from which the value was read
        channel: Channel,
        /// The value read from the channel DAC register
        value: u16,
    },
    /// Value read from the power-down register
    PowerDown {
        /// The power-down mode
        mode: PowerDownMode,
        /// Selected channels
        channels: Channels,
    },
    /// Value read from the clear code register
    ClearCode(ClearCode),
    /// Value read from the LDAC register
    Ldac(Channels),
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
/// Power-down modes for the DACx578 devices
pub enum PowerDownMode {
    /// Normal operation
    Normal = 0b00,
    /// 1 kΩ to GND
    KOhm1ToGnd = 0b01,
    /// 100 kΩ to GND
    KOhm100ToGnd = 0b10,
    /// High-Z state
    HighZ = 0b11,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
/// Clear code options for the DACx578 devices
/// The Clear pin behavior is configured via the Clear Code register.
pub enum ClearCode {
    /// Outputs are set to 0V when Clear pin is activated.
    ZeroScale = 0b00,
    /// Outputs are set to mid-scale when Clear pin is activated.
    MidScale = 0b01,
    /// Outputs are set to full-scale when Clear pin is activated.
    FullScale = 0b10,
    /// Clear pin functionality is disabled.
    Disabled = 0b11,
}

/// Two bit flags indicating the reset mode for the DAC5578
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ResetMode {
    /// Software reset (default). Same as power-on reset (POR).
    Por = 0b00,
    /// Software reset that sets device into High-Speed mode
    SetHighSpeed = 0b01,
    /// Software reset that maintains High-Speed mode state
    MaintainHighSpeed = 0b10,
}

/// The DACx5578 device instance.
///
/// DAC5578 is a 8-bit DAC ([`Dac8Bits`]).
/// DAC6578 is a 10-bit DAC ([`Dac10Bits`]).
/// DAC7578 is a 12-bit DAC ([`Dac12Bits`]).
pub struct DacX578<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C> DacX578<I2C> {
    /// Creates a new instance of the DACx578 driver.
    pub fn new(i2c: I2C, address: Address) -> Self {
        DacX578 {
            i2c,
            address: address as u8,
        }
    }
}
