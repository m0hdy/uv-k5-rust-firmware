#[path = "../src/board_startup_adapters.rs"]
mod board_startup_adapters;

use board_startup_adapters::{Call, Defines, StartupError, StartupIo, run};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    DeviceClockGates,
    Systick(u32),
    Portcon(bool),
    Gpio(bool),
    Placeholder(Call),
}

#[derive(Default)]
struct FakeIo {
    events: Vec<Event>,
    fail_systick: bool,
}

impl StartupIo for FakeIo {
    fn device_clock_gates(&mut self) {
        self.events.push(Event::DeviceClockGates);
    }

    fn systick(&mut self, period_ticks: u32) -> Result<(), StartupError> {
        self.events.push(Event::Systick(period_ticks));
        if self.fail_systick {
            Err(StartupError)
        } else {
            Ok(())
        }
    }

    fn portcon(&mut self, enable_swd: bool) {
        self.events.push(Event::Portcon(enable_swd));
    }

    fn gpio(&mut self, enable_fmradio: bool) {
        self.events.push(Event::Gpio(enable_fmradio));
    }

    fn placeholder(&mut self, call: Call) {
        self.events.push(Event::Placeholder(call));
    }
}

fn defines(enable_fmradio: bool, enable_uart: bool, enabled_aircopy: bool) -> Defines {
    Defines {
        enable_fmradio,
        enable_uart,
        enabled_aircopy,
    }
}

#[test]
fn sequencer_dispatches_adapters_in_pinned_order_and_keeps_other_calls_typed() {
    let mut io = FakeIo::default();
    assert_eq!(run(defines(true, true, false), true, &mut io), Ok(()));

    assert_eq!(
        io.events,
        [
            Event::DeviceClockGates,
            Event::Systick(480_000),
            Event::Portcon(true),
            Event::Gpio(true),
            Event::Placeholder(Call::BacklightInitHardware),
            Event::Placeholder(Call::AdcInit),
            Event::Placeholder(Call::St7565Init),
            Event::Placeholder(Call::Bk1080Init0),
            Event::Placeholder(Call::CrcInit),
        ]
    );
}

#[test]
fn swd_and_fm_features_are_forwarded_to_their_adapters() {
    for enable_swd in [false, true] {
        for enable_fmradio in [false, true] {
            let mut io = FakeIo::default();
            assert_eq!(
                run(defines(enable_fmradio, false, false), enable_swd, &mut io),
                Ok(())
            );
            assert!(io.events.contains(&Event::Portcon(enable_swd)));
            assert!(io.events.contains(&Event::Gpio(enable_fmradio)));
        }
    }
}

#[test]
fn invalid_systick_setup_stops_before_portcon_gpio_and_placeholders() {
    let mut io = FakeIo {
        fail_systick: true,
        ..FakeIo::default()
    };

    assert_eq!(
        run(defines(true, true, true), true, &mut io),
        Err(StartupError)
    );
    assert_eq!(
        io.events,
        [Event::DeviceClockGates, Event::Systick(480_000)]
    );
}
