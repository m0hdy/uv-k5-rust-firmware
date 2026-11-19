use uv_k5_rust_firmware::eeprom_read_transport::{
    Acknowledgement, I2cTransport, ReadError, execute_read, plan_read,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Start,
    Write(u8),
    RepeatedStart,
    Read(usize),
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BusError {
    Transfer(usize),
    Cleanup,
}

struct Mock {
    events: Vec<Event>,
    payload: Vec<u8>,
    fail_at: Option<usize>,
    nack_at: Option<usize>,
    fail_cleanup: bool,
}

impl Mock {
    fn new(payload: &[u8]) -> Self {
        Self {
            events: Vec::new(),
            payload: payload.to_vec(),
            fail_at: None,
            nack_at: None,
            fail_cleanup: false,
        }
    }

    fn record(&mut self, event: Event) -> Result<(), BusError> {
        let index = self.events.len();
        self.events.push(event);
        if self.fail_at == Some(index) {
            Err(BusError::Transfer(index))
        } else if event == Event::Stop && self.fail_cleanup {
            Err(BusError::Cleanup)
        } else {
            Ok(())
        }
    }
}

impl I2cTransport for Mock {
    type Error = BusError;

    fn start(&mut self) -> Result<(), BusError> {
        self.record(Event::Start)
    }

    fn repeated_start(&mut self) -> Result<(), BusError> {
        self.record(Event::RepeatedStart)
    }

    fn write_byte(&mut self, byte: u8) -> Result<Acknowledgement, BusError> {
        let index = self.events.len();
        self.record(Event::Write(byte))?;
        Ok(if self.nack_at == Some(index) {
            Acknowledgement::Nack
        } else {
            Acknowledgement::Ack
        })
    }

    fn read_buffer(&mut self, output: &mut [u8]) -> Result<(), BusError> {
        assert_eq!(output.len(), self.payload.len());
        let result = self.record(Event::Read(output.len()));
        if result.is_err() {
            let prefix = output.len().min(3);
            output[..prefix].copy_from_slice(&self.payload[..prefix]);
        } else {
            output.copy_from_slice(&self.payload);
        }
        result
    }

    fn stop(&mut self) -> Result<(), BusError> {
        self.record(Event::Stop)
    }
}

const CORE_TRACE: [Event; 8] = [
    Event::Start,
    Event::Write(0xa0),
    Event::Write(0x0e),
    Event::Write(0x70),
    Event::RepeatedStart,
    Event::Write(0xa1),
    Event::Read(8),
    Event::Stop,
];

#[test]
fn startup_addresses_have_literal_traces_and_route_their_payloads() {
    let core_payload = [1, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
    let display_payload = [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe];
    let mut core_output = [0; 8];
    let mut display_output = [0; 8];
    let mut bus = Mock::new(&core_payload);

    execute_read(&mut bus, plan_read(0x0e70, 8), &mut core_output).unwrap();
    assert_eq!(bus.events, CORE_TRACE);
    bus.payload.copy_from_slice(&display_payload);
    execute_read(&mut bus, plan_read(0x0e78, 8), &mut display_output).unwrap();

    assert_eq!(
        bus.events,
        [
            Event::Start,
            Event::Write(0xa0),
            Event::Write(0x0e),
            Event::Write(0x70),
            Event::RepeatedStart,
            Event::Write(0xa1),
            Event::Read(8),
            Event::Stop,
            Event::Start,
            Event::Write(0xa0),
            Event::Write(0x0e),
            Event::Write(0x78),
            Event::RepeatedStart,
            Event::Write(0xa1),
            Event::Read(8),
            Event::Stop,
        ]
    );
    assert_eq!(core_output, core_payload);
    assert_eq!(display_output, display_payload);
}

#[test]
fn wrong_destination_lengths_are_rejected_without_io_or_mutation() {
    for (size, actual) in [(8, 0), (8, 7), (8, 9), (0, 1), (255, 256)] {
        let mut bus = Mock::new(&[]);
        let mut output = vec![0x5a; actual];
        assert_eq!(
            execute_read(&mut bus, plan_read(0x0e70, size), &mut output),
            Err(ReadError::DestinationLength {
                expected: usize::from(size),
                actual,
            })
        );
        assert!(bus.events.is_empty());
        assert_eq!(output, vec![0x5a; actual]);
    }
}

#[test]
fn every_bus_error_stops_only_an_already_started_transaction_and_preserves_error() {
    for fail_at in 0..8 {
        for fail_cleanup in [false, true] {
            let mut bus = Mock::new(&[1, 2, 3, 4, 5, 6, 7, 8]);
            bus.fail_at = Some(fail_at);
            bus.fail_cleanup = fail_cleanup;
            let mut output = [0x5a; 8];

            assert_eq!(
                execute_read(&mut bus, plan_read(0x0e70, 8), &mut output),
                Err(ReadError::Transport(BusError::Transfer(fail_at)))
            );
            let mut expected = CORE_TRACE[..=fail_at].to_vec();
            if (1..7).contains(&fail_at) {
                expected.push(Event::Stop);
            }
            assert_eq!(bus.events, expected, "failure at operation {fail_at}");
            assert_eq!(
                output,
                match fail_at {
                    6 => [1, 2, 3, 0x5a, 0x5a, 0x5a, 0x5a, 0x5a],
                    7 => [1, 2, 3, 4, 5, 6, 7, 8],
                    _ => [0x5a; 8],
                }
            );
        }
    }
}

#[test]
fn every_write_nack_stops_without_further_transfers_even_if_cleanup_fails() {
    for nack_at in [1, 2, 3, 5] {
        for fail_cleanup in [false, true] {
            let mut bus = Mock::new(&[0xff; 8]);
            bus.nack_at = Some(nack_at);
            bus.fail_cleanup = fail_cleanup;
            let mut output = [0x5a; 8];

            assert_eq!(
                execute_read(&mut bus, plan_read(0x0e70, 8), &mut output),
                Err(ReadError::NotAcknowledged)
            );
            let mut expected = CORE_TRACE[..=nack_at].to_vec();
            expected.push(Event::Stop);
            assert_eq!(bus.events, expected, "NACK at operation {nack_at}");
            assert_eq!(output, [0x5a; 8]);
        }
    }
}

#[test]
fn erased_data_is_a_successful_response() {
    let mut bus = Mock::new(&[0xff; 8]);
    let mut output = [0; 8];

    assert_eq!(
        execute_read(&mut bus, plan_read(0x0e70, 8), &mut output),
        Ok(())
    );
    assert_eq!(bus.events, CORE_TRACE);
    assert_eq!(output, [0xff; 8]);
}

#[test]
fn zero_length_still_executes_the_full_trace_at_a_full_width_address() {
    let mut bus = Mock::new(&[]);

    execute_read(&mut bus, plan_read(0xffff, 0), &mut []).unwrap();
    assert_eq!(
        bus.events,
        [
            Event::Start,
            Event::Write(0xa0),
            Event::Write(0xff),
            Event::Write(0xff),
            Event::RepeatedStart,
            Event::Write(0xa1),
            Event::Read(0),
            Event::Stop,
        ]
    );
}

#[test]
fn maximum_read_length_is_routed_to_the_borrowed_slice_without_touching_its_edges() {
    let payload: Vec<u8> = (0..255).collect();
    let mut bus = Mock::new(&payload);
    let mut storage = [0x5a; 257];

    execute_read(&mut bus, plan_read(0x1fff, 255), &mut storage[1..256]).unwrap();
    assert_eq!(
        bus.events,
        [
            Event::Start,
            Event::Write(0xa0),
            Event::Write(0x1f),
            Event::Write(0xff),
            Event::RepeatedStart,
            Event::Write(0xa1),
            Event::Read(255),
            Event::Stop,
        ]
    );
    assert_eq!(&storage[1..256], payload);
    assert_eq!(storage[0], 0x5a);
    assert_eq!(storage[256], 0x5a);
}
