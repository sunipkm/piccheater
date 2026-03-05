use uom::{
    num_traits::float::FloatCore,
    si::{
        electric_potential::{microvolt, volt},
        f32::{ElectricCurrent, ElectricPotential},
        ratio::ratio,
    },
};

use crate::{AddrPin, registers::AdcConfig};

#[derive(Default)]
/// Builder for INA233 driver configuration
pub struct ConfigurationBuilder {
    pub(crate) a0: AddrPin,
    pub(crate) a1: AddrPin,
    pub(crate) adc_conf: AdcConfig,
    pub(crate) lsb: Option<ElectricCurrent>,
}

/// INA233 driver configuration
pub struct Configuration {
    pub(crate) a0: AddrPin,
    pub(crate) a1: AddrPin,
    pub(crate) adc_conf: AdcConfig,
    pub(crate) calibration: u16,
    pub(crate) lsb: ElectricCurrent,
}

#[cfg(feature = "defmt")]
impl defmt::Format for Configuration {
    fn format(&self, fmt: defmt::Formatter) {
        use uom::si::electric_current::microampere;

        defmt::write!(
            fmt,
            "Address A0={}, A1={}, ADC={:#?}, CAL={:x}h, LSB={}",
            self.a0,
            self.a1,
            self.adc_conf,
            self.calibration,
            self.lsb.get::<microampere>()
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
    pub fn address(mut self, addr: u8) -> Self {
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

    /// Set the current LSB.
    pub fn current_lsb(mut self, lsb: ElectricCurrent) -> Self {
        self.lsb = Some(lsb);
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
        let max_lsb = ElectricPotential::new::<microvolt>(2.56) / shunt_resistance; // Max LSB based on shunt voltage range
        let lsb = self.lsb.unwrap_or_else(|| {
            #[cfg(feature = "defmt")]
            {
                use uom::si::electric_current::microampere;

                defmt::warn!(
                    "[INA233] No current LSB provided. Defaulting to max possible LSB of {} uA based on shunt resistance.",
                    max_lsb.get::<microampere>()
                );
            }
            max_lsb
        }); // Default to max possible LSB if not provided
        if lsb > max_lsb {
            #[cfg(feature = "defmt")]
            {
                use uom::si::electric_current::microampere;

                defmt::warn!(
                    "[INA233] Provided current LSB {} uA exceeds maximum allowed {} uA based on shunt resistance. Clamping to max.",
                    lsb.get::<microampere>(),
                    max_lsb.get::<microampere>(),
                );
            }
        }
        let lsb = lsb.min(max_lsb);
        let calibration = ElectricPotential::new::<volt>(0.00512) / (lsb * shunt_resistance);
        let calibration = calibration.get::<ratio>().floor() as u16;
        let lsb = ElectricPotential::new::<volt>(0.00512) / (calibration as f32 * shunt_resistance);
        #[cfg(feature = "defmt")]
        {
            use uom::si::{electric_current::microampere, electrical_resistance::milliohm};

            defmt::debug!(
                "[INA233@{:#02x}] Calculated calibration value: {} (LSB: {} uA, Shunt: {} Ω)",
                u8::from(&self),
                calibration,
                lsb.get::<microampere>(),
                shunt_resistance.get::<milliohm>()
            );
        }
        Configuration {
            a0: self.a0,
            a1: self.a1,
            adc_conf: self.adc_conf,
            calibration,
            lsb,
        }
    }
}
