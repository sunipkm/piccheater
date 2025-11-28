use embedded_hal_async::i2c::I2c;

use crate::{Channel, Configuration, DacX578, Register, ResetMode};

#[allow(async_fn_in_trait)]
/// Trait to define asynchronous functions for the DAC device.
pub trait AsyncFunctions<I2C, E>
where
    I2C: I2c<Error = E>,
{
    /// Read a value from a specific register.
    async fn read(&mut self, register: Register) -> Result<Configuration, E>;
    /// Write a configuration to a specific register.
    async fn configure(&mut self, config: Configuration) -> Result<(), E>;
    /// Write a value to a specific channel.
    /// The upper N bits are used depending on the DAC resolution.
    async fn write(&mut self, channel: Channel, value: u16) -> Result<(), E>;
    /// Update a specific channel with a new value.
    /// The upper N bits are used depending on the DAC resolution.
    async fn update(&mut self, channel: Channel) -> Result<(), E>;
    /// Write and update a specific channel with a new value.
    /// The upper N bits are used depending on the DAC resolution.
    async fn write_and_update(&mut self, channel: Channel, value: u16) -> Result<(), E>;
    /// Write new value to a channel and update all channels (global LDAC).
    async fn write_and_update_all(&mut self, channel: Channel, value: u16) -> Result<(), E>;
    /// Reset the device with the specified reset mode.
    async fn reset(&mut self, mode: ResetMode) -> Result<(), E>;
    /// Wake up all devices on the bus.
    /// WARNING: This function uses the default I2C address (0x0) and may affect multiple devices.
    async fn wake_up_all(i2c: &mut I2C) -> Result<(), E> {
        i2c.write(0x0, &[0x6]).await
    }
    /// Reset all devices on the bus.
    /// WARNING: This function uses the default I2C address (0x0) and may affect multiple devices.
    async fn reset_all(i2c: &mut I2C) -> Result<(), E> {
        i2c.write(0x0, &[0x9]).await
    }
}

impl<I2C, E> AsyncFunctions<I2C, E> for DacX578<I2C>
where
    I2C: I2c<Error = E>,
{
    async fn read(&mut self, register: Register) -> Result<Configuration, E> {
        let mut buf = [0u8; 2];
        self.i2c
            .write_read(self.address, &[register.into()], &mut buf)
            .await?;
        let value = u16::from_be_bytes(buf);
        Ok(Configuration::from((register, value)))
    }

    async fn configure(&mut self, config: Configuration) -> Result<(), E> {
        let bytes: [u8; 3] = config.into();
        self.i2c.write(self.address, &bytes).await
    }

    async fn write(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let cmd = Self::get_command_bytes(crate::CommandType::Write, channel, value);
        self.i2c.write(self.address, &cmd).await
    }

    async fn update(&mut self, channel: Channel) -> Result<(), E> {
        let cmd = Self::get_command_bytes(crate::CommandType::Update, channel, 0);
        self.i2c.write(self.address, &cmd).await
    }

    async fn write_and_update(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let write_cmd = Self::get_command_bytes(crate::CommandType::WriteUpdate, channel, value);
        self.i2c.write(self.address, &write_cmd).await
    }

    async fn write_and_update_all(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let write_cmd = Self::get_command_bytes(crate::CommandType::WriteUpdateAll, channel, value);
        self.i2c.write(self.address, &write_cmd).await
    }

    async fn reset(&mut self, mode: ResetMode) -> Result<(), E> {
        self.i2c.write(self.address, &[0x70, mode as u8, 0x0]).await
    }
}
