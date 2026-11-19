#[path = "../src/board_startup_adapters.rs"]
mod board_startup_adapters;

use board_startup_adapters::backlight_startup_executor::{Register, RegisterIo, apply_with_io};
use board_startup_adapters::{Call, Defines, StartupError, StartupIo, run};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    DeviceClockGates,
    Systick(u32),
    Portcon(bool),
    Gpio(bool),
    BacklightInit,
    BacklightRead(Register),
    BacklightWrite(Register, u32),
    Placeholder(Call),
}

struct FakeIo {
    events: Vec<Event>,
    fail_systick: bool,
    backlight_registers: [u32; 5],
}

impl Default for FakeIo {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            fail_systick: false,
            backlight_registers: [0xa123_4567, 0xdead_beef, 0x89ab_cdef, 0, 0],
        }
    }
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

    fn backlight_init_hardware(&mut self) {
        self.events.push(Event::BacklightInit);
        apply_with_io(self);
    }

    fn placeholder(&mut self, call: Call) {
        self.events.push(Event::Placeholder(call));
    }
}

impl RegisterIo for FakeIo {
    fn read(&mut self, register: Register) -> u32 {
        let value = self.backlight_registers[register_index(register)];
        self.events.push(Event::BacklightRead(register));
        value
    }

    fn write(&mut self, register: Register, value: u32) {
        self.backlight_registers[register_index(register)] = value;
        self.events.push(Event::BacklightWrite(register, value));
    }
}

fn register_index(register: Register) -> usize {
    match register {
        Register::PwmPlus0ClockSource => 0,
        Register::PwmPlus0Period => 1,
        Register::PortBSel0 => 2,
        Register::PwmPlus0Generator => 3,
        Register::PwmPlus0Config => 4,
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
            Event::BacklightInit,
            Event::BacklightRead(Register::PwmPlus0ClockSource),
            Event::BacklightWrite(Register::PwmPlus0ClockSource, 0xa12f_4567),
            Event::BacklightWrite(Register::PwmPlus0Period, 1023),
            Event::BacklightRead(Register::PortBSel0),
            Event::BacklightWrite(Register::PortBSel0, 0x80ab_cdef),
            Event::BacklightRead(Register::PortBSel0),
            Event::BacklightWrite(Register::PortBSel0, 0x81ab_cdef),
            Event::BacklightWrite(Register::PwmPlus0Generator, 0x0101_0000),
            Event::BacklightWrite(Register::PwmPlus0Config, 5),
            Event::Placeholder(Call::AdcInit),
            Event::Placeholder(Call::St7565Init),
            Event::Placeholder(Call::Bk1080Init0),
            Event::Placeholder(Call::CrcInit),
        ]
    );
    assert_eq!(io.backlight_registers[0], 0xa12f_4567);
    assert_eq!(io.backlight_registers[2], 0x81ab_cdef);
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
fn invalid_systick_setup_stops_before_portcon_gpio_backlight_and_placeholders() {
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
