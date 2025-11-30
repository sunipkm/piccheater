#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::{Executor, Spawner};
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Instant, Timer};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};
