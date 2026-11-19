#[path = "../src/keyboard_matrix_scan.rs"]
mod keyboard_matrix_scan;

use keyboard_matrix_scan::{Key, MatrixIo, poll};
use std::collections::VecDeque;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Op {
    Update(u32, u32),
    Delay,
    Read(u32),
    Stop,
}

struct Mock {
    readings: VecDeque<u32>,
    trace: Vec<Op>,
    port: u32,
}

impl MatrixIo for Mock {
    fn update_gpioa(&mut self, and_mask: u32, or_mask: u32) {
        self.trace.push(Op::Update(and_mask, or_mask));
        self.port = (self.port & and_mask) | or_mask;
    }

    fn read_gpioa(&mut self) -> u32 {
        let reading = self.readings.pop_front().expect("unexpected sample");
        self.trace.push(Op::Read(reading));
        reading
    }

    fn delay_1us(&mut self) {
        self.trace.push(Op::Delay);
    }

    fn i2c_stop(&mut self) {
        self.trace.push(Op::Stop);
        self.port |= 0x0c00;
    }
}

const MASKS: [u32; 5] = [0xffff, 0xfbff, 0xf7ff, 0xefff, 0xdfff];
const CLEANUP: [Op; 3] = [
    Op::Stop,
    Op::Update(0xffff_efff, 0),
    Op::Update(0xffff_ffff, 0x2000),
];

fn check(rows: &[Vec<u32>], expected: Option<Key>) -> Mock {
    let mut trace = Vec::new();
    for (row, readings) in rows.iter().enumerate() {
        trace.push(Op::Update(0xffff_ffff, 0x3c00));
        trace.push(Op::Update(MASKS[row], 0));
        for &reading in readings {
            trace.push(Op::Delay);
            trace.push(Op::Read(reading));
        }
    }
    trace.extend(CLEANUP);
    let mut io = Mock {
        readings: rows.iter().flatten().copied().collect(),
        trace: Vec::new(),
        port: 0xa5a5_c3a5,
    };
    assert_eq!(poll(&mut io), expected);
    assert!(io.readings.is_empty(), "scan returned too soon");
    assert_eq!(io.trace, trace);
    // The row mask clears the upper half; STOP and voice updates restore
    // PA10/11/13 high and PA12 low without disturbing the other low bits.
    assert_eq!(io.port, 0xefa5);
    io
}

fn stable_rows(row: usize, reading: u32) -> Vec<Vec<u32>> {
    let mut rows = vec![vec![0xffff; 3]; row];
    rows.push(vec![reading; 3]);
    rows
}

#[test]
fn every_literal_key_table_entry_and_numeric_code() {
    let table = [[(Key::Side1, 18), (Key::Side2, 17)]];
    for (column, &(key, code)) in table[0].iter().enumerate() {
        assert_eq!(key as u8, code);
        check(&stable_rows(0, 0xffff & !(1 << (3 + column))), Some(key));
    }

    let matrix = [
        [
            (Key::Menu, 10),
            (Key::Digit1, 1),
            (Key::Digit4, 4),
            (Key::Digit7, 7),
        ],
        [
            (Key::Up, 11),
            (Key::Digit2, 2),
            (Key::Digit5, 5),
            (Key::Digit8, 8),
        ],
        [
            (Key::Down, 12),
            (Key::Digit3, 3),
            (Key::Digit6, 6),
            (Key::Digit9, 9),
        ],
        [
            (Key::Exit, 13),
            (Key::Star, 14),
            (Key::Digit0, 0),
            (Key::F, 15),
        ],
    ];
    for (row, keys) in matrix.iter().enumerate() {
        for (column, &(key, code)) in keys.iter().enumerate() {
            assert_eq!(key as u8, code);
            check(
                &stable_rows(row + 1, 0xffff & !(1 << (3 + column))),
                Some(key),
            );
        }
    }
}

#[test]
fn first_low_column_wins_every_pair_in_each_row() {
    let keys = [
        [Key::Menu, Key::Digit1, Key::Digit4, Key::Digit7],
        [Key::Up, Key::Digit2, Key::Digit5, Key::Digit8],
        [Key::Down, Key::Digit3, Key::Digit6, Key::Digit9],
        [Key::Exit, Key::Star, Key::Digit0, Key::F],
    ];
    check(&stable_rows(0, 0xffe7), Some(Key::Side1));
    for (row, keys) in keys.iter().enumerate() {
        for (first, &key) in keys.iter().enumerate() {
            for second in first + 1..4 {
                let reading = 0xffff & !(1 << (3 + first)) & !(1 << (3 + second));
                check(&stable_rows(row + 1, reading), Some(key));
            }
        }
    }
}

#[test]
fn first_row_with_a_key_wins_and_later_rows_are_not_touched() {
    for (row, key) in [Key::Side2, Key::Digit7, Key::Digit8, Key::Digit9, Key::F]
        .into_iter()
        .enumerate()
    {
        // Every later row would also read low if visited: the strict mock
        // rejects any access after the first winning row.
        let pin = if row == 0 { 4 } else { 6 };
        check(&stable_rows(row, 0xffff & !(1 << pin)), Some(key));
    }
}

#[test]
fn no_key_visits_all_rows_and_side_padding_does_not_decode_pa5_or_pa6() {
    check(&vec![vec![0xffff; 3]; 5], None);
    let mut rows = vec![vec![0xffff; 3]; 5];
    rows[0] = vec![0xff9f; 3];
    check(&rows, None);
}

#[test]
fn changed_reading_is_sample_one_and_can_stabilize_at_attempt_eight() {
    check(
        &[vec![1, 2, 3, 4, 5, 0xfff7, 0xfff7, 0xfff7]],
        Some(Key::Side1),
    );
    check(&[vec![0, 0, 0]], Some(Key::Side1));
    check(&[vec![1, 0xfff7, 0xfff7, 0xfff7]], Some(Key::Side1));
}

#[test]
fn two_matching_samples_at_the_limit_are_not_enough() {
    check(&[vec![1, 2, 3, 4, 5, 6, 0xfff7, 0xfff7]], None);
}

#[test]
fn noise_in_any_low_sixteen_bit_aborts_immediately_on_any_row() {
    for bit in 0..16 {
        for row in 0..5 {
            let mut rows = vec![vec![0xffff; 3]; row];
            // PA3 stays low except when it is itself the noise source.
            rows.push((0..8).map(|i| 0xfff7 ^ ((i % 2) << bit)).collect());
            check(&rows, None);
        }
    }
}

#[test]
fn upper_port_bits_are_truncated_before_stability_comparison() {
    check(
        &[vec![0x1234_fff7, 0xabcd_fff7, 0xffff_fff7]],
        Some(Key::Side1),
    );
}

#[test]
fn stable_count_is_reset_between_rows() {
    check(&[vec![0xffff; 3], vec![0xfff7; 3]], Some(Key::Menu));
    check(
        &[
            vec![0xffff; 3],
            vec![0xffff, 0xffff, 0xfff7, 0xfff7, 0xfff7],
        ],
        Some(Key::Menu),
    );
}
