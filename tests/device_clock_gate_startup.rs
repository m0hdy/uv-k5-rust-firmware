#[path = "../src/device_clock_gate_startup.rs"]
mod device_clock_gate_startup;

use device_clock_gate_startup::{DEFAULT_GATES, GateValue, Register, RegisterWrite, apply_with_io};

struct FakeRegister {
    value: u32,
    writes: Vec<(Register, GateValue)>,
}

impl RegisterWrite for FakeRegister {
    fn write(&mut self, register: Register, value: GateValue) {
        self.writes.push((register, value));
        self.value = value.bits();
    }
}

#[test]
fn default_assignment_has_the_literal_address_and_only_the_expected_bits() {
    assert_eq!(Register::DeviceClockGate.address(), 0x4000_0008);
    let enabled = [0, 1, 2, 7, 10, 20, 25, 27, 28]
        .into_iter()
        .fold(0u32, |bits, shift| bits | (1 << shift));
    assert_eq!(enabled, 0x1a10_0487);
    assert_eq!(DEFAULT_GATES.bits(), enabled);
    assert_eq!(DEFAULT_GATES.bits() & !enabled, 0);
}

#[test]
fn assignment_is_one_write_without_a_snapshot_even_for_nonzero_initial_values() {
    for initial in [0, u32::MAX, 0xa5a5_5a5a] {
        let mut io = FakeRegister {
            value: initial,
            writes: Vec::new(),
        };
        apply_with_io(&mut io);
        assert_eq!(io.writes, [(Register::DeviceClockGate, DEFAULT_GATES)]);
        assert_eq!(io.value, 0x1a10_0487);
    }
}
