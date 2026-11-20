#![cfg_attr(not(test), no_std)]

//! Pinned board SARADC startup sequence and DP32G030 target boundary.
//!
//! The sequence follows `BOARD_ADC_Init` in `board.c`, `ADC_Configure`,
//! `ADC_Enable`, and `ADC_SoftReset` in `driver/adc.c`, and the DP32G030
//! definitions in `bsp/dp32g030/{saradc,syscon}.h` at
//! [`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
//! The board startup dispatcher reuses this executor; callers supply EXTTRIG_SEL.

const DEVICE_CLK_GATE_SARADC: u32 = 1 << 25;

const SYSCON_CLK_SEL_W_PLL_SHIFT: u32 = 7;
const SYSCON_CLK_SEL_R_PLL_SHIFT: u32 = 11;
const SYSCON_CLK_SEL_R_PLL_MASK: u32 = 1 << SYSCON_CLK_SEL_R_PLL_SHIFT;
const FW_R_SARADC_SMPL_SHIFT: u32 = 7;
const FW_R_SARADC_SMPL_MASK: u32 = 3 << FW_R_SARADC_SMPL_SHIFT;
const SYSCON_CLK_SEL_W_SARADC_SMPL_SHIFT: u32 = 10;
const SYSCON_CLK_SEL_W_SARADC_SMPL_MASK: u32 = 3 << SYSCON_CLK_SEL_W_SARADC_SMPL_SHIFT;
const ADC_CLK_SEL_DIV2: u32 = 1;

const SARADC_CFG_CH_SEL_MASK: u32 = 0x7fff;
const SARADC_CFG_AVG_MASK: u32 = 3 << 16;
const SARADC_CFG_CONT_MASK: u32 = 1 << 18;
const SARADC_CFG_SMPL_SETUP_MASK: u32 = 7 << 19;
const SARADC_CFG_MEM_MODE_MASK: u32 = 1 << 22;
const SARADC_CFG_SMPL_CLK_MASK: u32 = 1 << 23;
const SARADC_CFG_SMPL_WIN_MASK: u32 = 7 << 24;
const SARADC_CFG_ADC_EN_MASK: u32 = 1 << 27;
const SARADC_CFG_ADC_TRIG_MASK: u32 = 1 << 28;
const SARADC_CFG_DMA_EN_MASK: u32 = 1 << 29;
const SARADC_CFG_CONFIGURATION_MASK: u32 = SARADC_CFG_CH_SEL_MASK
    | SARADC_CFG_AVG_MASK
    | SARADC_CFG_CONT_MASK
    | SARADC_CFG_SMPL_SETUP_MASK
    | SARADC_CFG_MEM_MODE_MASK
    | SARADC_CFG_SMPL_CLK_MASK
    | SARADC_CFG_SMPL_WIN_MASK
    | SARADC_CFG_ADC_TRIG_MASK
    | SARADC_CFG_DMA_EN_MASK;
const SARADC_CFG_BOARD_BITS: u32 =
    (1 << 4) | (1 << 9) | (3 << 16) | (1 << 22) | (1 << 23) | (7 << 24);

const SARADC_CALIB_OFFSET_VALID_MASK: u32 = 1 << 16;
const SARADC_CALIB_KD_VALID_MASK: u32 = 1 << 16;
const SARADC_START_SOFT_RESET_MASK: u32 = 1 << 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register {
    DeviceClockGate,
    ClockSelector,
    Configuration,
    ExternalTriggerSelector,
    CalibrationOffset,
    CalibrationKd,
    InterruptFlags,
    InterruptEnable,
    Start,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interrupt {
    Saradc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrqAction {
    Disable,
}

pub trait RegisterIo {
    fn read(&mut self, register: Register) -> u32;
    fn write(&mut self, register: Register, value: u32);
    fn set_irq(&mut self, interrupt: Interrupt, action: IrqAction);
}

fn disable(io: &mut impl RegisterIo) {
    let configuration = io.read(Register::Configuration);
    io.write(
        Register::Configuration,
        configuration & !SARADC_CFG_ADC_EN_MASK,
    );
}

fn configure_clock(io: &mut impl RegisterIo) {
    let clock_selector = io.read(Register::ClockSelector);
    let remapped = (clock_selector & !(SYSCON_CLK_SEL_R_PLL_MASK | FW_R_SARADC_SMPL_MASK))
        | (((clock_selector & SYSCON_CLK_SEL_R_PLL_MASK) >> SYSCON_CLK_SEL_R_PLL_SHIFT)
            << SYSCON_CLK_SEL_W_PLL_SHIFT)
        | (((clock_selector & FW_R_SARADC_SMPL_MASK) >> FW_R_SARADC_SMPL_SHIFT)
            << SYSCON_CLK_SEL_W_SARADC_SMPL_SHIFT);
    let configured = (remapped & !SYSCON_CLK_SEL_W_SARADC_SMPL_MASK)
        | ((ADC_CLK_SEL_DIV2 << SYSCON_CLK_SEL_W_SARADC_SMPL_SHIFT)
            & SYSCON_CLK_SEL_W_SARADC_SMPL_MASK);
    io.write(Register::ClockSelector, configured);
}

fn configure(io: &mut impl RegisterIo, exttrig_sel: u16) {
    io.set_irq(Interrupt::Saradc, IrqAction::Disable);

    let device_clock_gate = io.read(Register::DeviceClockGate);
    io.write(
        Register::DeviceClockGate,
        (device_clock_gate & !DEVICE_CLK_GATE_SARADC) | DEVICE_CLK_GATE_SARADC,
    );

    disable(io);
    configure_clock(io);

    let configuration = io.read(Register::Configuration);
    io.write(
        Register::Configuration,
        (configuration & !SARADC_CFG_CONFIGURATION_MASK) | SARADC_CFG_BOARD_BITS,
    );

    io.write(Register::ExternalTriggerSelector, u32::from(exttrig_sel));

    let calibration_offset = io.read(Register::CalibrationOffset);
    io.write(
        Register::CalibrationOffset,
        (calibration_offset & !SARADC_CALIB_OFFSET_VALID_MASK) | SARADC_CALIB_OFFSET_VALID_MASK,
    );

    let calibration_kd = io.read(Register::CalibrationKd);
    io.write(
        Register::CalibrationKd,
        (calibration_kd & !SARADC_CALIB_KD_VALID_MASK) | SARADC_CALIB_KD_VALID_MASK,
    );

    io.write(Register::InterruptFlags, u32::MAX);

    io.write(Register::InterruptEnable, 0);
}

pub fn apply_with_io(io: &mut impl RegisterIo, exttrig_sel: u16) {
    configure(io, exttrig_sel);

    let configuration = io.read(Register::Configuration);
    io.write(
        Register::Configuration,
        configuration | SARADC_CFG_ADC_EN_MASK,
    );

    let start = io.read(Register::Start);
    io.write(Register::Start, start & !SARADC_START_SOFT_RESET_MASK);

    let start = io.read(Register::Start);
    io.write(
        Register::Start,
        (start & !SARADC_START_SOFT_RESET_MASK) | SARADC_START_SOFT_RESET_MASK,
    );
}

#[cfg(any(test, all(target_arch = "arm", target_os = "none")))]
pub(crate) mod physical {
    use super::{Interrupt, IrqAction, Register, RegisterIo, apply_with_io};
    // Pinned DP32G030 syscon.h and saradc.h declare these as volatile uint32_t.
    const SYSCON_BASE: usize = 0x4000_0000;
    const SARADC_BASE: usize = 0x400b_a000;

    fn address(register: Register) -> usize {
        match register {
            Register::DeviceClockGate => SYSCON_BASE + 0x08,
            Register::ClockSelector => SYSCON_BASE,
            Register::Configuration => SARADC_BASE,
            Register::ExternalTriggerSelector => SARADC_BASE + 0xb0,
            Register::CalibrationOffset => SARADC_BASE + 0xf0,
            Register::CalibrationKd => SARADC_BASE + 0xf4,
            Register::InterruptFlags => SARADC_BASE + 0x0c,
            Register::InterruptEnable => SARADC_BASE + 0x08,
            Register::Start => SARADC_BASE + 0x04,
        }
    }

    // Pinned bsp/dp32g030/irq.h and CMSIS core_cm0.h (one NVIC bank).
    const DP32_SARADC_IRQ: u32 = 4;
    const NVIC_BASE: usize = 0xe000_e100;
    const NVIC_ICER: usize = NVIC_BASE + 0x80;

    pub(crate) trait WordIo {
        fn read_word(&mut self, address: usize) -> u32;
        fn write_word(&mut self, address: usize, value: u32);
        fn sync_irq_disable(&mut self);
    }

    struct MappedIo<'a, I>(&'a mut I);

    impl<I: WordIo> RegisterIo for MappedIo<'_, I> {
        fn read(&mut self, register: Register) -> u32 {
            self.0.read_word(address(register))
        }

        fn write(&mut self, register: Register, value: u32) {
            self.0.write_word(address(register), value);
        }

        fn set_irq(&mut self, interrupt: Interrupt, action: IrqAction) {
            let mask = match interrupt {
                Interrupt::Saradc => 1 << DP32_SARADC_IRQ,
            };
            match action {
                IrqAction::Disable => {
                    self.0.write_word(NVIC_ICER, mask);
                    self.0.sync_irq_disable();
                }
            }
        }
    }

    pub(crate) fn apply_with_words(io: &mut impl WordIo, exttrig_sel: u16) {
        apply_with_io(&mut MappedIo(io), exttrig_sel);
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    mod target {
        use super::{WordIo, apply_with_words};

        struct VolatileIo;

        impl WordIo for VolatileIo {
            fn read_word(&mut self, address: usize) -> u32 {
                // SAFETY: The mapper supplies aligned word addresses; the entry
                // point's caller guarantees the mapping and access.
                unsafe { core::ptr::read_volatile(address as *const u32) }
            }

            fn write_word(&mut self, address: usize, value: u32) {
                // SAFETY: The same address and access contract as read_word.
                unsafe { core::ptr::write_volatile(address as *mut u32, value) }
            }

            fn sync_irq_disable(&mut self) {
                // SAFETY: Cortex-M0 supports these CMSIS barriers, which do not
                // touch the stack or clobber registers or flags.
                unsafe { core::arch::asm!("dsb", "isb", options(nostack, preserves_flags)) }
            }
        }

        /// Apply the pinned sequence to DP32G030 SYSCON, SARADC and Cortex-M0 NVIC.
        ///
        /// # Safety
        ///
        /// The caller must guarantee the register map is present and accessible
        /// in the current privilege/clock state: SYSCON and NVIC at entry, and
        /// SARADC after this sequence sets its device clock gate. The selected
        /// sample clock's source must be operational.
        ///
        /// Serialize the entire call against every other writer to the touched
        /// SYSCON and SARADC registers, including writers of unrelated fields
        /// (e.g. other peripheral clock gates), and other changes to the SARADC
        /// NVIC line. Exclusive ownership of the ADC pins is not sufficient.
        ///
        /// This call disables the SARADC NVIC line before peripheral access
        /// and writes zero to the interrupt-enable register. The caller must
        /// serialize other changes to the line throughout setup. This call
        /// does not clear NVIC pending state or install a handler.
        ///
        /// `exttrig_sel` must be explicitly chosen by the caller for the board;
        /// the pinned board initializer does not supply a value.
        pub unsafe fn apply_to_hardware(exttrig_sel: u16) {
            apply_with_words(&mut VolatileIo, exttrig_sel);
        }
    }

    #[cfg(all(target_arch = "arm", target_os = "none"))]
    pub use target::apply_to_hardware;
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use physical::apply_to_hardware;
