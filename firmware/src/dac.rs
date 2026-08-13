use core::{str::FromStr as _, sync::atomic::Ordering};

use dacx578::{
    Address, AsyncFunctions as _, Channels, ClearCode, Configuration, DacX578, PowerDownMode,
    ResetMode, configure_all_async,
};
use defmt::{debug, error, trace};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_rp::{
    bind_interrupts,
    gpio::Output,
    i2c::{Config, I2c, InterruptHandler},
    peripherals::I2C0,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Ticker};
use heapless::{String, format};
use uom::si::{electric_potential::millivolt, f32::ElectricPotential};

use crate::{CommandReceiver, ResponseSender, commands::Commands, resources::DacDev};

use crate::usb::DAC_READY;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

#[embassy_executor::task]
pub async fn dac_task(dac: DacDev, receiver: CommandReceiver, sender: ResponseSender) {
    // Initialize the shared I2C bus and DAC control pins
    let i2c_config = Config::default();
    let i2c =
        Mutex::<NoopRawMutex, _>::new(I2c::new_async(dac.i2c, dac.scl, dac.sda, Irqs, i2c_config));
    let mut dac_en = Output::new(dac.en, embassy_rp::gpio::Level::Low);
    let mut ticker = Ticker::every(Duration::from_secs(1));
    const ADDR_LEN: usize = dacx578::Address::address_range().len();
    let mut addrs = heapless::Vec::<u8, ADDR_LEN>::new();
    'main: loop {
        // Reset
        addrs.clear();
        DAC_READY.store(false, Ordering::SeqCst);
        // Enumerate devices
        {
            let mut i2c = i2c.lock().await;
            while addrs.is_empty() {
                for addr in dacx578::Address::address_range() {
                    i2c.blocking_read(addr, &mut [0; 1])
                        .is_ok()
                        .then(|| addrs.push(addr).ok());
                }
                ticker.next().await;
            }
        }
        debug!("DACs found at addresses: {:#02x}", addrs);
        // Initialize devices
        let mut dacs = heapless::LinearMap::<u8, DacX578<_, _>, ADDR_LEN>::new();

        for addr in addrs.iter() {
            let mut dac = DacX578::new(
                I2cDevice::new(&i2c),
                Address::from(*addr),
                ElectricPotential::new::<millivolt>(2048.0),
            );
            if let Err(e) = dac.reset(ResetMode::Por).await {
                log::error!("Failed to reset DAC at address {:#02x}: {:?}", addr, e);
                error!("Failed to reset DAC at address {:#02x}: {:?}", addr, e);
                continue 'main;
            } else {
                debug!("DAC at address {:#02x} reset successfully", addr);
                dacs.insert(*addr, dac).ok();
            }
        }
        // Configure defaults
        {
            let mut i2c = I2cDevice::new(&i2c);
            if let Err(e) =
                configure_all_async(&mut i2c, Configuration::ClearCode(ClearCode::MidScale)).await
            {
                error!("Failed to set DAC clear code: {:?}", e);
                log::error!("Failed to set DAC clear code: {:?}", e);
            }
            #[cfg(feature = "midscale")]
            {
                for dac in dacs.values_mut() {
                    use dacx578::Channel;
                    dac.write_and_update(Channel::All, ElectricPotential::new::<millivolt>(1000.0))
                        .await
                        .inspect_err(|e| {
                            error!("Failed to set DAC to mid-scale: {:?}", e);
                            log::error!("Failed to set DAC to mid-scale: {:?}", e);
                        })
                        .ok();
                }
            }
            if let Err(e) = configure_all_async(
                &mut i2c,
                Configuration::PowerDown {
                    mode: PowerDownMode::HighZ,
                    channels: Channels::all(),
                },
            )
            .await
            {
                error!("Failed to set DACs sto power down with High Z: {:?}", e);
                log::error!("Failed to set DACs to power down with High Z: {:?}", e);
            }
        }
        DAC_READY.store(true, Ordering::SeqCst);
        trace!(
            "DACs initialized and ready for commands: {}",
            DAC_READY.load(Ordering::SeqCst)
        );
        // Clear all commands up to this point
        receiver.clear();
        #[allow(irrefutable_let_patterns)]
        while let cmd = receiver.receive().await {
            let resp = match cmd {
                Commands::ReadDac { dac, channel } => {
                    let key = dac as u8;
                    if let Some(ddac) = dacs.get_mut(&key) {
                        match ddac.read(dacx578::Register::ChannelDac(channel)).await {
                            Ok(v) => ("Read from DAC", Ok(format!("{:?}", v).unwrap_or_default())),
                            Err(e) => {
                                sender
                                    .send((
                                        "Failed to read from DAC",
                                        Err(format!("{:?}", e).unwrap_or_default()),
                                    ))
                                    .await;
                                continue 'main;
                            }
                        }
                    } else {
                        trace!("DAC at address {:#02x} not found for read", key);
                        (
                            "Read from DAC",
                            Ok(String::from_str("DAC not found").unwrap()),
                        )
                    }
                }
                Commands::WriteDac {
                    dac,
                    channel,
                    value,
                } => {
                    let key = dac as u8;
                    let value = ElectricPotential::new::<millivolt>(value);
                    if let Some(ddac) = dacs.get_mut(&key) {
                        match ddac.write_and_update(channel, value).await {
                            Ok(()) => ("Write to DAC", Ok(String::from_str("Success").unwrap())),
                            Err(e) => {
                                sender
                                    .send((
                                        "Failed to write to DAC",
                                        Err(format!("{:?}", e).unwrap_or_default()),
                                    ))
                                    .await;
                                continue 'main;
                            }
                        }
                    } else {
                        trace!("DAC at address {:#02x} not found for write", key);
                        (
                            "Write to DAC",
                            Ok(String::from_str("DAC not found").unwrap()),
                        )
                    }
                }
                Commands::EnableOutputs => {
                    let mut i2c = I2cDevice::new(&i2c);
                    if let Err(e) = configure_all_async(
                        &mut i2c,
                        Configuration::PowerDown {
                            mode: PowerDownMode::Normal,
                            channels: Channels::all(),
                        },
                    )
                    .await
                    {
                        error!("Failed to power up DACs: {:?}", e);
                        log::error!("Failed to power up DACs: {:?}", e);
                    }
                    dac_en.set_high();
                    (
                        "Enable AMP EN pin",
                        Ok(String::from_str("Success").unwrap()),
                    )
                }
                Commands::DisableOutputs | Commands::AllOff => {
                    #[cfg(feature = "midscale")]
                    {
                        for dac in dacs.values_mut() {
                            use dacx578::Channel;
                            dac.write_and_update(
                                Channel::All,
                                ElectricPotential::new::<millivolt>(1000.0),
                            )
                            .await
                            .inspect_err(|e| {
                                error!("Failed to set DAC to mid-scale: {:?}", e);
                                log::error!("Failed to set DAC to mid-scale: {:?}", e);
                            })
                            .ok();
                        }
                    }
                    let mut i2c = I2cDevice::new(&i2c);
                    if let Err(e) = configure_all_async(
                        &mut i2c,
                        Configuration::PowerDown {
                            mode: PowerDownMode::HighZ,
                            channels: Channels::all(),
                        },
                    )
                    .await
                    {
                        error!("Failed to power down DACs with High Z: {:?}", e);
                        log::error!("Failed to power down DACs with High Z: {:?}", e);
                    }
                    dac_en.set_low();
                    (
                        "Disable AMP EN pin",
                        Ok(String::from_str("Success").unwrap()),
                    )
                }
                _ => (
                    "Unknown command",
                    Err(String::from_str("Command not recognized").unwrap()),
                ),
            };
            sender.send(resp).await;
        }
        // main loop runs every second
        ticker.next().await;
    }
}
