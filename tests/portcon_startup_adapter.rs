#[path = "../src/portcon_startup_adapter.rs"]
mod portcon_startup_adapter;

use portcon_startup_adapter::{Features, RegisterAddress, RegisterIo, apply_with_io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Read(RegisterAddress, u32),
    Write(RegisterAddress, u32),
}

struct FakeRegisterIo {
    values: [u32; 17],
    events: Vec<Event>,
}

impl FakeRegisterIo {
    fn new(values: [u32; 17]) -> Self {
        Self {
            values,
            events: Vec::new(),
        }
    }

    fn index(register: RegisterAddress) -> usize {
        match register {
            RegisterAddress::PortASel0 => 0,
            RegisterAddress::PortASel1 => 1,
            RegisterAddress::PortBSel0 => 2,
            RegisterAddress::PortBSel1 => 3,
            RegisterAddress::PortCSel0 => 4,
            RegisterAddress::AInputEnable => 5,
            RegisterAddress::BInputEnable => 6,
            RegisterAddress::CInputEnable => 7,
            RegisterAddress::APullUp => 8,
            RegisterAddress::BPullUp => 9,
            RegisterAddress::CPullUp => 10,
            RegisterAddress::APullDown => 11,
            RegisterAddress::BPullDown => 12,
            RegisterAddress::CPullDown => 13,
            RegisterAddress::AOpenDrain => 14,
            RegisterAddress::BOpenDrain => 15,
            RegisterAddress::COpenDrain => 16,
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

const INITIAL: [u32; 17] = [
    0xa5a5_5a5a,
    0x1357_9bdf,
    0xdead_beef,
    0x89ab_cdef,
    0xfedc_ba98,
    0xa5a5_5a5a,
    0xfedc_ba98,
    0x55aa_55aa,
    0x1357_9bdf,
    0x0123_4567,
    0x1234_5678,
    0xdead_beef,
    0x7654_3210,
    0xa5a5_a5a5,
    0x89ab_cdef,
    0x89ab_cdef,
    0xf0f0_f0f0,
];

type ExpectedOperation = (RegisterAddress, u32, u32);

fn expected_events(operations: [ExpectedOperation; 29]) -> Vec<Event> {
    operations
        .into_iter()
        .flat_map(|(register, read, write)| {
            [Event::Read(register, read), Event::Write(register, write)]
        })
        .collect()
}

fn assert_trace(enable_swd: bool, operations: [ExpectedOperation; 29]) -> FakeRegisterIo {
    let mut io = FakeRegisterIo::new(INITIAL);
    apply_with_io(&mut io, Features { enable_swd });
    assert_eq!(io.events, expected_events(operations));
    io
}

#[test]
fn address_map_contains_only_the_pinned_portcon_registers() {
    assert_eq!(
        [
            RegisterAddress::PortASel0.address(),
            RegisterAddress::PortASel1.address(),
            RegisterAddress::PortBSel0.address(),
            RegisterAddress::PortBSel1.address(),
            RegisterAddress::PortCSel0.address(),
            RegisterAddress::AInputEnable.address(),
            RegisterAddress::BInputEnable.address(),
            RegisterAddress::CInputEnable.address(),
            RegisterAddress::APullUp.address(),
            RegisterAddress::BPullUp.address(),
            RegisterAddress::CPullUp.address(),
            RegisterAddress::APullDown.address(),
            RegisterAddress::BPullDown.address(),
            RegisterAddress::CPullDown.address(),
            RegisterAddress::AOpenDrain.address(),
            RegisterAddress::BOpenDrain.address(),
            RegisterAddress::COpenDrain.address(),
        ],
        [
            0x400b_0000,
            0x400b_0004,
            0x400b_0008,
            0x400b_000c,
            0x400b_0010,
            0x400b_0100,
            0x400b_0104,
            0x400b_0108,
            0x400b_0200,
            0x400b_0204,
            0x400b_0208,
            0x400b_0300,
            0x400b_0304,
            0x400b_0308,
            0x400b_0400,
            0x400b_0404,
            0x400b_0408,
        ]
    );
}

#[test]
fn ordered_trace_uses_fresh_reads_without_swd() {
    let io = assert_trace(
        false,
        [
            (RegisterAddress::PortASel0, 0xa5a5_5a5a, 0xa000_0a5a),
            (RegisterAddress::PortASel0, 0xa000_0a5a, 0xb000_0a5a),
            (RegisterAddress::PortASel1, 0x1357_9bdf, 0x1300_00df),
            (RegisterAddress::PortASel1, 0x1300_00df, 0x1700_00df),
            (RegisterAddress::PortBSel0, 0xdead_beef, 0x0ead_beef),
            (RegisterAddress::PortBSel0, 0x0ead_beef, 0x1ead_beef),
            (RegisterAddress::PortBSel1, 0x89ab_cdef, 0x00ab_0d0f),
            (RegisterAddress::PortBSel1, 0x00ab_0d0f, 0x00ab_0d0f),
            (RegisterAddress::PortCSel0, 0xfedc_ba98, 0xfe00_0000),
            (RegisterAddress::AInputEnable, 0xa5a5_5a5a, 0xa5a5_5b7a),
            (RegisterAddress::AInputEnable, 0xa5a5_5b7a, 0xa5a5_437a),
            (RegisterAddress::APullUp, 0x1357_9bdf, 0x1357_9bff),
            (RegisterAddress::APullUp, 0x1357_9bff, 0x1357_83ff),
            (RegisterAddress::APullDown, 0xdead_beef, 0xdead_8287),
            (RegisterAddress::AOpenDrain, 0x89ab_cdef, 0x89ab_cdff),
            (RegisterAddress::AOpenDrain, 0x89ab_cdff, 0x89ab_c1ff),
            (RegisterAddress::BInputEnable, 0xfedc_ba98, 0xfedc_fa98),
            (RegisterAddress::BInputEnable, 0xfedc_fa98, 0xfedc_7018),
            (RegisterAddress::BPullUp, 0x0123_4567, 0x0123_0527),
            (RegisterAddress::BPullDown, 0x7654_3210, 0x7654_3010),
            (RegisterAddress::BOpenDrain, 0x89ab_cdef, 0x89ab_45af),
            (RegisterAddress::BOpenDrain, 0x89ab_45af, 0x89ab_45af),
            (RegisterAddress::CInputEnable, 0x55aa_55aa, 0x55aa_55aa),
            (RegisterAddress::CInputEnable, 0x55aa_55aa, 0x55aa_55a0),
            (RegisterAddress::CPullUp, 0x1234_5678, 0x1234_5678),
            (RegisterAddress::CPullUp, 0x1234_5678, 0x1234_5660),
            (RegisterAddress::CPullDown, 0xa5a5_a5a5, 0xa5a5_a580),
            (RegisterAddress::COpenDrain, 0xf0f0_f0f0, 0xf0f0_f0e0),
            (RegisterAddress::COpenDrain, 0xf0f0_f0e0, 0xf0f0_f0e0),
        ],
    );
    assert_eq!(io.events.len(), 58);
}

#[test]
fn ordered_trace_uses_fresh_reads_with_swd() {
    assert_trace(
        true,
        [
            (RegisterAddress::PortASel0, 0xa5a5_5a5a, 0xa000_0a5a),
            (RegisterAddress::PortASel0, 0xa000_0a5a, 0xb000_0a5a),
            (RegisterAddress::PortASel1, 0x1357_9bdf, 0x1300_00df),
            (RegisterAddress::PortASel1, 0x1300_00df, 0x1700_00df),
            (RegisterAddress::PortBSel0, 0xdead_beef, 0x0ead_beef),
            (RegisterAddress::PortBSel0, 0x0ead_beef, 0x1ead_beef),
            (RegisterAddress::PortBSel1, 0x89ab_cdef, 0x00ab_0d0f),
            (RegisterAddress::PortBSel1, 0x00ab_0d0f, 0x01ab_1d0f),
            (RegisterAddress::PortCSel0, 0xfedc_ba98, 0xfe00_0000),
            (RegisterAddress::AInputEnable, 0xa5a5_5a5a, 0xa5a5_5b7a),
            (RegisterAddress::AInputEnable, 0xa5a5_5b7a, 0xa5a5_437a),
            (RegisterAddress::APullUp, 0x1357_9bdf, 0x1357_9bff),
            (RegisterAddress::APullUp, 0x1357_9bff, 0x1357_83ff),
            (RegisterAddress::APullDown, 0xdead_beef, 0xdead_8287),
            (RegisterAddress::AOpenDrain, 0x89ab_cdef, 0x89ab_cdff),
            (RegisterAddress::AOpenDrain, 0x89ab_cdff, 0x89ab_c1ff),
            (RegisterAddress::BInputEnable, 0xfedc_ba98, 0xfedc_fa98),
            (RegisterAddress::BInputEnable, 0xfedc_fa98, 0xfedc_7818),
            (RegisterAddress::BPullUp, 0x0123_4567, 0x0123_0527),
            (RegisterAddress::BPullDown, 0x7654_3210, 0x7654_3010),
            (RegisterAddress::BOpenDrain, 0x89ab_cdef, 0x89ab_45af),
            (RegisterAddress::BOpenDrain, 0x89ab_45af, 0x89ab_45af),
            (RegisterAddress::CInputEnable, 0x55aa_55aa, 0x55aa_55aa),
            (RegisterAddress::CInputEnable, 0x55aa_55aa, 0x55aa_55a0),
            (RegisterAddress::CPullUp, 0x1234_5678, 0x1234_5678),
            (RegisterAddress::CPullUp, 0x1234_5678, 0x1234_5660),
            (RegisterAddress::CPullDown, 0xa5a5_a5a5, 0xa5a5_a580),
            (RegisterAddress::COpenDrain, 0xf0f0_f0f0, 0xf0f0_f0e0),
            (RegisterAddress::COpenDrain, 0xf0f0_f0e0, 0xf0f0_f0e0),
        ],
    );
}

#[test]
fn unrelated_register_bits_are_preserved() {
    let mut io = FakeRegisterIo::new(INITIAL);
    apply_with_io(&mut io, Features { enable_swd: true });

    let touched_masks = [
        0x1fff_f000,
        0x05ff_ff51,
        0xf000_0000,
        0xff00_f1f1,
        0x00ff_ffff,
        0x0000_3d78,
        0x0000_cfc0,
        0x0000_003f,
        0x0000_3c78,
        0x0000_ca40,
        0x0000_003f,
        0x0000_3c78,
        0x0000_ca40,
        0x0000_003f,
        0x0000_3c78,
        0x0000_ca40,
        0x0000_003f,
    ];
    for ((before, after), touched) in INITIAL.into_iter().zip(io.values).zip(touched_masks) {
        assert_eq!((before ^ after) & !touched, 0);
    }
}
