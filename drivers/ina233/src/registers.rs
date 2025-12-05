use bitfield_struct::bitfield;


pub(crate) trait Register {
    const ADDRESS: u8;
    fn from_u16(value: u16) -> Self
    where
        Self: Sized;
    fn to_u16(&self) -> u16;
}

macro_rules! impl_register {
    ($reg:ident, $addr:expr) => {
        impl Register for $reg {
            const ADDRESS: u8 = $addr;
            #[inline(always)]
            fn from_u16(value: u16) -> Self {
                Self::from(value)
            }
            #[inline(always)]
            fn to_u16(&self) -> u16 {
                self.0
            }
        }
    };
    ($addr:expr, $reg:ident) => {
        impl Register for $reg {
            const ADDRESS: u8 = $addr;
            #[inline(always)]
            fn from_u16(value: u16) -> Self {
                Self { 0: value }
            }
            #[inline(always)]
            fn to_u16(&self) -> u16 {
                self.0
            }
        }
    };
}

#[bitfield(u16)]
#[derive(Default)]
#[cfg_attr("defmt", derive(defmt::Format))]
pub struct AdcConfig {
    #[bits(3)]
    pub mode: AdcMode,
    #[bits(3)]
    pub vshunt_conv_time: ConversionTime,
    #[bits(3)]
    pub vbus_conv_time: ConversionTime,
    #[bits(3)]
    pub averaging: Averages,
    #[bits(4, access=RO, default=0b0100)]
    rsvd: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr("defmt", derive(defmt::Format))]
pub enum AdcMode {
    PowerDown,
    Triggered(MeasureActive),
    #[default]
    Continuous(MeasureActive),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr("defmt", derive(defmt::Format))]
pub enum MeasureActive {
    Shunt,
    Bus,
    #[default]
    Both,
}

impl From<AdcMode> for u8 {
    fn from(mode: AdcMode) -> Self {
        match mode {
            AdcMode::PowerDown => 0b000,
            AdcMode::Triggered(MeasureActive::Shunt) => 0b001,
            AdcMode::Triggered(MeasureActive::Bus) => 0b010,
            AdcMode::Triggered(MeasureActive::Both) => 0b011,
            AdcMode::Continuous(MeasureActive::Shunt) => 0b101,
            AdcMode::Continuous(MeasureActive::Bus) => 0b110,
            AdcMode::Continuous(MeasureActive::Both) => 0b111,
        }
    }
}

impl From<u8> for AdcMode {
    fn from(value: u8) -> Self {
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
#[cfg_attr("defmt", derive(defmt::Format))]
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

impl From<u8> for ConversionTime {
    fn from(value: u8) -> Self {
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr("defmt", derive(defmt::Format))]
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

impl_register!(AdcConfig, 0xd0);