use kmdparse::Parsable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Parsable)]
pub enum Dacs {
    Dac0 = 0x48,
    Dac1 = 0x4a,
    Dac2 = 0x4c,
}

impl From<Dacs> for dacx578::Address {
    fn from(dac: Dacs) -> Self {
        match dac {
            Dacs::Dac0 => Self::PinLow,
            Dacs::Dac1 => Self::PinHigh,
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
