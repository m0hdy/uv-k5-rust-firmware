#[path = "../src/board_sequence.rs"]
mod board_sequence;

use board_sequence::{Backend, Call, Defines, run};

struct FakeBackend {
    calls: [Call; 9],
    len: usize,
}

impl FakeBackend {
    fn new() -> Self {
        Self {
            calls: [Call::DeviceClockGates; 9],
            len: 0,
        }
    }

    fn trace(&self) -> &[Call] {
        &self.calls[..self.len]
    }
}

impl Backend for FakeBackend {
    fn call(&mut self, call: Call) {
        self.calls[self.len] = call;
        self.len += 1;
    }
}

const BASE: [Call; 7] = [
    Call::DeviceClockGates,
    Call::SystickInit,
    Call::PortconInit,
    Call::GpioInit,
    Call::BacklightInitHardware,
    Call::AdcInit,
    Call::St7565Init,
];

#[test]
fn base_trace_has_only_default_calls_without_overlay_clock_or_flash_setup() {
    let mut backend = FakeBackend::new();
    run(
        Defines {
            enable_fmradio: false,
            enable_uart: false,
            enabled_aircopy: false,
        },
        &mut backend,
    );

    assert_eq!(backend.trace(), BASE);
}

#[test]
fn every_optional_macro_combination_preserves_order_and_initializes_crc_once() {
    let cases: [(bool, bool, bool, &[Call]); 8] = [
        (false, false, false, &[]),
        (false, false, true, &[Call::CrcInit]),
        (false, true, false, &[Call::CrcInit]),
        (false, true, true, &[Call::CrcInit]),
        (true, false, false, &[Call::Bk1080Init0]),
        (true, false, true, &[Call::Bk1080Init0, Call::CrcInit]),
        (true, true, false, &[Call::Bk1080Init0, Call::CrcInit]),
        (true, true, true, &[Call::Bk1080Init0, Call::CrcInit]),
    ];

    for (enable_fmradio, enable_uart, enabled_aircopy, tail) in cases {
        let defines = Defines {
            enable_fmradio,
            enable_uart,
            enabled_aircopy,
        };
        let mut backend = FakeBackend::new();
        run(defines, &mut backend);

        assert_eq!(&backend.trace()[..BASE.len()], BASE, "{defines:?}");
        assert_eq!(&backend.trace()[BASE.len()..], tail, "{defines:?}");
    }
}

#[test]
fn pinned_makefile_defaults_include_fm_then_crc() {
    let mut backend = FakeBackend::new();
    run(
        Defines {
            enable_fmradio: true,
            enable_uart: true,
            enabled_aircopy: false,
        },
        &mut backend,
    );

    assert_eq!(
        backend.trace(),
        [
            Call::DeviceClockGates,
            Call::SystickInit,
            Call::PortconInit,
            Call::GpioInit,
            Call::BacklightInitHardware,
            Call::AdcInit,
            Call::St7565Init,
            Call::Bk1080Init0,
            Call::CrcInit,
        ]
    );
}
