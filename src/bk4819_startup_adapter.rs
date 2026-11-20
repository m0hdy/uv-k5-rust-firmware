//! Standalone BK4819 cold-initialization executor.

#[path = "bk4819_register_transport.rs"]
#[allow(dead_code)]
pub mod transport;

const DTMF_COEFFICIENTS: [u16; 16] = [
    111, 107, 103, 98, 80, 71, 58, 44, 65, 55, 37, 23, 228, 203, 181, 159,
];

fn write(
    io: &mut impl transport::WordIo,
    delay: &mut impl transport::Delay1us,
    reg: u8,
    value: u16,
) {
    transport::write_register(io, delay, reg, value);
}

fn set_agc(io: &mut impl transport::WordIo, delay: &mut impl transport::Delay1us) {
    let value = transport::read_register(io, delay, 0x7e);
    if value & 0x8000 != 0 {
        write(io, delay, 0x7e, (value & 0x0fff) | 0x3000);
    }
}

fn set_gpio_data_bit(io: &mut impl transport::WordIo, mask: u32) {
    let value = io.read_word(transport::Register::GpioCData);
    io.write_word(transport::Register::GpioCData, value | mask);
}

/// Executes the pinned cold initialization through the GPIO register transport.
///
/// The caller owns `gpio_output_shadow`; it is updated before register 0x33 is
/// written. The supplied I/O and delay implementations must not panic.
pub fn initialize_with_io(
    io: &mut impl transport::WordIo,
    delay: &mut impl transport::Delay1us,
    gpio_output_shadow: &mut u16,
) {
    set_gpio_data_bit(io, transport::SCN_MASK);
    set_gpio_data_bit(io, transport::SCL_MASK);
    set_gpio_data_bit(io, transport::SDA_MASK);

    write(io, delay, 0x00, 0x8000);
    write(io, delay, 0x00, 0x0000);
    write(io, delay, 0x37, 0x1d0f);
    write(io, delay, 0x36, 0x0022);

    write(io, delay, 0x13, 0x03be);
    write(io, delay, 0x12, 0x037b);
    write(io, delay, 0x11, 0x027b);
    write(io, delay, 0x10, 0x007a);
    write(io, delay, 0x14, 0x0019);
    write(io, delay, 0x49, 0x2a38);
    write(io, delay, 0x7b, 0x8420);

    set_agc(io, delay);

    write(io, delay, 0x19, 0x1041);
    write(io, delay, 0x7d, 0xe940);
    write(io, delay, 0x48, 0xb3a8);
    for (index, coefficient) in DTMF_COEFFICIENTS.into_iter().enumerate() {
        write(io, delay, 0x09, ((index as u16) << 12) | coefficient);
    }

    write(io, delay, 0x1f, 0x5454);
    write(io, delay, 0x3e, 0xa037);
    *gpio_output_shadow = 0x9000;
    write(io, delay, 0x33, *gpio_output_shadow);
    write(io, delay, 0x3f, 0x0000);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileWordIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl transport::WordIo for VolatileWordIo {
    fn read_word(&mut self, register: transport::Register) -> u32 {
        // SAFETY: The caller of initialize_mmio guarantees the DP32G030 mappings.
        unsafe { core::ptr::read_volatile(register.address() as *const u32) }
    }

    fn write_word(&mut self, register: transport::Register, value: u32) {
        // SAFETY: The caller of initialize_mmio guarantees the DP32G030 mappings.
        unsafe { core::ptr::write_volatile(register.address() as *mut u32, value) }
    }
}

/// Executes cold initialization using the existing volatile GPIO transport.
///
/// # Safety
///
/// The caller must ensure the DP32G030 GPIOC and PORTCON mappings are present
/// and accessible, GPIOC's SYSCON clock gate is enabled, and GPIOC0/1/2 are
/// muxed to GPIO, configured as outputs, with input-enable and open-drain
/// disabled. The register transport temporarily enables input for GPIOC2 while
/// reading and restores output direction. The caller must exclusively own the
/// BK4819 bus and serialize this whole call against every GPIOC DATA/DIR and
/// PORTC input-enable/open-drain writer, including interrupts and unrelated-bit
/// writers, because register updates use full-word read-modify-write. The
/// supplied one-microsecond delay must be accurate and non-panicking.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn initialize_mmio(delay: &mut impl transport::Delay1us, gpio_output_shadow: &mut u16) {
    initialize_with_io(&mut VolatileWordIo, delay, gpio_output_shadow);
}
