#![cfg_attr(not(test), no_std)]

//! Isolated DP32G030 register adapter for bounded SARADC battery acquisition.

#[allow(dead_code, unused_attributes)]
#[path = "adc_battery_acquisition.rs"]
mod adc_battery_acquisition;

pub use adc_battery_acquisition::{ReadRegister, WriteRegister};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use adc_battery_acquisition::RegisterIo;
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use adc_battery_acquisition::{AcquisitionError, BatterySamples};

const SARADC_BASE: usize = 0x400b_a000;
const SARADC_START: usize = SARADC_BASE + 0x04;
const SARADC_IF: usize = SARADC_BASE + 0x0c;
const SARADC_CHANNELS: usize = SARADC_BASE + 0x10;
const ADC_CHANNEL_STRIDE: usize = 2 * core::mem::size_of::<u32>();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadRegisterAddress {
    Start,
    Channel9Status,
    Channel4Data,
    Channel9Data,
}

impl ReadRegisterAddress {
    pub const fn address(self) -> usize {
        match self {
            Self::Start => SARADC_START,
            Self::Channel9Status => SARADC_CHANNELS + 9 * ADC_CHANNEL_STRIDE,
            Self::Channel4Data => SARADC_CHANNELS + 4 * ADC_CHANNEL_STRIDE + 4,
            Self::Channel9Data => SARADC_CHANNELS + 9 * ADC_CHANNEL_STRIDE + 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteRegisterAddress {
    Start,
    InterruptFlags,
}

impl WriteRegisterAddress {
    pub const fn address(self) -> usize {
        match self {
            Self::Start => SARADC_START,
            Self::InterruptFlags => SARADC_IF,
        }
    }
}

pub const fn read_register_address(register: ReadRegister) -> ReadRegisterAddress {
    match register {
        ReadRegister::Start => ReadRegisterAddress::Start,
        ReadRegister::Channel9Status => ReadRegisterAddress::Channel9Status,
        ReadRegister::Channel4Data => ReadRegisterAddress::Channel4Data,
        ReadRegister::Channel9Data => ReadRegisterAddress::Channel9Data,
    }
}

pub const fn write_register_address(register: WriteRegister) -> WriteRegisterAddress {
    match register {
        WriteRegister::Start => WriteRegisterAddress::Start,
        WriteRegister::InterruptFlags => WriteRegisterAddress::InterruptFlags,
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileBatteryAdcIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RegisterIo for VolatileBatteryAdcIo {
    fn read(&mut self, register: ReadRegister) -> u32 {
        let address = read_register_address(register).address() as *const u32;
        // SAFETY: The caller of acquire_battery_samples_mmio guarantees the DP32G030 mapping.
        unsafe { core::ptr::read_volatile(address) }
    }

    fn write(&mut self, register: WriteRegister, value: u32) {
        let address = write_register_address(register).address() as *mut u32;
        // SAFETY: The caller of acquire_battery_samples_mmio guarantees the DP32G030 mapping.
        unsafe { core::ptr::write_volatile(address, value) }
    }
}

/// Acquires channel 4 voltage and channel 9 current samples from the DP32G030.
///
/// # Safety
///
/// The caller must ensure the DP32G030 register map matches the mapped
/// addresses, the SARADC clock is enabled, and the ADC is already configured
/// for channel-memory sampling of channels 4 and 9 with software triggering.
/// The caller must have exclusive access to SARADC START, channel 9 STAT,
/// channels 4 and 9 DATA, and IF for the whole call. Polling is bounded by the
/// supplied `poll_budget`; this function does not choose a timeout policy.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn acquire_battery_samples_mmio(
    poll_budget: usize,
) -> Result<BatterySamples, AcquisitionError> {
    adc_battery_acquisition::acquire_battery_samples(&mut VolatileBatteryAdcIo, poll_budget)
}
