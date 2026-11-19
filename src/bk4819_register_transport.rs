//! Standalone GPIO serial transport for BK4819 8-bit register addresses and
//! 16-bit values.

pub const GPIOC_DATA: usize = 0x4006_1000;
pub const GPIOC_DIRECTION: usize = 0x4006_1004;
pub const PORTC_INPUT_ENABLE: usize = 0x400b_0108;

pub const SCN_MASK: u32 = 1 << 0;
pub const SCL_MASK: u32 = 1 << 1;
pub const SDA_MASK: u32 = 1 << 2;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Register {
    GpioCData,
    GpioCDirection,
    PortCInputEnable,
}

impl Register {
    pub const fn address(self) -> usize {
        match self {
            Self::GpioCData => GPIOC_DATA,
            Self::GpioCDirection => GPIOC_DIRECTION,
            Self::PortCInputEnable => PORTC_INPUT_ENABLE,
        }
    }
}

pub trait WordIo {
    fn read_word(&mut self, register: Register) -> u32;
    fn write_word(&mut self, register: Register, value: u32);
}

pub trait Delay1us {
    fn delay_1us(&mut self);
}

fn update_bit(io: &mut impl WordIo, register: Register, mask: u32, high: bool) {
    let current = io.read_word(register);
    let updated = if high {
        current | mask
    } else {
        current & !mask
    };
    io.write_word(register, updated);
}

fn scn(io: &mut impl WordIo, high: bool) {
    update_bit(io, Register::GpioCData, SCN_MASK, high);
}

fn scl(io: &mut impl WordIo, high: bool) {
    update_bit(io, Register::GpioCData, SCL_MASK, high);
}

fn sda(io: &mut impl WordIo, high: bool) {
    update_bit(io, Register::GpioCData, SDA_MASK, high);
}

fn write_u8(io: &mut impl WordIo, delay: &mut impl Delay1us, mut value: u8) {
    scl(io, false);
    for _ in 0..8 {
        sda(io, value & 0x80 != 0);
        delay.delay_1us();
        scl(io, true);
        delay.delay_1us();
        value <<= 1;
        scl(io, false);
        delay.delay_1us();
    }
}

fn write_u16(io: &mut impl WordIo, delay: &mut impl Delay1us, mut value: u16) {
    scl(io, false);
    for _ in 0..16 {
        sda(io, value & 0x8000 != 0);
        delay.delay_1us();
        scl(io, true);
        value <<= 1;
        delay.delay_1us();
        scl(io, false);
        delay.delay_1us();
    }
}

fn read_u16(io: &mut impl WordIo, delay: &mut impl Delay1us) -> u16 {
    update_bit(io, Register::PortCInputEnable, SDA_MASK, true);
    update_bit(io, Register::GpioCDirection, SDA_MASK, false);
    delay.delay_1us();

    let mut value = 0u16;
    for _ in 0..16 {
        value <<= 1;
        value |= u16::from(io.read_word(Register::GpioCData) & SDA_MASK != 0);
        scl(io, true);
        delay.delay_1us();
        scl(io, false);
        delay.delay_1us();
    }

    update_bit(io, Register::PortCInputEnable, SDA_MASK, false);
    update_bit(io, Register::GpioCDirection, SDA_MASK, true);
    value
}

/// Executes the pinned BK4819 read transaction for one raw 8-bit register address.
pub fn read_register(io: &mut impl WordIo, delay: &mut impl Delay1us, register: u8) -> u16 {
    scn(io, true);
    scl(io, false);
    delay.delay_1us();

    scn(io, false);
    write_u8(io, delay, register | 0x80);
    let value = read_u16(io, delay);
    scn(io, true);

    delay.delay_1us();
    scl(io, true);
    sda(io, true);
    value
}

/// Executes the pinned BK4819 write transaction for one raw 8-bit register address.
pub fn write_register(io: &mut impl WordIo, delay: &mut impl Delay1us, register: u8, value: u16) {
    scn(io, true);
    scl(io, false);
    delay.delay_1us();

    scn(io, false);
    write_u8(io, delay, register);
    delay.delay_1us();
    write_u16(io, delay, value);
    delay.delay_1us();

    scn(io, true);
    delay.delay_1us();
    scl(io, true);
    sda(io, true);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct VolatileWordIo;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl WordIo for VolatileWordIo {
    fn read_word(&mut self, register: Register) -> u32 {
        // SAFETY: The MMIO entry points require the DP32G030 register map.
        unsafe { core::ptr::read_volatile(register.address() as *const u32) }
    }

    fn write_word(&mut self, register: Register, value: u32) {
        // SAFETY: The MMIO entry points require the DP32G030 register map.
        unsafe { core::ptr::write_volatile(register.address() as *mut u32, value) }
    }
}

/// Executes a BK4819 register read using volatile DP32G030 GPIOC/PORTC accesses.
///
/// # Safety
///
/// The caller must ensure the DP32G030 GPIOC and PORTCON mappings are present
/// and accessible, the GPIOC clock is enabled, and GPIOC0/1/2 are muxed to
/// GPIO, configured as outputs, and have input-enable and open-drain disabled
/// before entry. The caller must exclusively own the BK4819 GPIO bus for this
/// full transaction and serialize it against every writer to GPIOC DATA/DIR
/// and PORTC input-enable, including interrupts and writers to unrelated bits
/// in those full-word RMW registers. The supplied one-microsecond delay must be
/// accurate and non-panicking.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn read_register_mmio(delay: &mut impl Delay1us, register: u8) -> u16 {
    read_register(&mut VolatileWordIo, delay, register)
}

/// Executes a BK4819 register write using volatile DP32G030 GPIOC/PORTC accesses.
///
/// # Safety
///
/// The caller must ensure the DP32G030 GPIOC and PORTCON mappings are present
/// and accessible, the GPIOC clock is enabled, and GPIOC0/1/2 are muxed to
/// GPIO, configured as outputs, and have input-enable and open-drain disabled
/// before entry. The caller must exclusively own the BK4819 GPIO bus for this
/// full transaction and serialize it against every writer to GPIOC DATA/DIR
/// and PORTC input-enable, including interrupts and writers to unrelated bits
/// in those full-word RMW registers. The supplied one-microsecond delay must be
/// accurate and non-panicking.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn write_register_mmio(delay: &mut impl Delay1us, register: u8, value: u16) {
    write_register(&mut VolatileWordIo, delay, register, value)
}
