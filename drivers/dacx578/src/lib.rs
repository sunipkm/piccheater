#![no_std]
#![deny(missing_docs)]
//! Driver for the DACx578 series Digital to Analog Converters (DACs) over I2C.
//! DAC5578 (8-bit), DAC6578 (10-bit), and DAC7578 (12-bit) devices are supported.

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

impl From<u8> for Channel {
    fn from(index: u8) -> Self {
        match index {
            0 => Channel::A,
            1 => Channel::B,
            2 => Channel::C,
            3 => Channel::D,
            4 => Channel::E,
            5 => Channel::F,
            6 => Channel::G,
            7 => Channel::H,
            _ => panic!("Unkown channel number {}", index),
        }
    }
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
pub enum ReadRegister {
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
pub enum Readout {
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
    PowerDown(u8),
    /// Value read from the clear code register
    ClearCode(u8),
    /// Value read from the LDAC register
    Ldac(Channels),
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
pub struct DacX578<BITS, I2C>
where
    BITS: DacBits,
{
    i2c: I2C,
    address: u8,
    _bits: core::marker::PhantomData<BITS>,
}

/// Trait to define the bit resolution of the DAC device.
pub trait DacBits {
    /// The number of bits to shift the input value to map to the DAC resolution.
    const SHIFT: u8;

    /// Maps the input value to the DAC resolution by shifting left.
    fn map(inp: u16) -> u16 {
        inp << Self::SHIFT
    }
}

/// 8-bit DAC5578
pub struct Dac8Bits;

impl DacBits for Dac8Bits {
    const SHIFT: u8 = 8;
}

/// 10-bit DAC6578
pub struct Dac10Bits;

impl DacBits for Dac10Bits {
    const SHIFT: u8 = 6;
}

/// 12-bit DAC7578
pub struct Dac12Bits;

impl DacBits for Dac12Bits {
    const SHIFT: u8 = 4;
}

impl<BITS: DacBits, I2C> DacX578<BITS, I2C> {
    /// Creates a new instance of the DACx578 driver.
    pub fn new(i2c: I2C, address: Address) -> Self {
        DacX578 {
            i2c,
            address: address as u8,
            _bits: core::marker::PhantomData,
        }
    }

    /// Get the command bytes
    pub(crate) fn get_command_bytes(command: CommandType, channel: Channel, value: u16) -> [u8; 3] {
        let command_byte = (command as u8) | (channel as u8);
        let value = BITS::map(value);
        let high_byte = (value >> 8) as u8;
        let low_byte = (value & 0xff) as u8;
        [command_byte, high_byte, low_byte]
    }
}

mod async_impl;
mod blocking_impl;

pub use async_impl::AsyncFunctions;
use bitfield_struct::bitfield;
pub use blocking_impl::SyncFunctions;

#[allow(clippy::from_over_into)]
impl Into<u8> for ReadRegister {
    fn into(self) -> u8 {
        match self {
            ReadRegister::ChannelInput(channel) => channel as u8,
            ReadRegister::ChannelDac(channel) => (channel as u8) | 0x10,
            ReadRegister::PowerDown => 0x40,
            ReadRegister::ClearCode => 0x50,
            ReadRegister::Ldac => 0x60,
        }
    }
}

impl From<(ReadRegister, u16, u8)> for Readout {
    fn from(value: (ReadRegister, u16, u8)) -> Self {
        let (reg, value, shift) = value;
        match reg {
            ReadRegister::ChannelInput(channel) => Readout::ChannelInput {
                channel,
                value: value >> shift,
            },
            ReadRegister::ChannelDac(channel) => Readout::ChannelDac {
                channel,
                value: value >> shift,
            },
            ReadRegister::PowerDown => Readout::PowerDown(value as u8),
            ReadRegister::ClearCode => Readout::ClearCode(value as u8),
            ReadRegister::Ldac => Readout::Ldac(Channels::from_bits(value as u8)),
        }
    }
}
