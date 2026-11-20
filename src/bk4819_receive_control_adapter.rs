//! Caller-driven BK4819 receive power-control operations.

#[allow(dead_code)]
#[path = "bk4819_register_transport.rs"]
mod transport;

pub use transport::{Delay1us, Register, WordIo};

const REG_30_ENABLE_VCO_CALIB: u16 = 1 << 15;
const REG_30_ENABLE_RX_LINK: u16 = 0xF << 10;
const REG_30_ENABLE_AF_DAC: u16 = 1 << 9;
const REG_30_ENABLE_DISC_MODE: u16 = 1 << 8;
const REG_30_ENABLE_PLL_VCO: u16 = 0xF << 4;
const REG_30_ENABLE_RX_DSP: u16 = 1;

const RECEIVE_ON_REG_30: u16 = REG_30_ENABLE_VCO_CALIB
    | REG_30_ENABLE_RX_LINK
    | REG_30_ENABLE_AF_DAC
    | REG_30_ENABLE_DISC_MODE
    | REG_30_ENABLE_PLL_VCO
    | REG_30_ENABLE_RX_DSP;

/// Executes the pinned receive-on register writes through the supplied GPIO
/// register transport and one-microsecond delay provider.
pub fn receive_on(io: &mut impl WordIo, delay: &mut impl Delay1us) {
    transport::write_register(io, delay, 0x37, 0x1F0F);
    transport::write_register(io, delay, 0x30, 0x0000);
    transport::write_register(io, delay, 0x30, RECEIVE_ON_REG_30);
}

/// Executes the pinned sleep register writes through the supplied GPIO
/// register transport and one-microsecond delay provider.
pub fn sleep(io: &mut impl WordIo, delay: &mut impl Delay1us) {
    transport::write_register(io, delay, 0x30, 0x0000);
    transport::write_register(io, delay, 0x37, 0x1D00);
}

/// Executes the pinned idle register write through the supplied GPIO register
/// transport and one-microsecond delay provider.
pub fn idle(io: &mut impl WordIo, delay: &mut impl Delay1us) {
    transport::write_register(io, delay, 0x30, 0x0000);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
/// Executes receive-on using volatile DP32G030 GPIOC/PORTCON accesses.
///
/// # Safety
///
/// The caller must ensure the DP32G030 GPIOC and PORTCON mappings are present
/// and accessible, the GPIOC clock is enabled, and GPIOC0/1/2 are muxed to
/// GPIO, configured as outputs, and have input-enable and open-drain disabled
/// before entry. The BK4819 must already be initialized. The caller must
/// exclusively own the BK4819 GPIO bus for this full operation and serialize
/// it against every writer to GPIOC DATA/DIR and PORTC input-enable/open-drain,
/// including interrupts and writers to unrelated bits in those full-word
/// read-modify-write registers. The supplied one-microsecond delay must be
/// accurate and non-panicking.
pub unsafe fn receive_on_mmio(delay: &mut impl Delay1us) {
    // SAFETY: The caller guarantees the GPIOC/PORTCON MMIO preconditions.
    unsafe { transport::write_register_mmio(delay, 0x37, 0x1F0F) };
    // SAFETY: The same caller guarantees remain in force for the whole call.
    unsafe { transport::write_register_mmio(delay, 0x30, 0x0000) };
    // SAFETY: The same caller guarantees remain in force for the whole call.
    unsafe { transport::write_register_mmio(delay, 0x30, RECEIVE_ON_REG_30) };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
/// Executes sleep using volatile DP32G030 GPIOC/PORTCON accesses.
///
/// # Safety
///
/// The caller must ensure the DP32G030 GPIOC and PORTCON mappings are present
/// and accessible, the GPIOC clock is enabled, and GPIOC0/1/2 are muxed to
/// GPIO, configured as outputs, and have input-enable and open-drain disabled
/// before entry. The BK4819 must already be initialized. The caller must
/// exclusively own the BK4819 GPIO bus for this full operation and serialize
/// it against every writer to GPIOC DATA/DIR and PORTC input-enable/open-drain,
/// including interrupts and writers to unrelated bits in those full-word
/// read-modify-write registers. The supplied one-microsecond delay must be
/// accurate and non-panicking.
pub unsafe fn sleep_mmio(delay: &mut impl Delay1us) {
    // SAFETY: The caller guarantees the GPIOC/PORTCON MMIO preconditions.
    unsafe { transport::write_register_mmio(delay, 0x30, 0x0000) };
    // SAFETY: The same caller guarantees remain in force for the whole call.
    unsafe { transport::write_register_mmio(delay, 0x37, 0x1D00) };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
/// Executes idle using volatile DP32G030 GPIOC/PORTCON accesses.
///
/// # Safety
///
/// The caller must ensure the DP32G030 GPIOC and PORTCON mappings are present
/// and accessible, the GPIOC clock is enabled, and GPIOC0/1/2 are muxed to
/// GPIO, configured as outputs, and have input-enable and open-drain disabled
/// before entry. The BK4819 must already be initialized. The caller must
/// exclusively own the BK4819 GPIO bus for this full operation and serialize
/// it against every writer to GPIOC DATA/DIR and PORTC input-enable/open-drain,
/// including interrupts and writers to unrelated bits in those full-word
/// read-modify-write registers. The supplied one-microsecond delay must be
/// accurate and non-panicking.
pub unsafe fn idle_mmio(delay: &mut impl Delay1us) {
    // SAFETY: The caller guarantees the GPIOC/PORTCON MMIO preconditions.
    unsafe { transport::write_register_mmio(delay, 0x30, 0x0000) };
}
