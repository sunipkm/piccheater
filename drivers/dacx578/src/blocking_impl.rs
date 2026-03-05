use embedded_hal::i2c::I2c;

use crate::{details::BCAST_ADDR, Channel, Configuration, Register, ResetMode, ValidValue};

/// Trait to define synchronous functions for the DAC device.
pub trait SyncFunctions<I2C, E, T>
where
    I2C: I2c<Error = E>,
{
    /// Read a value from a specific register.
    fn read(&mut self, register: Register) -> Result<Configuration, E>;

    /// Write a configuration to a specific register.
    fn configure(&mut self, config: Configuration) -> Result<(), E>;

    /// Write a value to a specific channel.
    /// The upper N bits are used depending on the DAC resolution.
    fn write(&mut self, channel: Channel, value: T) -> Result<(), E>;

    /// Update a specific channel with a new value.
    /// The upper N bits are used depending on the DAC resolution.
    fn update(&mut self, channel: Channel) -> Result<(), E>;

    /// Write and update a specific channel with a new value.
    /// The upper N bits are used depending on the DAC resolution.
    fn write_and_update(&mut self, channel: Channel, value: T) -> Result<(), E>;

    /// Write new value to a channel and update all channels (global LDAC).
    fn write_and_update_all(&mut self, channel: Channel, value: T) -> Result<(), E>;

    /// Reset the device with the specified reset mode.
    fn reset(&mut self, mode: ResetMode) -> Result<(), E>;
}

impl<I2C, E, T> SyncFunctions<I2C, E, T> for crate::DacX578<I2C, T>
where
    I2C: I2c<Error = E>,
    T: ValidValue,
{
    fn read(&mut self, register: Register) -> Result<Configuration, E> {
        let mut buf = [0u8; 2];
        self.i2c
            .write_read(self.address, &[register.into()], &mut buf)?;
        let value = u16::from_be_bytes(buf);
        Ok(Configuration::from((register, value)))
    }

    fn configure(&mut self, config: Configuration) -> Result<(), E> {
        let bytes: [u8; 3] = config.into();
        self.i2c.write(self.address, &bytes)
    }

    fn write(&mut self, channel: Channel, value: T) -> Result<(), E> {
        let cmd = self.get_command_bytes(crate::CommandType::Write, channel, value);
        self.i2c.write(self.address, &cmd)
    }

    fn update(&mut self, channel: Channel) -> Result<(), E> {
        let cmd = self.get_command_bytes(crate::CommandType::Update, channel, T::zero());
        self.i2c.write(self.address, &cmd)
    }

    fn write_and_update(&mut self, channel: Channel, value: T) -> Result<(), E> {
        let write_cmd = self.get_command_bytes(crate::CommandType::WriteUpdate, channel, value);
        self.i2c.write(self.address, &write_cmd)
    }

    fn write_and_update_all(&mut self, channel: Channel, value: T) -> Result<(), E> {
        let write_cmd = self.get_command_bytes(crate::CommandType::WriteUpdateAll, channel, value);
        self.i2c.write(self.address, &write_cmd)
    }

    fn reset(&mut self, mode: ResetMode) -> Result<(), E> {
        self.i2c.write(self.address, &[0x70, mode as u8, 0x0])
    }
}

/// Wake up all devices on the bus.
/// WARNING: This function uses the default I2C address (0x47) and will affect ALL DACx578 devices.
pub fn wake_up_all_sync<I2C, E>(i2c: &mut I2C) -> Result<(), E>
where
    I2C: I2c<Error = E>,
{
    i2c.write(BCAST_ADDR, &[0x6])
}
/// Reset all devices on the bus.
/// WARNING: This function uses the default I2C address (0x47) and will affect ALL DACx578 devices.
pub fn reset_all_sync<I2C, E>(i2c: &mut I2C) -> Result<(), E>
where
    I2C: I2c<Error = E>,
{
    i2c.write(BCAST_ADDR, &[0x9])
}

/// Configure all devices on the bus with the specified configuration.
/// Useful for setting global configurations like power-down mode or LDAC settings.
/// WARNING: This function uses the default I2C address (0x47) and will affect ALL DACx578 devices.
pub fn configure_all_sync<I2C, E>(i2c: &mut I2C, config: Configuration) -> Result<(), E>
where
    I2C: I2c<Error = E>,
{
    let bytes: [u8; 3] = config.into();
    i2c.write(BCAST_ADDR, &bytes)
}
