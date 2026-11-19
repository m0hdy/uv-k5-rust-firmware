#![cfg_attr(not(test), no_std)]

//! Offline call ordering from pinned
//! [`main.c:61–77`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L61-L77)
//! and [`board.c:490–505`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/board.c#L490-L505).
//! Dispatches logical calls only, with no MMIO, timing, or hardware operations.
//! `BOARD_Init` is flattened into its calls; the separate overlay flash/clock
//! initialization path is intentionally not represented.

/// Presence of the C preprocessor macros, not their numeric values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Defines {
    /// `#ifdef ENABLE_FMRADIO`
    pub enable_fmradio: bool,
    /// `defined(ENABLE_UART)`
    pub enable_uart: bool,
    /// `defined(ENABLED_AIRCOPY)` (not `ENABLE_AIRCOPY`).
    pub enabled_aircopy: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Call {
    DeviceClockGates,
    SystickInit,
    PortconInit,
    GpioInit,
    BacklightInitHardware,
    AdcInit,
    St7565Init,
    Bk1080Init0,
    CrcInit,
}

/// A consumer of logical call events; no hardware backend is supplied here.
pub trait Backend {
    fn call(&mut self, call: Call);
}

/// Dispatch the default (non-overlay) bring-up sequence without allocation.
pub fn run(defines: Defines, backend: &mut impl Backend) {
    backend.call(Call::DeviceClockGates);
    backend.call(Call::SystickInit);
    backend.call(Call::PortconInit);
    backend.call(Call::GpioInit);
    backend.call(Call::BacklightInitHardware);
    backend.call(Call::AdcInit);
    backend.call(Call::St7565Init);

    if defines.enable_fmradio {
        backend.call(Call::Bk1080Init0);
    }
    if defines.enable_uart || defines.enabled_aircopy {
        backend.call(Call::CrcInit);
    }
}
