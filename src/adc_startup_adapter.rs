#![cfg_attr(not(test), no_std)]

//! Offline model of the pinned board SARADC startup register sequence.
//!
//! The sequence follows `BOARD_ADC_Init` in `board.c`, `ADC_Configure`,
//! `ADC_Enable`, and `ADC_SoftReset` in `driver/adc.c`, and the DP32G030
//! definitions in `bsp/dp32g030/{saradc,syscon}.h` at
//! [`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
//! It has no MMIO implementation and is not connected to firmware startup.

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
const SARADC_IE_CHX_EOC_MASK: u32 = 0xffff;
const SARADC_IE_FIFO_FULL_MASK: u32 = 1 << 16;
const SARADC_IE_FIFO_HFULL_MASK: u32 = 1 << 17;
const SARADC_IE_CONFIGURATION_MASK: u32 =
    SARADC_IE_CHX_EOC_MASK | SARADC_IE_FIFO_FULL_MASK | SARADC_IE_FIFO_HFULL_MASK;
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
    Enable,
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

    let interrupt_enable = io.read(Register::InterruptEnable);
    io.write(
        Register::InterruptEnable,
        interrupt_enable & !SARADC_IE_CONFIGURATION_MASK,
    );
    let interrupt_enable_readback = io.read(Register::InterruptEnable);
    io.set_irq(
        Interrupt::Saradc,
        if interrupt_enable_readback == 0 {
            IrqAction::Disable
        } else {
            IrqAction::Enable
        },
    );
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
