#[path = "../src/board_startup_adapters.rs"]
mod board_startup_adapters;
#[path = "../src/st7565_display.rs"]
#[allow(dead_code)]
mod st7565_display;
#[path = "../src/st7565_line_transport.rs"]
#[allow(dead_code)]
mod st7565_line_transport;
#[path = "../src/st7565_mmio_transport.rs"]
#[allow(dead_code)]
mod st7565_mmio_transport;
#[path = "../src/st7565_startup.rs"]
#[allow(dead_code)]
mod st7565_startup;
#[path = "../src/st7565_startup_executor.rs"]
#[allow(dead_code)]
mod st7565_startup_executor;

use board_startup_adapters::backlight_startup_executor::{Register, RegisterIo, apply_with_io};
use board_startup_adapters::crc_startup_adapter::{WordIo, apply_with_io as apply_crc};
use board_startup_adapters::{Call, Defines, StartupError, StartupIo, run};
use core::num::NonZeroU32;
use st7565_line_transport::LineError;
use st7565_startup_executor::target_spi0::DelayMs;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    DeviceClockGates,
    Systick(u32),
    Portcon(bool),
    Gpio(bool),
    BacklightInit,
    AdcInit(u16),
    St7565Init(NonZeroU32),
    CrcInit,
    CrcWrite(usize, u32),
    BacklightRead(Register),
    BacklightWrite(Register, u32),
    Placeholder(Call),
}

struct FakeIo {
    events: Vec<Event>,
    fail_systick: bool,
    fail_st7565: bool,
    backlight_registers: [u32; 5],
}

impl Default for FakeIo {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            fail_systick: false,
            fail_st7565: false,
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
            Err(StartupError::SysTick)
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

    fn adc_init(&mut self, exttrig_sel: u16) {
        self.events.push(Event::AdcInit(exttrig_sel));
    }

    fn st7565_init(
        &mut self,
        delay: &mut dyn DelayMs,
        fifo_poll_limit: NonZeroU32,
    ) -> Result<(), LineError> {
        self.events.push(Event::St7565Init(fifo_poll_limit));
        delay.delay_ms(37);
        if self.fail_st7565 {
            Err(LineError::FifoTimeout)
        } else {
            Ok(())
        }
    }

    fn placeholder(&mut self, call: Call) {
        self.events.push(Event::Placeholder(call));
    }

    fn crc_init(&mut self) {
        self.events.push(Event::CrcInit);
        apply_crc(self);
    }
}

impl WordIo for FakeIo {
    fn write_word(&mut self, address: usize, value: u32) {
        self.events.push(Event::CrcWrite(address, value));
    }
}

#[derive(Default)]
struct DelayProbe(Vec<u16>);

impl DelayMs for DelayProbe {
    fn delay_ms(&mut self, milliseconds: u16) {
        self.0.push(milliseconds);
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
    let mut delay = DelayProbe::default();
    assert_eq!(
        run(
            defines(true, true, false),
            true,
            0x5aa5,
            &mut delay,
            NonZeroU32::new(0x1234).unwrap(),
            &mut io
        ),
        Ok(())
    );

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
            Event::AdcInit(0x5aa5),
            Event::St7565Init(NonZeroU32::new(0x1234).unwrap()),
            Event::Placeholder(Call::Bk1080Init0),
            Event::CrcInit,
            Event::CrcWrite(0x4000_3000, 0x100),
            Event::CrcWrite(0x4000_3004, 0),
        ]
    );
    assert_eq!(delay.0, [37]);
    assert_eq!(io.backlight_registers[0], 0xa12f_4567);
    assert_eq!(io.backlight_registers[2], 0x81ab_cdef);
}

#[test]
fn swd_and_fm_features_are_forwarded_to_their_adapters() {
    for enable_swd in [false, true] {
        for enable_fmradio in [false, true] {
            let mut io = FakeIo::default();
            let mut delay = DelayProbe::default();
            assert_eq!(
                run(
                    defines(enable_fmradio, false, false),
                    enable_swd,
                    0x35c7,
                    &mut delay,
                    NonZeroU32::new(7).unwrap(),
                    &mut io
                ),
                Ok(())
            );
            assert!(io.events.contains(&Event::Portcon(enable_swd)));
            assert!(io.events.contains(&Event::Gpio(enable_fmradio)));
        }
    }
}

#[test]
fn invalid_systick_setup_stops_before_all_later_board_stages() {
    let mut io = FakeIo {
        fail_systick: true,
        ..FakeIo::default()
    };
    let mut delay = DelayProbe::default();

    assert_eq!(
        run(
            defines(true, true, true),
            true,
            0x35c7,
            &mut delay,
            NonZeroU32::new(7).unwrap(),
            &mut io
        ),
        Err(StartupError::SysTick)
    );
    assert_eq!(
        io.events,
        [Event::DeviceClockGates, Event::Systick(480_000)]
    );
}

#[test]
fn display_error_preserves_line_cause_and_stops_optional_stages() {
    let mut io = FakeIo {
        fail_st7565: true,
        ..FakeIo::default()
    };
    let mut delay = DelayProbe::default();

    assert_eq!(
        run(
            defines(true, true, true),
            false,
            0x5aa5,
            &mut delay,
            NonZeroU32::new(19).unwrap(),
            &mut io
        ),
        Err(StartupError::St7565(LineError::FifoTimeout))
    );
    assert_eq!(
        io.events.last(),
        Some(&Event::St7565Init(NonZeroU32::new(19).unwrap()))
    );
    assert_eq!(delay.0, [37]);
    assert!(!io.events.contains(&Event::Placeholder(Call::Bk1080Init0)));
    assert!(!io.events.contains(&Event::CrcInit));
}

#[test]
fn crc_runs_once_only_when_uart_or_aircopy_is_enabled() {
    for fm in [false, true] {
        for uart in [false, true] {
            for aircopy in [false, true] {
                let mut io = FakeIo::default();
                let mut delay = DelayProbe::default();
                assert_eq!(
                    run(
                        defines(fm, uart, aircopy),
                        false,
                        9,
                        &mut delay,
                        NonZeroU32::MIN,
                        &mut io
                    ),
                    Ok(())
                );
                let tail = io
                    .events
                    .iter()
                    .position(|event| matches!(event, Event::St7565Init(_)))
                    .unwrap();
                let mut expected = Vec::new();
                if fm {
                    expected.push(Event::Placeholder(Call::Bk1080Init0));
                }
                if uart || aircopy {
                    expected.extend([
                        Event::CrcInit,
                        Event::CrcWrite(0x4000_3000, 0x100),
                        Event::CrcWrite(0x4000_3004, 0),
                    ]);
                }
                assert_eq!(&io.events[tail + 1..], expected);
            }
        }
    }
}
