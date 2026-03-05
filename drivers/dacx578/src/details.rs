#[cfg(feature = "uom")]
use uom::si::{f32::ElectricPotential, ratio::ratio};

use crate::{
    Channel, Channels, ClearCode, CommandType, Configuration, DacX578, PowerDownMode, Register,
};

pub(crate) const INPUT_REG: u8 = 0x0;
pub(crate) const DAC_REG: u8 = 0x10;
pub(crate) const PD_REG: u8 = 0x40;
pub(crate) const CC_REG: u8 = 0x50;
pub(crate) const LDAC_REG: u8 = 0x60;

impl<I2C, T: ValidValue> DacX578<I2C, T> {
    pub(crate) fn get_command_bytes(
        &self,
        command: CommandType,
        channel: Channel,
        value: T,
    ) -> [u8; 3] {
        let value = value.scale(self.scale);
        let command_byte = (command as u8) | (channel as u8);
        let high_byte = (value >> 8) as u8;
        let low_byte = (value & 0xff) as u8;
        [command_byte, high_byte, low_byte]
    }
}

/// Valid value that can be written to
/// the DAC.
pub trait ValidValue: Copy {
    /// Scale the value to u16
    fn scale(self, scale: Self) -> u16;
    /// Get the zero scale value
    fn zero() -> Self;
    /// Invert the scaling of the value, converting from a u16 back to the original type
    fn invert(self, value: u16) -> Self;
}

impl ValidValue for u16 {
    fn scale(self, _scale: Self) -> u16 {
        self
    }

    fn zero() -> u16 {
        0u16
    }

    fn invert(self, value: u16) -> Self {
        value
    }
}

#[cfg(feature = "uom")]
impl ValidValue for ElectricPotential {
    fn scale(self, scale: Self) -> u16 {
        (self * u16::MAX as f32 / scale).get::<ratio>() as u16
    }

    fn zero() -> Self {
        use uom::ConstZero;

        ElectricPotential::ZERO
    }

    fn invert(self, value: u16) -> Self {
        (value as f32 / u16::MAX as f32) * self
    }
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

#[allow(clippy::from_over_into)]
impl Into<u8> for Register {
    fn into(self) -> u8 {
        match self {
            Register::ChannelInput(channel) => channel as u8 | INPUT_REG,
            Register::ChannelDac(channel) => (channel as u8) | DAC_REG,
            Register::PowerDown => PD_REG,
            Register::ClearCode => CC_REG,
            Register::Ldac => LDAC_REG,
        }
    }
}

impl From<(Register, u16)> for Configuration {
    fn from(value: (Register, u16)) -> Self {
        let (reg, value) = value;
        match reg {
            Register::ChannelInput(channel) => Configuration::ChannelInput {
                channel,
                value: value & 0xfff0,
            },
            Register::ChannelDac(channel) => Configuration::ChannelDac {
                channel,
                value: value & 0xfff0,
            },
            Register::PowerDown => {
                let channels = Channels::from((value & 0xff) as u8);
                let mode = match (value >> 8) & 0b11 {
                    0b00 => PowerDownMode::Normal,
                    0b01 => PowerDownMode::KOhm1ToGnd,
                    0b10 => PowerDownMode::KOhm100ToGnd,
                    0b11 => PowerDownMode::HighZ,
                    _ => unreachable!(),
                };
                Configuration::PowerDown { mode, channels }
            }
            Register::ClearCode => {
                let clear_code = match value & 0b11 {
                    0b00 => ClearCode::ZeroScale,
                    0b01 => ClearCode::MidScale,
                    0b10 => ClearCode::FullScale,
                    0b11 => ClearCode::Disabled,
                    _ => unreachable!(),
                };
                Configuration::ClearCode(clear_code)
            }
            Register::Ldac => Configuration::Ldac(Channels::from_bits(value as u8)),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<[u8; 3]> for Configuration {
    fn into(self) -> [u8; 3] {
        match self {
            Configuration::Ldac(channels) => {
                let command_byte = LDAC_REG;
                let high_byte = channels.into_bits();
                let low_byte = 0;
                [command_byte, high_byte, low_byte]
            }
            Configuration::ClearCode(clear_code) => {
                let command_byte = CC_REG;
                let high_byte = 0;
                let low_byte = (clear_code as u8) << 4;
                [command_byte, high_byte, low_byte]
            }
            Configuration::PowerDown { mode, channels } => {
                let command_byte = PD_REG;
                let value = ((mode as u16) << 8 | (channels.into_bits() as u16)) << 5;
                let high_byte = (value >> 8) as u8;
                let low_byte = (value & 0xff) as u8;
                [command_byte, high_byte, low_byte]
            }
            Configuration::ChannelInput { channel, value } => {
                let command_byte = channel as u8 | INPUT_REG;
                let high_byte = (value >> 8) as u8;
                let low_byte = (value & 0xff) as u8;
                [command_byte, high_byte, low_byte]
            }
            Configuration::ChannelDac { channel, value } => {
                let command_byte = channel as u8 | DAC_REG;
                let high_byte = (value >> 8) as u8;
                let low_byte = (value & 0xff) as u8;
                [command_byte, high_byte, low_byte]
            }
        }
    }
}

pub(crate) const BCAST_ADDR: u8 = 0b0100_0111;
