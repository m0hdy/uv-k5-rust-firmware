//! Fallible nonzero-frequency BK1080 initialization.

#[allow(unused_attributes)]
#[allow(dead_code)]
#[path = "bk1080_receive_adapter.rs"]
mod receive_adapter;

use crate::eeprom_read_transport::I2cTransport;
pub use receive_adapter::{DelayMs, ReceiveError, Register};

const GPIOB_DATA: usize = 0x4006_0800;
const PB15: u32 = 1 << 15;
const INITIALIZATION_TABLE: [u16; 34] = [
    0x0008, 0x1080, 0x0201, 0x0000, 0x40c0, 0x0a1f, 0x002e, 0x02ff, 0x5b11, 0x0000, 0x411e, 0x0000,
    0xce00, 0x0000, 0x0000, 0x1000, 0x3197, 0x0000, 0x13ff, 0x9852, 0x0000, 0x0000, 0x0008, 0x0000,
    0x51e1, 0xa8bc, 0x2645, 0x00e4, 0x1cd8, 0x3a50, 0xeae0, 0x3000, 0x0200, 0x0000,
];

pub trait WordIo {
    fn read_word(&mut self, address: usize) -> u32;
    fn write_word(&mut self, address: usize, value: u32);
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Initialized(bool);

impl Initialized {
    pub const fn new() -> Self {
        Self(false)
    }

    pub const fn is_initialized(self) -> bool {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationError<E> {
    ZeroFrequency,
    InvalidRegisterIndex(u8),
    Receive(ReceiveError<E>),
}

fn set_pb15<G: WordIo>(gpio: &mut G, asserted: bool) {
    let data = gpio.read_word(GPIOB_DATA);
    let data = if asserted { data | PB15 } else { data & !PB15 };
    gpio.write_word(GPIOB_DATA, data);
}

/// Initializes and tunes a nonzero-frequency receiver, retaining successful
/// first calibration in caller-owned state.
///
/// The GPIOB read-modify-write, all bus transactions, and delays must be
/// serialized against GPIOB DATA writers and other BK1080 users. The supplied
/// delays must be accurate, infallible, non-panicking, and must not access the
/// shared GPIO registers. On failure, a fresh GPIOB DATA read reasserts PB15
/// after the failed transaction's transport cleanup. Calibration state is
/// committed only after the table, both internal-register writes, and both
/// initialization delays complete.
pub fn initialize_with_io<T, G, D>(
    transport: &mut T,
    gpio: &mut G,
    delay: &mut D,
    frequency: u16,
    band: u8,
    initialized: &mut Initialized,
) -> Result<(), InitializationError<T::Error>>
where
    T: I2cTransport,
    G: WordIo,
    D: DelayMs,
{
    if frequency == 0 {
        return Err(InitializationError::ZeroFrequency);
    }

    set_pb15(gpio, false);

    let result = initialize_registers(transport, delay, frequency, band, initialized);
    if result.is_err() {
        set_pb15(gpio, true);
    }
    result
}

fn initialize_registers<T, D>(
    transport: &mut T,
    delay: &mut D,
    frequency: u16,
    band: u8,
    initialized: &mut Initialized,
) -> Result<(), InitializationError<T::Error>>
where
    T: I2cTransport,
    D: DelayMs,
{
    if initialized.is_initialized() {
        receive_adapter::write_register(transport, Register::PowerConfiguration, 0x0201)
            .map_err(InitializationError::Receive)?;
    } else {
        for (index, value) in INITIALIZATION_TABLE.iter().copied().enumerate() {
            let register = Register::from_index(index as u8)
                .ok_or(InitializationError::InvalidRegisterIndex(index as u8))?;
            receive_adapter::write_register(transport, register, value)
                .map_err(InitializationError::Receive)?;
        }

        delay.delay_ms(250);
        receive_adapter::write_register(transport, Register::Internal25, 0xa83c)
            .map_err(InitializationError::Receive)?;
        receive_adapter::write_register(transport, Register::Internal25, 0xa8bc)
            .map_err(InitializationError::Receive)?;
        delay.delay_ms(60);
        *initialized = Initialized(true);
    }

    receive_adapter::write_register(transport, Register::SystemConfiguration2, 0x0a1f)
        .map_err(InitializationError::Receive)?;
    receive_adapter::set_frequency(transport, delay, frequency, band)
        .map_err(InitializationError::Receive)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct TargetWords;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl WordIo for TargetWords {
    fn read_word(&mut self, address: usize) -> u32 {
        // SAFETY: the caller of initialize_mmio guarantees the register mapping.
        unsafe { core::ptr::read_volatile(address as *const u32) }
    }

    fn write_word(&mut self, address: usize, value: u32) {
        // SAFETY: the caller of initialize_mmio guarantees the register mapping.
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }
}

/// Performs nonzero-frequency initialization using the target GPIO bitbang bus.
///
/// # Safety
///
/// The caller must provide a DP32G030 with GPIOA, GPIOB, and PORTCON mapped,
/// GPIOA and GPIOB clocks enabled, and PB15 initialized as an output. PA10/PA11
/// must be muxed to GPIOA10/GPIOA11 and initialized as GPIO outputs, with
/// input-enable and open-drain disabled on both pins. The caller must
/// exclusively own the I2C bus, PB15, and keypad-shared PA10/PA11, and serialize
/// the complete operation—including delays, error cleanup, GPIOB
/// read-modify-writes, keypad handoff, and owner drop—against interrupts and
/// every GPIOA DATA/DIR, PORTA input-enable/open-drain, or GPIOB DATA writer,
/// including writers to unrelated bits. The one-microsecond and millisecond
/// providers must accurately implement every requested interval, be
/// infallible and non-panicking, and must not access these shared registers.
/// This includes each bitbang 1 us delay and the 250 ms, 60 ms, and 10 ms waits.
/// The caller owns `initialized`: preserve it across ordinary receiver
/// shutdown; set it back to `Initialized::new()` after hardware reset or any
/// loss of receiver register/calibration state.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn initialize_mmio<D, M>(
    frequency: u16,
    band: u8,
    delay_1us: D,
    delay_ms: &mut M,
    initialized: &mut Initialized,
) -> Result<(), InitializationError<core::convert::Infallible>>
where
    D: crate::i2c_bitbang_flow::target_gpio::Delay1us,
    M: DelayMs,
{
    use crate::eeprom_i2c_bridge::I2cBitbangTransport;
    use crate::i2c_bitbang_flow::target_gpio::GpioI2cIo;

    if frequency == 0 {
        return Err(InitializationError::ZeroFrequency);
    }

    // SAFETY: the caller upholds the mapping, timing, and exclusive ownership
    // requirements documented above for this complete operation.
    let mut pins = unsafe { GpioI2cIo::new_target(delay_1us) };
    let result = {
        let mut transport = I2cBitbangTransport::new(&mut pins);
        initialize_with_io(
            &mut transport,
            &mut TargetWords,
            delay_ms,
            frequency,
            band,
            initialized,
        )
    };
    pins.handoff_to_keypad();
    result
}
