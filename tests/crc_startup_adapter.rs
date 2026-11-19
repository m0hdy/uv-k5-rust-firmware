#[path = "../src/crc_startup_adapter.rs"]
mod crc_startup_adapter;

use crc_startup_adapter::{WordIo, apply_with_io};

struct FakeWordIo {
    registers: [u32; 4],
    writes: Vec<(usize, u32)>,
}

impl WordIo for FakeWordIo {
    fn write_word(&mut self, address: usize, value: u32) {
        self.writes.push((address, value));
        let index = match address {
            0x4000_3000 => 0,
            0x4000_3004 => 1,
            0x4000_3008 => 2,
            0x4000_300c => 3,
            _ => panic!("unexpected register address {address:#x}"),
        };
        self.registers[index] = value;
    }
}

#[test]
fn initialization_assigns_only_control_and_initial_value_in_order() {
    let mut io = FakeWordIo {
        registers: [0xffff_ffff, 0x1234_5678, 0xa5a5_a5a5, 0x5a5a_5a5a],
        writes: Vec::new(),
    };

    apply_with_io(&mut io);

    assert_eq!(io.writes, [(0x4000_3000, 0x0000_0100), (0x4000_3004, 0)]);
    assert_eq!(io.registers, [0x0000_0100, 0, 0xa5a5_a5a5, 0x5a5a_5a5a]);
}
