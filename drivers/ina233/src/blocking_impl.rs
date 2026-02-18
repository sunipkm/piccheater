use embedded_hal::{delay::DelayNs, i2c};
use uom::si::f32::{ElectricCurrent, ElectricPotential, Power};

use crate::{
    Error, Ina233,
    configuration::Configuration,
    interface::I2cInterface,
    registers::{
        AdcConfig, Calibration, ConvertRaw, LoadCurrent, LoadPower, LoadVoltage, MfrId, MfrModel,
        Register, ShuntVoltage,
    },
};

/// Trait for reading and writing registers synchronously.
pub(crate) trait SyncRegister<I2C, T, const N: usize>
where
    I2C: i2c::I2c,
    Self: Register<T> + ConvertRaw<N> + Sized,
{
    fn read_register(iface: &mut I2cInterface<I2C>) -> Result<Self, I2C::Error> {
        #[cfg(feature = "defmt")]
        {
            defmt::trace!("INA233: Reading register 0x{:02X}", Self::ADDRESS);
        }
        let mut data = [0u8; N];
        let addr = [Self::ADDRESS];
        iface.i2c.transaction(
            iface.address,
            &mut [
                i2c::Operation::Write(&addr),
                i2c::Operation::Read(&mut data),
            ],
        )?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!("INA233: Read register 0x{:02X}: {:?}", Self::ADDRESS, &data);
        }
        Ok(Self::from_raw(data))
    }

    fn write_register(&self, iface: &mut I2cInterface<I2C>) -> Result<(), I2C::Error> {
        #[cfg(feature = "defmt")]
        {
            defmt::trace!(
                "INA233: Writing register 0x{:02X}: {:?}",
                Self::ADDRESS,
                self
            );
        }
        iface.i2c.transaction(
            iface.address,
            &mut [
                i2c::Operation::Write(&[Self::ADDRESS]),
                i2c::Operation::Write(&self.to_raw()),
            ],
        )?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!("INA233: Wrote register 0x{:02X}", Self::ADDRESS);
            defmt::trace!("INA233: Data: {:?}", Self::read_register(iface)?);
        }
        Ok(())
    }
}

macro_rules! impl_sync_register {
    ($(($reg:ty, $num:expr)),+ $(,)?) => {
        $(
            impl<I2C, T> SyncRegister<I2C, T, $num> for $reg
            where
                I2C: i2c::I2c,
                $reg: Register<T>
            {}
        )+
    };
}

impl_sync_register! {
    (AdcConfig, 2),
    (Calibration, 2),
    (LoadVoltage, 2),
    (LoadCurrent, 2),
    (LoadPower, 2),
    (ShuntVoltage, 2),
    (MfrId, 4),
    (MfrModel, 8),
}

/// Trait defining the synchronous interface for the INA233 driver.
pub trait SyncInterface<I2C, D>
where
    I2C: i2c::I2c,
{
    /// Reset the INA233 with the given ADC configuration.
    fn reset(&mut self, configuration: AdcConfig) -> Result<(), Error<I2C::Error>>;
    /// Read the load current and load voltage.
    fn read(&mut self) -> Result<(ElectricCurrent, ElectricPotential), Error<I2C::Error>>;
    /// Read the load power.
    fn read_power(&mut self) -> Result<Power, Error<I2C::Error>>;
    /// Read the shunt voltage.
    fn read_shunt(&mut self) -> Result<ElectricPotential, Error<I2C::Error>>;
}

impl<I2C, D> Ina233<I2C, D>
where
    I2C: i2c::I2c,
    D: DelayNs,
{
    /// Create a new synchronous INA233 driver instance.
    /// # Arguments
    /// * `i2c` - The I2C interface to use.
    /// * `delay` - The delay provider to use.
    /// * `configuration` - The configuration to initialize the INA233 with.
    /// # Returns
    /// A Result containing the INA233 driver instance or an I2C error.
    pub fn new_sync(
        i2c: I2C,
        delay: D,
        configuration: Configuration,
    ) -> Result<Self, Error<I2C::Error>> {
        let mut delay = delay;
        let base_address = 0x40;
        let address = base_address | (configuration.a1 as u8) << 2 | (configuration.a0 as u8);
        #[cfg(feature = "defmt")]
        {
            defmt::trace!("[INA233] Initializing at address {:#02x}", address);
        }
        let mut i2c = I2cInterface { i2c, address };
        let mfrid = MfrId::read_register(&mut i2c)?;
        if mfrid.id() != *b"TI" {
            #[cfg(feature = "defmt")]
            {
                defmt::error!("[INA233] Unexpected Manufacturer ID: {:?}", mfrid.id());
            }
            return Err(Error::DeviceId);
        }
        let mfrmodel = MfrModel::read_register(&mut i2c)?;
        if mfrmodel.model() != *b"INA233" {
            #[cfg(feature = "defmt")]
            {
                defmt::error!("[INA233] Unexpected Model: {:?}", mfrmodel.model());
            }
            return Err(Error::DeviceId);
        }
        i2c.i2c.write(i2c.address, &[0x12])?; // Reset command
        delay.delay_ms(100); // Wait for reset to complete

        let calibration = Calibration::from(configuration.calibration);
        calibration.write_register(&mut i2c)?; // Write calibration
        let new_calibration = Calibration::read_register(&mut i2c)?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!(
                "[INA233] Written calibration value: {}",
                configuration.calibration
            );
            defmt::trace!("[INA233] Verified calibration value: {}", new_calibration,);
            if calibration != new_calibration {
                defmt::warn!(
                    "[INA233] Calibration mismatch: written {}, read {}",
                    calibration.calibration(),
                    new_calibration.calibration()
                );
            }
        }
        if calibration != new_calibration {
            return Err(Error::CalibrationMismatch);
        }
        configuration.adc_conf.write_register(&mut i2c)?; // Write ADC configuration
        let new_adc_conf = AdcConfig::read_register(&mut i2c)?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!(
                "[INA233] Written ADC configuration: {:?}",
                configuration.adc_conf
            );
            defmt::trace!(
                "[INA233] Verified ADC configuration: {} => Avg={:?}, Bus Read Time={:?}, Shunt Read Time={:?}, Mode={:?}",
                new_adc_conf,
                new_adc_conf.averaging(),
                new_adc_conf.vbus_conv_time(),
                new_adc_conf.vshunt_conv_time(),
                new_adc_conf.mode()
            );
            if configuration.adc_conf != new_adc_conf {
                defmt::warn!(
                    "[INA233] ADC configuration mismatch: written {:?}, read {:?}",
                    configuration.adc_conf,
                    new_adc_conf
                );
            }
        }
        if configuration.adc_conf != new_adc_conf {
            return Err(Error::ConfigurationMismatch);
        }
        Ok(Self {
            i2c,
            delay,
            lsb: configuration.lsb,
            calibration,
        })
    }
}

impl<I2C, D> SyncInterface<I2C, D> for Ina233<I2C, D>
where
    I2C: i2c::I2c,
    D: DelayNs,
{
    fn reset(&mut self, configuration: AdcConfig) -> Result<(), Error<I2C::Error>> {
        self.i2c.i2c.write(self.i2c.address, &[0x12])?; // Reset command
        self.delay.delay_ms(100); // Wait for reset to complete
        self.calibration.write_register(&mut self.i2c)?; // Write calibration
        let new_calibration = Calibration::read_register(&mut self.i2c)?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!(
                "INA233: Written calibration value: {}, Read back: {}",
                self.calibration.calibration(),
                new_calibration.calibration()
            );
            if self.calibration != new_calibration {
                defmt::warn!(
                    "INA233: Calibration mismatch: written {}, read {}",
                    self.calibration.calibration(),
                    new_calibration.calibration()
                );
            }
        }
        if self.calibration != new_calibration {
            return Err(Error::CalibrationMismatch); // Use DeviceId error for calibration mismatch
        }
        configuration.write_register(&mut self.i2c)?; // Write ADC configuration
        let new_configuration = AdcConfig::read_register(&mut self.i2c)?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!(
                "INA233: Written ADC configuration: {:?}, Read back: {:?}",
                configuration,
                new_configuration
            );
            if configuration != new_configuration {
                defmt::warn!(
                    "INA233: ADC configuration mismatch: written {:?}, read {:?}",
                    configuration,
                    new_configuration
                );
            }
        }
        if configuration != new_configuration {
            return Err(Error::ConfigurationMismatch); // Use DeviceId error for ADC configuration mismatch
        }
        Ok(())
    }

    fn read(&mut self) -> Result<(ElectricCurrent, ElectricPotential), Error<I2C::Error>> {
        let current_raw = LoadCurrent::read_register(&mut self.i2c)?;
        let voltage_raw = LoadVoltage::read_register(&mut self.i2c)?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!(
                "INA233: Raw Current: {:?}, Raw Voltage: {:?}",
                current_raw,
                voltage_raw
            );
        }
        let current = current_raw.into_current(self.lsb);
        let voltage = ElectricPotential::from(voltage_raw); // LSB = 1.25 mV
        Ok((current, voltage))
    }

    fn read_power(&mut self) -> Result<Power, Error<I2C::Error>> {
        let power_raw = LoadPower::read_register(&mut self.i2c)?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!("INA233: Raw Power: {:?}", power_raw);
        }
        let power = ElectricPotential::from(power_raw) * self.lsb; // Power LSB = Current LSB * 25
        Ok(power)
    }

    fn read_shunt(&mut self) -> Result<ElectricPotential, Error<I2C::Error>> {
        let shunt_raw = ShuntVoltage::read_register(&mut self.i2c)?;
        #[cfg(feature = "defmt")]
        {
            defmt::trace!("INA233: Raw Shunt Voltage: {:?}", shunt_raw);
        }
        let shunt_voltage = ElectricPotential::from(shunt_raw); // LSB = 2.5 uV
        Ok(shunt_voltage)
    }
}
