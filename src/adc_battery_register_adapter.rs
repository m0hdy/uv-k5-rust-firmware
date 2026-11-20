//! Isolated DP32G030 register adapter for composed SARADC battery sampling.

#[allow(dead_code, unused_attributes)]
#[path = "startup_battery_sampling.rs"]
mod startup_battery_sampling;

pub use startup_battery_sampling::{ReadRegister, WriteRegister};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use startup_battery_sampling::{RegisterIo, sample_startup_battery};
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use startup_battery_sampling::{StartupBatteryError, StartupBatteryReadings};

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

/// Acquires and calibrates four battery samples from the DP32G030.
///
/// # Safety
///
/// The caller must ensure the DP32G030 register map matches the mapped
/// addresses, the SARADC clock is enabled, and the ADC is already configured
/// for channel-memory sampling of channels 4 and 9 with software triggering.
/// The caller must have exclusive access to SARADC START, channel 9 STAT,
/// channels 4 and 9 DATA, and IF for the whole call. `poll_budget` independently
/// bounds each of the four acquisition polls. `calibration` must be the
/// calibration value selected by the caller; a zero value is rejected before
/// any register access.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn sample_startup_battery_mmio(
    poll_budget: usize,
    calibration: u16,
) -> Result<StartupBatteryReadings, StartupBatteryError> {
    sample_startup_battery(&mut VolatileBatteryAdcIo, poll_budget, calibration)
}
