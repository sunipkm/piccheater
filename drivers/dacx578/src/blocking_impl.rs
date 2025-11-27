use embedded_hal::i2c::I2c;

use crate::{Channel, DacBits, ResetMode};

/// Trait to define synchronous functions for the DAC device.
pub trait SyncFunctions<I2C, E>
where
    I2C: I2c<Error = E>,
{
    /// Write a value to a specific channel.
    fn write(&mut self, channel: Channel, value: u16) -> Result<(), E>;
    /// Update a specific channel with a new value.
    fn update(&mut self, channel: Channel) -> Result<(), E>;
    /// Write and update a specific channel with a new value.
    fn write_and_update(&mut self, channel: Channel, value: u16) -> Result<(), E>;
    /// Write new value to a channel and update all channels (global LDAC).
    fn write_and_update_all(&mut self, channel: Channel, value: u16) -> Result<(), E>;
    /// Reset the device with the specified reset mode.
    fn reset(&mut self, mode: ResetMode) -> Result<(), E>;
    /// Wake up all devices on the bus.
    /// WARNING: This function uses the default I2C address (0x0) and may affect multiple devices.
    fn wake_up_all(i2c: &mut I2C) -> Result<(), E> {
        i2c.write(0x0, &[0x6])
    }
    /// Reset all devices on the bus.
    /// WARNING: This function uses the default I2C address (0x0) and may affect multiple devices.
    fn reset_all(i2c: &mut I2C) -> Result<(), E> {
        i2c.write(0x0, &[0x9])
    }
}

impl<I2C, E, BITS> SyncFunctions<I2C, E> for crate::DacX578<BITS, I2C>
where
    I2C: I2c<Error = E>,
    BITS: DacBits,
{
    fn write(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let cmd = crate::DacX578::<BITS, I2C>::get_command_bytes(
            crate::CommandType::Write,
            channel,
            value,
        );
        self.i2c.write(self.address, &cmd)
    }

    fn update(&mut self, channel: Channel) -> Result<(), E> {
        let cmd = crate::DacX578::<BITS, I2C>::get_command_bytes(
            crate::CommandType::Update,
            channel,
            0,
        );
        self.i2c.write(self.address, &cmd)
    }

    fn write_and_update(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let write_cmd = crate::DacX578::<BITS, I2C>::get_command_bytes(
            crate::CommandType::WriteUpdate,
            channel,
            value,
        );
        self.i2c.write(self.address, &write_cmd)
    }

    fn write_and_update_all(&mut self, channel: Channel, value: u16) -> Result<(), E> {
        let write_cmd = crate::DacX578::<BITS, I2C>::get_command_bytes(
            crate::CommandType::WriteUpdateAll,
            channel,
            value,
        );
        self.i2c.write(self.address, &write_cmd)
    }

    fn reset(&mut self, mode: ResetMode) -> Result<(), E> {
        self.i2c.write(self.address, &[0x70, mode as u8, 0x0])
    }
}
