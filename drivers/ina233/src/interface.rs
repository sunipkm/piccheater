/// I2C interface for the MMC5983MA sensor.
pub(crate) struct I2cInterface<I2C> {
    pub(crate) i2c: I2C,
    pub(crate) address: u8,
}