pub use uv_k5_rust_firmware::{eeprom_read_transport, settings_boot_composition};

#[path = "../src/settings_channel_acquisition.rs"]
mod settings_channel_acquisition;

use settings_channel_acquisition::{
    ChannelSelectionBootDefaults, SettingsChannelAcquisitionError, acquire_with_transport,
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::rc::Rc;
use uv_k5_rust_firmware::eeprom_i2c_bridge::I2cBitbangTransport;
use uv_k5_rust_firmware::eeprom_read_transport::{Acknowledgement, I2cTransport, ReadError};
use uv_k5_rust_firmware::i2c_bitbang_flow::Level;
use uv_k5_rust_firmware::i2c_bitbang_flow::target_gpio::{
    Delay1us, GpioI2cIo, Register, RegisterIo, SCL_MASK, SDA_MASK,
};
use uv_k5_rust_firmware::settings_boot_composition::{
    BacklightMinStatus, Features, SettingsBootBlock, SettingsBootReadError, compose_with_transport,
};

const FEATURES: Features = Features {
    noaa_enabled: true,
    vox_enabled: true,
    blmin_tmp_off_enabled: true,
};

const CORE_DATA: [u8; 8] = [73, 8, 10, 1, 1, 1, 9, 4];
const DISPLAY_DATA: [u8; 8] = [0x24, 3, 1, 4, 2, 5, 1, 1];
const CHANNEL_DATA: [u8; 8] = [37, 173, 203, 91, 4, 204, 211, 216];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BusEvent {
    Start,
    Write(u8),
    RepeatedStart,
    Read(usize),
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportError {
    Transfer(usize),
    Cleanup,
}

struct MockTransport {
    events: Vec<BusEvent>,
    responses: [[u8; 8]; 4],
    reads: usize,
    fail_at: Option<usize>,
    nack_at: Option<usize>,
    transfer_failed: bool,
    fail_cleanup: bool,
}

impl MockTransport {
    fn new(responses: [[u8; 8]; 4]) -> Self {
        Self {
            events: Vec::new(),
            responses,
            reads: 0,
            fail_at: None,
            nack_at: None,
            transfer_failed: false,
            fail_cleanup: false,
        }
    }

    fn record(&mut self, event: BusEvent) -> Result<usize, TransportError> {
        let index = self.events.len();
        self.events.push(event);
        if self.fail_at == Some(index) {
            self.transfer_failed = true;
            Err(TransportError::Transfer(index))
        } else if event == BusEvent::Stop && self.transfer_failed && self.fail_cleanup {
            Err(TransportError::Cleanup)
        } else {
            Ok(index)
        }
    }
}

impl I2cTransport for MockTransport {
    type Error = TransportError;

    fn start(&mut self) -> Result<(), Self::Error> {
        self.record(BusEvent::Start).map(|_| ())
    }

    fn repeated_start(&mut self) -> Result<(), Self::Error> {
        self.record(BusEvent::RepeatedStart).map(|_| ())
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, Self::Error> {
        let index = self.record(BusEvent::Write(byte))?;
        Ok(if self.nack_at == Some(index) {
            Acknowledgement::Nack
        } else {
            Acknowledgement::Ack
        })
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), Self::Error> {
        let response = self.responses[self.reads];
        self.reads += 1;
        if let Err(error) = self.record(BusEvent::Read(output.len())) {
            if let Some(first) = output.first_mut() {
                *first = response[0];
            }
            return Err(error);
        }
        output.copy_from_slice(&response);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.record(BusEvent::Stop).map(|_| ())
    }
}

const fn read_trace(address: u8) -> [BusEvent; 8] {
    [
        BusEvent::Start,
        BusEvent::Write(0xa0),
        BusEvent::Write(0x0e),
        BusEvent::Write(address),
        BusEvent::RepeatedStart,
        BusEvent::Write(0xa1),
        BusEvent::Read(8),
        BusEvent::Stop,
    ]
}

const ACQUISITION_TRACE: [BusEvent; 32] = {
    let core = read_trace(0x70);
    let display = read_trace(0x78);
    let welcome = read_trace(0x90);
    let channel = read_trace(0x80);
    let mut trace = [BusEvent::Stop; 32];
    let mut index = 0;
    while index < 8 {
        trace[index] = core[index];
        trace[index + 8] = display[index];
        trace[index + 16] = welcome[index];
        trace[index + 24] = channel[index];
        index += 1;
    }
    trace
};

fn responses(channel: [u8; 8]) -> [[u8; 8]; 4] {
    [CORE_DATA, DISPLAY_DATA, [0; 8], channel]
}

fn expected_channel(
    screen_channel: [u8; 2],
    mr_channel: [u8; 2],
    freq_channel: [u8; 2],
    noaa_channel: Option<[u8; 2]>,
) -> ChannelSelectionBootDefaults {
    ChannelSelectionBootDefaults {
        screen_channel,
        mr_channel,
        freq_channel,
        noaa_channel,
    }
}

fn expected_error(
    event_index: usize,
    error: ReadError<TransportError>,
) -> SettingsChannelAcquisitionError<TransportError> {
    match event_index {
        0..=7 => SettingsChannelAcquisitionError::SettingsBoot(SettingsBootReadError {
            block: SettingsBootBlock::Core,
            error,
        }),
        8..=15 => SettingsChannelAcquisitionError::SettingsBoot(SettingsBootReadError {
            block: SettingsBootBlock::Display,
            error,
        }),
        16..=23 => SettingsChannelAcquisitionError::SettingsBoot(SettingsBootReadError {
            block: SettingsBootBlock::Welcome,
            error,
        }),
        _ => SettingsChannelAcquisitionError::ChannelSelection(error),
    }
}

#[test]
fn acquires_four_ordered_blocks_and_decodes_the_channel_selection() {
    let mut bus = MockTransport::new(responses(CHANNEL_DATA));

    let result = acquire_with_transport(&mut bus, FEATURES).unwrap();
    let mut baseline_bus = MockTransport::new(responses(CHANNEL_DATA));
    let baseline = compose_with_transport(&mut baseline_bus, FEATURES).unwrap();

    assert_eq!(
        result.channel_selection,
        expected_channel([37, 91], [173, 4], [203, 204], Some([211, 216]))
    );
    assert_eq!(result.settings, baseline);
    assert_eq!(baseline_bus.events.as_slice(), &ACQUISITION_TRACE[..24]);
    assert_eq!(bus.events, ACQUISITION_TRACE);
    assert_eq!(bus.reads, 4);
}

#[test]
fn decoder_matches_valid_zero_erased_and_channel_boundary_bytes() {
    let cases = [
        (
            CHANNEL_DATA,
            expected_channel([37, 91], [173, 4], [203, 204], Some([211, 216])),
        ),
        (
            [0; 8],
            expected_channel([0, 0], [0, 0], [205, 205], Some([207, 207])),
        ),
        (
            [0xff; 8],
            expected_channel([205, 205], [0, 0], [205, 205], Some([207, 207])),
        ),
        (
            [216, 199, 200, 216, 199, 206, 207, 216],
            expected_channel([216, 216], [199, 199], [200, 206], Some([207, 216])),
        ),
    ];

    for (data, expected) in cases {
        let mut bus = MockTransport::new(responses(data));
        let result = acquire_with_transport(&mut bus, FEATURES).unwrap();

        assert_eq!(result.channel_selection, expected);
        assert_eq!(bus.events, ACQUISITION_TRACE);
    }
}

#[test]
fn feature_flags_keep_the_three_settings_reads_and_noaa_selection_independent() {
    for noaa_enabled in [false, true] {
        for vox_enabled in [false, true] {
            for blmin_tmp_off_enabled in [false, true] {
                let mut bus = MockTransport::new(responses(CHANNEL_DATA));
                let features = Features {
                    noaa_enabled,
                    vox_enabled,
                    blmin_tmp_off_enabled,
                };
                let result = acquire_with_transport(&mut bus, features).unwrap();
                let mut baseline_bus = MockTransport::new(responses(CHANNEL_DATA));
                let baseline = compose_with_transport(&mut baseline_bus, features).unwrap();

                assert_eq!(result.settings, baseline);
                assert_eq!(
                    result.channel_selection.noaa_channel,
                    noaa_enabled.then_some([211, 216])
                );
                assert_eq!(
                    result.settings.core.noaa_auto_scan,
                    noaa_enabled.then_some(true)
                );
                assert_eq!(result.settings.core.vox_switch, vox_enabled.then_some(true));
                assert_eq!(result.settings.core.vox_level, vox_enabled.then_some(9));
                assert_eq!(
                    result.settings.display.backlight_min_status,
                    blmin_tmp_off_enabled.then_some(BacklightMinStatus::On)
                );
                assert_eq!(bus.events, ACQUISITION_TRACE);
                assert_eq!(baseline_bus.events.as_slice(), &ACQUISITION_TRACE[..24]);
            }
        }
    }
}

#[test]
fn every_transport_failure_stops_at_its_event_and_keeps_its_original_identity() {
    for event_index in 0..ACQUISITION_TRACE.len() {
        let mut bus = MockTransport::new(responses(CHANNEL_DATA));
        bus.fail_at = Some(event_index);

        assert_eq!(
            acquire_with_transport(&mut bus, FEATURES),
            Err(expected_error(
                event_index,
                ReadError::Transport(TransportError::Transfer(event_index)),
            )),
            "failure at event {event_index}"
        );

        let mut expected_events = ACQUISITION_TRACE[..=event_index].to_vec();
        if !matches!(
            ACQUISITION_TRACE[event_index],
            BusEvent::Start | BusEvent::Stop
        ) {
            expected_events.push(BusEvent::Stop);
        }
        assert_eq!(
            bus.events, expected_events,
            "failure at event {event_index}"
        );
        assert_eq!(
            bus.reads,
            usize::from(event_index >= 6)
                + usize::from(event_index >= 14)
                + usize::from(event_index >= 22)
                + usize::from(event_index >= 30)
        );
    }
}

#[test]
fn a_cleanup_stop_failure_never_replaces_the_original_error() {
    for event_index in [2, 10, 18, 26] {
        let mut bus = MockTransport::new(responses(CHANNEL_DATA));
        bus.fail_at = Some(event_index);
        bus.fail_cleanup = true;

        assert_eq!(
            acquire_with_transport(&mut bus, FEATURES),
            Err(expected_error(
                event_index,
                ReadError::Transport(TransportError::Transfer(event_index)),
            ))
        );
        assert_eq!(
            bus.events,
            [
                ACQUISITION_TRACE[..=event_index].to_vec(),
                vec![BusEvent::Stop],
            ]
            .concat()
        );
    }
}

#[test]
fn every_write_nack_cleans_up_and_prevents_later_blocks() {
    for (event_index, event) in ACQUISITION_TRACE.iter().enumerate() {
        if !matches!(event, BusEvent::Write(_)) {
            continue;
        }

        let mut bus = MockTransport::new(responses(CHANNEL_DATA));
        bus.nack_at = Some(event_index);

        assert_eq!(
            acquire_with_transport(&mut bus, FEATURES),
            Err(expected_error(event_index, ReadError::NotAcknowledged,))
        );
        assert_eq!(
            bus.events,
            [
                ACQUISITION_TRACE[..=event_index].to_vec(),
                vec![BusEvent::Stop],
            ]
            .concat()
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GpioBusEdge {
    Start,
    Stop,
}

struct RegisterState {
    values: [u32; 4],
    samples: VecDeque<Level>,
    last_input: Level,
    pending_sample: bool,
    writes: Vec<(Register, u32)>,
    bus_edges: Vec<GpioBusEdge>,
}

impl RegisterState {
    fn new(samples: impl IntoIterator<Item = Level>) -> Self {
        Self {
            values: [
                0x1234_5678 | SCL_MASK | SDA_MASK,
                0x89ab_cdef | SDA_MASK,
                0x1357_9bdf & !SDA_MASK,
                0x2468_ace0 & !SDA_MASK,
            ],
            samples: samples.into_iter().collect(),
            last_input: Level::High,
            pending_sample: false,
            writes: Vec::new(),
            bus_edges: Vec::new(),
        }
    }

    fn index(register: Register) -> usize {
        match register {
            Register::GpioAData => 0,
            Register::GpioADirection => 1,
            Register::PortAInputEnable => 2,
            Register::PortAOpenDrain => 3,
        }
    }

    fn read(&mut self, register: Register) -> u32 {
        if self.pending_sample {
            if let Some(level) = self.samples.pop_front() {
                self.last_input = level;
            }
            self.pending_sample = false;
        }
        let index = Self::index(register);
        let mut value = self.values[index];
        if register == Register::GpioAData && self.values[1] & SDA_MASK == 0 {
            let level = self.samples.front().copied().unwrap_or(self.last_input);
            value = match level {
                Level::Low => value & !SDA_MASK,
                Level::High => value | SDA_MASK,
            };
            self.pending_sample = true;
        }
        value
    }

    fn write(&mut self, register: Register, value: u32) {
        self.pending_sample = false;
        let index = Self::index(register);
        if register == Register::GpioAData {
            let previous = self.values[index];
            let was_scl_high = previous & SCL_MASK != 0;
            let is_scl_high = value & SCL_MASK != 0;
            let was_sda_high = previous & SDA_MASK != 0;
            let is_sda_high = value & SDA_MASK != 0;
            if was_scl_high && is_scl_high && was_sda_high && !is_sda_high {
                self.bus_edges.push(GpioBusEdge::Start);
            } else if was_scl_high && is_scl_high && !was_sda_high && is_sda_high {
                self.bus_edges.push(GpioBusEdge::Stop);
            }
        }
        self.values[index] = value;
        self.writes.push((register, value));
    }
}

type SharedRegisterState = Rc<RefCell<RegisterState>>;

struct MockRegisters(SharedRegisterState);

impl MockRegisters {
    fn new(samples: impl IntoIterator<Item = Level>) -> (Self, SharedRegisterState) {
        let state = Rc::new(RefCell::new(RegisterState::new(samples)));
        (Self(state.clone()), state)
    }
}

impl RegisterIo for MockRegisters {
    fn read(&mut self, register: Register) -> u32 {
        self.0.borrow_mut().read(register)
    }

    fn write(&mut self, register: Register, value: u32) {
        self.0.borrow_mut().write(register, value);
    }
}

fn response_samples(responses: &[[u8; 8]]) -> Vec<Level> {
    let mut samples = Vec::new();
    for response in responses {
        samples.extend([Level::Low; 4]);
        for byte in response {
            for bit in (0..8).rev() {
                samples.push(if byte & (1 << bit) == 0 {
                    Level::Low
                } else {
                    Level::High
                });
            }
        }
    }
    samples
}

fn nack_samples(block: usize, write: usize) -> Vec<Level> {
    let mut samples = response_samples(&responses(CHANNEL_DATA)[..block]);
    samples.extend(std::iter::repeat_n(Level::Low, write));
    samples.extend([Level::High; 255]);
    samples
}

fn expected_gpio_nack(block: usize) -> SettingsChannelAcquisitionError<Infallible> {
    let error = ReadError::NotAcknowledged;
    match block {
        0 => SettingsChannelAcquisitionError::SettingsBoot(SettingsBootReadError {
            block: SettingsBootBlock::Core,
            error,
        }),
        1 => SettingsChannelAcquisitionError::SettingsBoot(SettingsBootReadError {
            block: SettingsBootBlock::Display,
            error,
        }),
        2 => SettingsChannelAcquisitionError::SettingsBoot(SettingsBootReadError {
            block: SettingsBootBlock::Welcome,
            error,
        }),
        _ => SettingsChannelAcquisitionError::ChannelSelection(error),
    }
}

fn expected_gpio_edges(block: usize, write: usize) -> Vec<GpioBusEdge> {
    let mut edges = Vec::new();
    for _ in 0..block {
        edges.extend([GpioBusEdge::Start, GpioBusEdge::Start, GpioBusEdge::Stop]);
    }
    edges.push(GpioBusEdge::Start);
    if write == 3 {
        edges.push(GpioBusEdge::Start);
    }
    edges.push(GpioBusEdge::Stop);
    edges
}

fn assert_keypad_mode(state: &RegisterState) {
    assert_eq!(state.values[0] & (SCL_MASK | SDA_MASK), SCL_MASK | SDA_MASK);
    assert_eq!(state.values[1] & SDA_MASK, SDA_MASK);
    assert_eq!(state.values[2] & SDA_MASK, 0);
    assert_eq!(state.values[3] & SDA_MASK, 0);
}

struct NoDelay;

impl Delay1us for NoDelay {
    fn delay_1us(&mut self) {}
}

#[test]
fn gpio_register_mock_runs_all_four_blocks_through_the_existing_bridge() {
    let data = responses(CHANNEL_DATA);
    let (registers, _) = MockRegisters::new(response_samples(&data));
    let mut io = GpioI2cIo::new(registers, NoDelay);
    let mut transport = I2cBitbangTransport::new(&mut io);

    let result = acquire_with_transport(&mut transport, FEATURES).unwrap();

    assert_eq!(
        result.channel_selection,
        expected_channel([37, 91], [173, 4], [203, 204], Some([211, 216]))
    );
}

#[test]
fn gpio_register_mock_cleans_up_each_write_nack_before_owner_release() {
    for block in 0..4 {
        for write in 0..4 {
            for handoff in [false, true] {
                let (registers, shared) = MockRegisters::new(nack_samples(block, write));
                let mut io = GpioI2cIo::new(registers, NoDelay);
                {
                    let mut transport = I2cBitbangTransport::new(&mut io);
                    assert_eq!(
                        acquire_with_transport(&mut transport, FEATURES),
                        Err(expected_gpio_nack(block)),
                        "NACK at block {block}, write {write}"
                    );
                }

                {
                    let state = shared.borrow();
                    assert_eq!(
                        state.bus_edges,
                        expected_gpio_edges(block, write),
                        "bus edges at block {block}, write {write}"
                    );
                    assert!(
                        state.samples.is_empty(),
                        "unread GPIO samples at block {block}, write {write}"
                    );
                    let data_writes = state
                        .writes
                        .iter()
                        .filter_map(|(register, value)| {
                            (*register == Register::GpioAData)
                                .then_some(value & (SCL_MASK | SDA_MASK))
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        data_writes.ends_with(&[0, 0, SCL_MASK, SCL_MASK | SDA_MASK]),
                        "missing cleanup STOP write trace at block {block}, write {write}"
                    );
                }

                if handoff {
                    io.handoff_to_keypad();
                } else {
                    drop(io);
                }
                assert_keypad_mode(&shared.borrow());
            }
        }
    }
}
