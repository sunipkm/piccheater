use bitfield_struct::bitfield;

pub(crate) trait ConvertRaw<const N: usize> {
    fn from_raw(raw: [u8; N]) -> Self
    where
        Self: Sized;
    fn to_raw(&self) -> [u8; N];
}

pub(crate) trait Register<T> {
    const ADDRESS: u8;
}

macro_rules! impl_register {
    ($reg:ident, $addr:expr, $typ:ty, $size:expr) => {
        impl ConvertRaw<$size> for $reg {
            #[inline(always)]
            fn from_raw(raw: [u8; $size]) -> Self {
                Self::from(<$typ>::from_le_bytes(raw))
            }
            #[inline(always)]
            fn to_raw(&self) -> [u8; $size] {
                self.0.to_le_bytes()
            }
        }

        impl Register<$typ> for $reg {
            const ADDRESS: u8 = $addr;
        }
    };
}

#[bitfield(u16)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AdcConfig {
    #[bits(3, from=AdcMode::from_u8, into=AdcMode::into_u8)]
    pub mode: AdcMode,
    #[bits(3, from=ConversionTime::from_u8, into=ConversionTime::into_u8)]
    pub vshunt_conv_time: ConversionTime,
    #[bits(3, from=ConversionTime::from_u8, into=ConversionTime::into_u8)]
    pub vbus_conv_time: ConversionTime,
    #[bits(3, from=Averages::from_u8, into=Averages::into_u8)]
    pub averaging: Averages,
    #[bits(4, access=RO, default=0b0100)]
    rsvd: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AdcMode {
    PowerDown,
    Triggered(MeasureActive),
    Continuous(MeasureActive),
}

impl Default for AdcMode {
    fn default() -> Self {
        Self::Continuous(MeasureActive::default())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum MeasureActive {
    Shunt = 0b01,
    Bus = 0b10,
    #[default]
    Both = 0b11,
}

impl AdcMode {
    const fn into_u8(mode: AdcMode) -> u8 {
        match mode {
            AdcMode::PowerDown => 0b000,
            AdcMode::Triggered(c) => c as u8,
            AdcMode::Continuous(c) => (c as u8) | 0b100,
        }
    }

    const fn from_u8(value: u8) -> Self {
        match value & 0b111 {
            0b000 | 0b100 => AdcMode::PowerDown,
            0b001 => AdcMode::Triggered(MeasureActive::Shunt),
            0b010 => AdcMode::Triggered(MeasureActive::Bus),
            0b011 => AdcMode::Triggered(MeasureActive::Both),
            0b101 => AdcMode::Continuous(MeasureActive::Shunt),
            0b110 => AdcMode::Continuous(MeasureActive::Bus),
            0b111 => AdcMode::Continuous(MeasureActive::Both),
            _ => unreachable!(), // Should not happen
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum ConversionTime {
    Us140 = 0b000,
    Us204 = 0b001,
    Us332 = 0b010,
    Us588 = 0b011,
    #[default]
    Ms1_1 = 0b100,
    Ms2_116 = 0b101,
    Ms4_156 = 0b110,
    Ms8_244 = 0b111,
}

impl ConversionTime {
    const fn from_u8(value: u8) -> Self {
        match value & 0b111 {
            0b000 => ConversionTime::Us140,
            0b001 => ConversionTime::Us204,
            0b010 => ConversionTime::Us332,
            0b011 => ConversionTime::Us588,
            0b100 => ConversionTime::Ms1_1,
            0b101 => ConversionTime::Ms2_116,
            0b110 => ConversionTime::Ms4_156,
            0b111 => ConversionTime::Ms8_244,
            _ => unreachable!(), // Should not happen
        }
    }

    const fn into_u8(time: ConversionTime) -> u8 {
        time as u8 & 0b111
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Averages {
    #[default]
    Avg1 = 0b000,
    Avg4 = 0b001,
    Avg16 = 0b010,
    Avg64 = 0b011,
    Avg128 = 0b100,
    Avg256 = 0b101,
    Avg512 = 0b110,
    Avg1024 = 0b111,
}

impl Averages {
    const fn from_u8(value: u8) -> Self {
        match value & 0b111 {
            0b000 => Averages::Avg1,
            0b001 => Averages::Avg4,
            0b010 => Averages::Avg16,
            0b011 => Averages::Avg64,
            0b100 => Averages::Avg128,
            0b101 => Averages::Avg256,
            0b110 => Averages::Avg512,
            0b111 => Averages::Avg1024,
            _ => unreachable!(), // Should not happen
        }
    }

    const fn into_u8(averages: Averages) -> u8 {
        averages as u8 & 0b111
    }
}

impl_register!(AdcConfig, 0xd0, u16, 2);

pub(crate) struct LoadVoltage(pub(crate) i16);

impl From<i16> for LoadVoltage {
    fn from(value: i16) -> Self {
        Self(value)
    }
}

impl_register!(LoadVoltage, 0x88, i16, 2);
pub(crate) struct ShuntVoltage(pub(crate) i16);

impl From<i16> for ShuntVoltage {
    fn from(value: i16) -> Self {
        Self(value)
    }
}

impl_register!(ShuntVoltage, 0xd1, i16, 2);

pub(crate) struct LoadCurrent(pub(crate) i16);

impl From<i16> for LoadCurrent {
    fn from(value: i16) -> Self {
        Self(value)
    }
}

impl_register!(LoadCurrent, 0x89, i16, 2);

pub(crate) struct LoadPower(pub(crate) u16);

impl From<u16> for LoadPower {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl_register!(LoadPower, 0x97, u16, 2);

#[bitfield(u32)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct MfrId {
    #[bits(8, default = 2)]
    bytes: u8,
    #[bits(16, default=*b"TI", from=from_bits_u16, into=to_bits_u16)]
    pub id: [u8; 2],
    #[bits(8, default = 0)]
    rsvd: u8,
}

impl_register!(MfrId, 0x99, u32, 4);

#[bitfield(u64)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct MfrModel {
    #[bits(8, default = 6)]
    pub bytes: u8,
    #[bits(48, default=*b"INA233", from=from_bits_u64, into=to_bits_u64)]
    pub model: [u8; 6],
    #[bits(8, default = 0)]
    rsvd: u8,
}

const fn from_bits_u64(bits: u64) -> [u8; 6] {
    let bytes = bits.to_le_bytes();
    [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]]
}

const fn to_bits_u64(model: [u8; 6]) -> u64 {
    u64::from_le_bytes([
        model[0], model[1], model[2], model[3], model[4], model[5], 0, 0,
    ])
}

impl_register!(MfrModel, 0x9a, u64, 8);

#[bitfield(u32)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct MfrRevision {
    #[bits(8, default = 2)]
    bytes: u8,
    #[bits(16, default=*b"A0", from=from_bits_u16, into=to_bits_u16)]
    pub revision: [u8; 2],
    #[bits(8, default = 0)]
    rsvd: u8,
}

const fn from_bits_u16(bits: u16) -> [u8; 2] {
    bits.to_le_bytes()
}

const fn to_bits_u16(bytes: [u8; 2]) -> u16 {
    u16::from_le_bytes(bytes)
}

impl_register!(MfrRevision, 0x9b, u32, 4);

#[bitfield(u16)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct Calibration {
    #[bits(15, default = 1)]
    pub calibration: i16,
    #[bits(1, default = 0)]
    rsvd: u8,
}

impl_register!(Calibration, 0xd4, u16, 2);

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;

    #[test]
    fn test_adc_config_conversion() {
        let config = AdcConfig {
            ..Default::default()
        };
        let raw = config.to_raw();
        std::println!("Raw ADC Config: {:04x}", u16::from_le_bytes(raw));
    }
}
