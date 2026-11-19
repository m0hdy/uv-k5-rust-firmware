#[path = "../src/gpio_startup_adapter.rs"]
mod gpio_startup_adapter;

use gpio_startup_adapter::{Features, GpioPlan, RegisterAddress, RegisterIo, apply_with_io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(RegisterAddress, u32),
    Write(RegisterAddress, u32),
}

struct FakeRegisterIo {
    values: [u32; 6],
    events: Vec<Event>,
}

impl FakeRegisterIo {
    fn new(values: [u32; 6]) -> Self {
        Self {
            values,
            events: Vec::new(),
        }
    }

    fn index(register: RegisterAddress) -> usize {
        match register {
            RegisterAddress::AData => 0,
            RegisterAddress::ADirection => 1,
            RegisterAddress::BData => 2,
            RegisterAddress::BDirection => 3,
            RegisterAddress::CData => 4,
            RegisterAddress::CDirection => 5,
        }
    }
}

impl RegisterIo for FakeRegisterIo {
    fn read(&mut self, register: RegisterAddress) -> u32 {
        let value = self.values[Self::index(register)];
        self.events.push(Event::Read(register, value));
        value
    }

    fn write(&mut self, register: RegisterAddress, value: u32) {
        self.events.push(Event::Write(register, value));
        self.values[Self::index(register)] = value;
    }
}

#[test]
fn address_map_contains_only_the_six_literal_dp32g030_gpio_registers() {
    assert_eq!(
        [
            RegisterAddress::AData.address(),
            RegisterAddress::ADirection.address(),
            RegisterAddress::BData.address(),
            RegisterAddress::BDirection.address(),
            RegisterAddress::CData.address(),
            RegisterAddress::CDirection.address(),
        ],
        [
            0x4006_0000,
            0x4006_0004,
            0x4006_0800,
            0x4006_0804,
            0x4006_1000,
            0x4006_1004,
        ]
    );
}

#[test]
fn each_plan_update_reads_then_writes_once_in_order_using_the_latest_value() {
    let plan = GpioPlan::for_features(Features {
        enable_fmradio: true,
    });
    let mut io = FakeRegisterIo::new([
        0xa5a5_5a5a,
        0x1357_9bdf,
        0xdead_beef,
        0x89ab_cdef,
        0xfedc_ba98,
        0x0123_4567,
    ]);

    apply_with_io(&mut io, &plan);

    assert_eq!(
        io.events,
        vec![
            Event::Read(RegisterAddress::ADirection, 0x1357_9bdf),
            Event::Write(RegisterAddress::ADirection, 0x1357_bfdf),
            Event::Read(RegisterAddress::ADirection, 0x1357_bfdf),
            Event::Write(RegisterAddress::ADirection, 0x1357_bf87),
            Event::Read(RegisterAddress::BDirection, 0x89ab_cdef),
            Event::Write(RegisterAddress::BDirection, 0x89ab_cfef),
            Event::Read(RegisterAddress::CDirection, 0x0123_4567),
            Event::Write(RegisterAddress::CDirection, 0x0123_457f),
            Event::Read(RegisterAddress::CDirection, 0x0123_457f),
            Event::Write(RegisterAddress::CDirection, 0x0123_455f),
            Event::Read(RegisterAddress::BData, 0xdead_beef),
            Event::Write(RegisterAddress::BData, 0xdead_beef),
        ]
    );
}

#[test]
fn adversarial_snapshots_preserve_every_untouched_bit() {
    let initial = [
        u32::MAX,
        0xa5a5_5a5a,
        0x1357_1bdf,
        0xdead_beef,
        0x89ab_cdef,
        0xfedc_ba98,
    ];
    let mut io = FakeRegisterIo::new(initial);

    apply_with_io(
        &mut io,
        &GpioPlan::for_features(Features {
            enable_fmradio: true,
        }),
    );

    let touched_masks = [0, 0x3c78, 0x8000, 0x8a00, 0, 0x003f];
    for ((before, after), touched) in initial.into_iter().zip(io.values).zip(touched_masks) {
        assert_eq!((before ^ after) & !touched, 0);
    }
    assert_eq!(io.values[2], 0x1357_9bdf);
}

#[test]
fn fm_feature_changes_only_the_optional_data_update() {
    let mut enabled = FakeRegisterIo::new([0x1111_1111; 6]);
    let mut disabled = FakeRegisterIo::new([0x1111_1111; 6]);

    apply_with_io(
        &mut enabled,
        &GpioPlan::for_features(Features {
            enable_fmradio: true,
        }),
    );
    apply_with_io(
        &mut disabled,
        &GpioPlan::for_features(Features {
            enable_fmradio: false,
        }),
    );

    assert_eq!(&enabled.events[..disabled.events.len()], disabled.events);
    assert_eq!(
        &enabled.events[disabled.events.len()..],
        &[
            Event::Read(RegisterAddress::BData, 0x1111_1111),
            Event::Write(RegisterAddress::BData, 0x1111_9111),
        ]
    );
    for index in [0, 1, 3, 4, 5] {
        assert_eq!(enabled.values[index], disabled.values[index]);
    }
    assert_eq!(enabled.values[2], disabled.values[2] | 0x8000);
}
