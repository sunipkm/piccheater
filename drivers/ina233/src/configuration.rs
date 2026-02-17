use uom::si::f32::ElectricalResistance;

use crate::{AddrPin, registers::AdcConfig};

#[derive(Default)]
/// Builder for INA233 driver configuration
pub struct ConfigurationBuilder {
    a0: AddrPin,
    a1: AddrPin,
    adc_conf: AdcConfig,
    max_current: Option<uom::si::f32::ElectricCurrent>,
}

/// INA233 driver configuration
pub struct Configuration {
    pub(crate) a0: AddrPin,
    pub(crate) a1: AddrPin,
    pub(crate) adc_conf: AdcConfig,
    pub(crate) calibration: u16,
    // pub(crate) current_lsb: uom::si::f32::ElectricCurrent,
    pub(crate) shunt: ElectricalResistance,
}

#[cfg(feature = "defmt")]
impl defmt::Format for Configuration {
    fn format(&self, fmt: defmt::Formatter) {
        use uom::si::electrical_resistance::milliohm;

        defmt::write!(
            fmt,
            "Address A0={}, A1={}, ADC={:#?}, Shunt={:?}, CAL={:x}h",
            self.a0,
            self.a1,
            self.adc_conf,
            self.shunt.get::<milliohm>(),
            self.calibration
        )
    }
}

impl ConfigurationBuilder {
    /// Set the address pin A0 configuration.
    pub fn addr_a0(mut self, a0: AddrPin) -> Self {
        self.a0 = a0;
        self
    }

    /// Set the address pin A1 configuration.
    pub fn addr_a1(mut self, a1: AddrPin) -> Self {
        self.a1 = a1;
        self
    }

    /// Set the address pin configuration for both A0 and A1.
    pub fn addr(mut self, addr: u8) -> Self {
        let a0 = AddrPin::from(addr);
        let a1 = AddrPin::from(addr >> 2);
        self.a0 = a0;
        self.a1 = a1;
        self
    }

    /// Set the ADC configuration.
    pub fn adc_config(mut self, config: AdcConfig) -> Self {
        self.adc_conf = config;
        self
    }

    /// Set the maximum expected current for calibration.
    pub fn max_expected_current(mut self, current: uom::si::f32::ElectricCurrent) -> Self {
        self.max_current = Some(current);
        self
    }

    /// Build the final Configuration.
    ///
    /// # Arguments
    /// * `shunt_resistance` - The shunt resistor value used for current measurement.
    ///
    /// # Returns
    /// A `Configuration` instance with the specified settings.
    ///
    /// # Note
    /// The current LSB is calculated based on the maximum expected current and the shunt
    /// resistance. In case the maximum expected current exceeds the limit imposed by the
    /// shunt voltage range, or it is not provided, it will be clamped to the maximum
    /// allowed value.
    pub fn build(self, shunt_resistance: uom::si::f32::ElectricalResistance) -> Configuration {
        let current_limit = uom::si::f32::ElectricPotential::new::<
                    uom::si::electric_potential::microvolt,
                >(65536.0 * 2.5) // Max shunt voltage = 2^16 * 2.5uV
                    / shunt_resistance;
        let _max_current = match self.max_current {
            Some(c) if c > current_limit => {
                #[cfg(feature = "defmt")]
                defmt::warn!(
                    "Max expected current {}A exceeds allowed maximum {}, clamping",
                    c.get::<uom::si::electric_current::ampere>(),
                    current_limit.get::<uom::si::electric_current::ampere>()
                );
                current_limit
            }
            Some(c) => c,
            None => current_limit,
        };
        let calibration = uom::si::f32::ElectricPotential::new::<uom::si::electric_potential::volt>(
            0.00512 * 32768.0,
        ) / (current_limit * shunt_resistance);
        let calibration = calibration.get::<uom::si::ratio::ratio>() as u16;
        Configuration {
            a0: self.a0,
            a1: self.a1,
            adc_conf: self.adc_conf,
            calibration,
            shunt: shunt_resistance,
        }
    }
}
