#![no_std]
#![allow(async_fn_in_trait)]

use core::slice;

use embassy_rp::clocks::clk_sys_freq;
use embassy_rp::dma::Channel;
use embassy_rp::gpio::{Drive, Level, Pull, SlewRate};
use embassy_rp::pio::program::pio_asm;
use embassy_rp::pio::{Common, Config, Direction, Instance, PioPin, ShiftDirection, StateMachine};
use embassy_rp::Peri;
use fixed::types::extra::U8;
use fixed::FixedU32;

/// Shift register reader driven by PIO.
pub struct PioShiftRegister<'d, PIO: Instance, const SM: usize, DMA: Channel> {
    sm: StateMachine<'d, PIO, SM>,
    dma: Peri<'d, DMA>,
}

/// Clock divider used with the RM2
/// With default core clock configuration:
/// RP2350: 150Mhz / 3 = 50Mhz pio clock -> 25Mhz GSPI clock
/// RP2040: 133Mhz / 3 = 44.33Mhz pio clock -> 22.16Mhz GSPI clock
pub const CLOCK_DIVIDER: FixedU32<U8> = FixedU32::from_bits(0x0300);

impl<'d, PIO, const SM: usize, DMA> PioShiftRegister<'d, PIO, SM, DMA>
where
    DMA: Channel,
    PIO: Instance,
{
    /// Create a new instance of [`PioShiftRegister`].
    pub fn new(
        common: &mut Common<'d, PIO>,
        mut sm: StateMachine<'d, PIO, SM>,
        clock_divider: FixedU32<U8>,
        load: Peri<'d, impl PioPin>,
        din: Peri<'d, impl PioPin>,
        clk: Peri<'d, impl PioPin>,
        dma: Peri<'d, DMA>,
    ) -> Self {
        let effective_pio_frequency =
            (clk_sys_freq() as f32 / clock_divider.to_num::<f32>()) as u32;

        #[cfg(feature = "defmt")]
        defmt::trace!("Effective pio frequency: {}Hz", effective_pio_frequency);

        // Non-integer pio clock dividers are achieved by introducing clock jitter resulting in a
        // combination of long and short cycles. The long and short cycles average to achieve the
        // requested clock speed.
        // This can be a problem for peripherals that expect a consistent clock / have a clock
        // speed upper bound that is violated by the short cycles. The cyw43 seems to handle the
        // jitter well, but we emit a warning to recommend an integer divider anyway.
        if clock_divider.frac() != FixedU32::<U8>::ZERO {
            #[cfg(feature = "defmt")]
            defmt::trace!(
                "Configured clock divider is not a whole number. Some clock cycles may violate the maximum recommended GSPI speed. Use at your own risk."
            );
        }

        // Different pio programs must be used for different pio clock speeds.
        // The programs used below are based on the pico SDK: https://github.com/raspberrypi/pico-sdk/blob/master/src/rp2_common/pico_cyw43_driver/cyw43_bus_pio_spi.pio
        // The clock speed cutoff for each program has been determined experimentally:
        // > 100Mhz -> Overclock program
        // [75Mhz, 100Mhz] -> High speed program
        // [0, 75Mhz) -> Low speed program
        let loaded_program = {
            // Adapted from [here](https://github.com/derekfountain/pico-shift-register-74xx165/blob/main/pio_version/shift_reg.pio)
            let prog = pio_asm!(
                ".side_set 1"

                ".wrap_target"
                "pull block    side 0x00"        // Stall, wait for signal
                /*
                 * Fig 8 in the datasheet shows the /PL pulse time as tW which is minimum 9.0ns.
                 * At 125MHz the RP2040 has a clock time of 8ns. So I need 2 clocks with /LD low
                 * to be sure. However this is moot because I want to read Q ASAP, and the 
                 * datasheet says the propagation time from /PL to Q is tPD, which is max of 22ns 
                 * at 3V3. So I need to wait at least 22ns for the Q value to become ready before 
                 * progressing. In theory the "set pins" instruction to bring down the /LD, plus 
                 * the "set pins" to put /LD back, plus the setting of the loop counter, gives a 
                 * pause of 3 RP2040 cycles, which is 24ns. That should be long enough, but it 
                 * doesn't work. I think the time it takes /LD to fall, which is about 3ns to come 
                 * down 1V5, makes the actual width of the pulse too short. I need to stall 2 more 
                 * cycles to make it work. I'm not sure why. 
                 */
                "set pins, 0   side 0x00    [2]" // Write 0 to the load pin, wait
                "set pins, 1   side 0x00"        // put load pin back

                "set x, 7      side 0x00"        // 8 bits to read
                /*
                 * Dropping from the /LD into the reading of the Q pin which is now ready.
                 * Fig 8 says I need to wait tW (9.0ns) (which I do above) then tREM 
                 * (which I think should be tREC, tREM doesn't appear anywhere else in 
                 * the document) before sending the clock high. rRec is 6.0ns which is 
                 * less than the time of the "in pins" instruction, so no extra delay needed.
                 */
                "lp:"
                "in pins, 1    side 0x00"        // Pick up Q
                /*
                 * Fig 7 says the clock needs to go high for half a clock cycle. At 50MMhz 
                 * that's 10ns. The first nop here is 8ns, so I stall another cycle. It 
                 * turns out that for my 74LV165 the extra cycle stall isn't necessary, the 
                 * shift register works without it, but it's correct so I keep it in. 
                 * Fig 7 also says that the output is ready on the Q pin at the halfway 
                 * point on the cycle, so no need to wait any longer before pulling clock 
                 * low again.
                 */
                "nop           side 0x01    [1]" // wait and drive clock high
                /*
                 * Fig 7 says that the Q pin is ready to read tPHL (same as tpd) after clock 
                 * is driven high. That's max 21.5ns on my chip. This jmp and a 2 cycle stall 
                 * represents a delay of 24ns so the data is ready on Q when I get back to the 
                 * "in pins". The delay here is actually 3 cycles, the extra one being needed 
                 * alongside the "side 0x00" to pull the clock pin low for 8ns. This negates the 
                 * need for the extra nop above, saving that instruction.
                 */
                "jmp x--,  lp  side 0x00    [3]" // Jump back for the next bit
                /* 
                 * All 8 bits are collected in the ISR. Push it back to the core. This triggers 
                 * the DREQ which the DMA is waiting on, so the core code continues with the 
                 * value pushed here magically in its variable.
                 */
                "push block    side 0x00"        // Push the read data to RX FIFO
                ".wrap"
            );
            common.load_program(&prog.program)
        };

        let mut pin_inp: embassy_rp::pio::Pin<PIO> = common.make_pio_pin(din);
        pin_inp.set_pull(Pull::None);
        pin_inp.set_schmitt(true);
        pin_inp.set_input_sync_bypass(true);

        let mut pin_load: embassy_rp::pio::Pin<PIO> = common.make_pio_pin(load);
        pin_load.set_pull(Pull::None);
        pin_load.set_drive_strength(Drive::_12mA);
        pin_load.set_slew_rate(SlewRate::Fast);

        let mut pin_clk = common.make_pio_pin(clk);
        pin_clk.set_drive_strength(Drive::_12mA);
        pin_clk.set_slew_rate(SlewRate::Fast);

        let mut cfg = Config::default();
        cfg.use_program(&loaded_program, &[&pin_clk]);
        cfg.set_out_pins(&[&pin_load]);
        cfg.set_in_pins(&[&pin_inp]);
        cfg.shift_in.direction = ShiftDirection::Left;
        cfg.shift_in.auto_fill = false;
        cfg.shift_in.threshold = 8;
        cfg.clock_divider = clock_divider;

        sm.set_config(&cfg);

        sm.set_pin_dirs(Direction::Out, &[&pin_clk, &pin_load]);
        sm.set_pin_dirs(Direction::In, &[&pin_inp]);
        sm.set_pins(Level::Low, &[&pin_clk]);
        sm.set_pins(Level::High, &[&pin_load]);

        Self { sm, dma }
    }

    /// Read a byte from the shift register.
    pub async fn read(&mut self) -> u8 {
        let mut status = 0;
        // Send dummy byte to clock data in
        self.sm
            .tx()
            .dma_push(self.dma.reborrow(), slice::from_ref(&status), false)
            .await;
        // Pull data out via DMA
        self.sm
            .rx()
            .dma_pull(self.dma.reborrow(), slice::from_mut(&mut status), false)
            .await;
        status
    }
}
