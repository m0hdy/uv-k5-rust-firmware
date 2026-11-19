#[path = "../src/st7565_startup.rs"]
mod st7565_startup;

use st7565_startup::{ResetLevel, SpiEffect, StartupStep, startup_steps};
use uv_k5_rust_firmware::st7565_display::{CommandStep, DisplayBlitPlan, LCD_WIDTH};

#[test]
fn startup_matches_pinned_chronological_trace() {
    use ResetLevel::{High, Low};
    use SpiEffect::{DrainTransmitFifo, InitializeSpi0, SetMasterSelect, WriteCommand};
    use StartupStep::{DelayMs, FillScreen, Reset, Spi};

    let trace: Vec<_> = startup_steps().collect();
    assert_eq!(
        trace,
        [
            Spi(InitializeSpi0),
            Reset(High),
            DelayMs(1),
            Reset(Low),
            DelayMs(20),
            Reset(High),
            DelayMs(120),
            Spi(SetMasterSelect(false)),
            Spi(WriteCommand(0xe2)),
            DelayMs(120),
            Spi(WriteCommand(0xa2)),
            Spi(WriteCommand(0xc0)),
            Spi(WriteCommand(0xa1)),
            Spi(WriteCommand(0xa6)),
            Spi(WriteCommand(0xa4)),
            Spi(WriteCommand(0x24)),
            Spi(WriteCommand(0x81)),
            Spi(WriteCommand(0x1f)),
            Spi(WriteCommand(0x2b)),
            DelayMs(1),
            Spi(WriteCommand(0x2e)),
            DelayMs(1),
            Spi(WriteCommand(0x2f)),
            Spi(WriteCommand(0x2f)),
            Spi(WriteCommand(0x2f)),
            Spi(WriteCommand(0x2f)),
            DelayMs(40),
            Spi(WriteCommand(0x40)),
            Spi(WriteCommand(0xaf)),
            Spi(DrainTransmitFifo),
            Spi(SetMasterSelect(true)),
            FillScreen(0),
        ]
    );
}

#[test]
fn zero_fill_handoff_can_be_planned_as_eight_zero_lines() {
    assert_eq!(startup_steps().last(), Some(StartupStep::FillScreen(0)));
    let zero_line = [0; LCD_WIDTH];
    for page in 0..8 {
        let plan = DisplayBlitPlan::draw_line(page, 0, &zero_line).unwrap();
        let steps: Vec<_> = plan.steps().collect();
        assert_eq!(
            steps,
            [
                CommandStep::Command(0xb0 + page as u8),
                CommandStep::Command(0x10),
                CommandStep::Command(0x04),
                CommandStep::Data(&zero_line),
            ]
        );
    }
}
