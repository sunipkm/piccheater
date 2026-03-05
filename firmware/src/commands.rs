use kmdparse::Parsable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Parsable)]
#[repr(u8)]
pub enum Dacs {
    Dac0 = dacx578::Address::PinHigh as u8,
    Dac1 = dacx578::Address::PinLow as u8,
    Dac2 = dacx578::Address::PinFloat as u8,
}

impl From<Dacs> for dacx578::Address {
    fn from(dac: Dacs) -> Self {
        match dac {
            Dacs::Dac0 => Self::PinHigh,
            Dacs::Dac1 => Self::PinLow,
            Dacs::Dac2 => Self::PinFloat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Parsable)]
pub enum Commands {
    ReadDac {
        dac: Dacs,
        channel: dacx578::Channel,
    },
    WriteDac {
        dac: Dacs,
        channel: dacx578::Channel,
        value: f32,
    },
    AllOff,
    EnableOutputs,
    DisableOutputs,
    GetReportCadence,
    SetReportCadence(u32),
    Help,
}
