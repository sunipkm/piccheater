use embedded_hal_async::i2c::I2c;

use crate::{Channel, DacBits, ReadRegister, ResetMode};

#[allow(async_fn_in_trait)]
/// Trait to define asynchronous functions for the DAC device.
pub trait AsyncFunctions<I2C, E>
where
    I2C: I2c<Error = E>,
{
    /// Read a value from a specific register.
    async fn read(&mut self, register: ReadRegister) -> Result<u16, E>;
    /// Write a value to a specific channel.
    async fn write(&mut self, channel: Channel, value: u16) -> Result<(), E>;
    /// Update a specific channel with a new value.
    async fn update(&mut self, channel: Channel) -> Result<(), E>;
    /// Write and update a specific channel with a new value.
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

impl<I2C, E, BITS> AsyncFunctions<I2C, E> for crate::DacX578<BITS, I2C>
where
    I2C: I2c<Error = E>,
    BITS: DacBits,
{
    async fn read(&mut self, register: ReadRegister) -> Result<u16, E> {
        let mut buf = [0u8; 2];
        self.i2c
            .write_read(self.address, &[register.into()], &mut buf)
            .await?;
        Ok(u16::from_be_bytes(buf))
    }

    async fn write(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let cmd = crate::DacX578::<BITS, I2C>::get_command_bytes(
            crate::CommandType::Write,
            channel,
            value,
        );
        self.i2c.write(self.address, &cmd).await
    }

    async fn update(&mut self, channel: Channel) -> Result<(), E> {
        let cmd =
            crate::DacX578::<BITS, I2C>::get_command_bytes(crate::CommandType::Update, channel, 0);
        self.i2c.write(self.address, &cmd).await
    }

    async fn write_and_update(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let write_cmd = crate::DacX578::<BITS, I2C>::get_command_bytes(
            crate::CommandType::WriteUpdate,
            channel,
            value,
        );
        self.i2c.write(self.address, &write_cmd).await
    }

    async fn write_and_update_all(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let write_cmd = crate::DacX578::<BITS, I2C>::get_command_bytes(
            crate::CommandType::WriteUpdateAll,
            channel,
            value,
        );
        self.i2c.write(self.address, &write_cmd).await
    }

    async fn reset(&mut self, mode: ResetMode) -> Result<(), E> {
        self.i2c.write(self.address, &[0x70, mode as u8, 0x0]).await
    }
}
