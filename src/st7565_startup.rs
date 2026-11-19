#![cfg_attr(not(test), no_std)]

//! Offline ST7565 startup effects, ending at the fill-screen handoff.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpiEffect {
    InitializeSpi0,
    SetMasterSelect(bool),
    WriteCommand(u8),
    DrainTransmitFifo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetLevel {
    High,
    Low,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupStep {
    Spi(SpiEffect),
    Reset(ResetLevel),
    DelayMs(u16),
    FillScreen(u8),
}

/// Returns the pinned startup sequence, without executing any hardware I/O.
///
/// Each `WriteCommand` represents the source's A0-low, FIFO-ready wait and
/// byte write. `FillScreen` is a call boundary, not a claim that data was sent.
pub fn startup_steps() -> impl Iterator<Item = StartupStep> {
    use ResetLevel::{High, Low};
    use SpiEffect::{DrainTransmitFifo, InitializeSpi0, SetMasterSelect, WriteCommand};
    use StartupStep::{DelayMs, FillScreen, Reset, Spi};

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
    .into_iter()
}
