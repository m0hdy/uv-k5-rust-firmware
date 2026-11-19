use uv_k5_rust_firmware::eeprom_transaction::{
    Command, EEPROM_ADDRESS_LIMIT, EEPROM_WRITE_DELAY_MS, EEPROM_WRITE_SIZE, plan_read, plan_write,
};

#[test]
fn read_plan_has_the_literal_repeated_start_trace_and_big_endian_address() {
    assert_eq!(
        plan_read(0x1234, EEPROM_WRITE_SIZE as u8).commands(),
        &[
            Command::Start,
            Command::Write(0xa0),
            Command::Write(0x12),
            Command::Write(0x34),
            Command::Start,
            Command::Write(0xa1),
            Command::ReadBuffer(8),
            Command::Stop,
        ]
    );
}

#[test]
fn changed_write_has_eight_data_bytes_stop_and_eight_millisecond_wait() {
    let current = [0; EEPROM_WRITE_SIZE];
    let data = [1, 2, 3, 4, 5, 6, 7, 8];

    assert_eq!(EEPROM_WRITE_DELAY_MS, 8);
    assert_eq!(
        plan_write(0x1234, Some(&data), &current)
            .unwrap()
            .commands(),
        &[
            Command::Start,
            Command::Write(0xa0),
            Command::Write(0x12),
            Command::Write(0x34),
            Command::Write(1),
            Command::Write(2),
            Command::Write(3),
            Command::Write(4),
            Command::Write(5),
            Command::Write(6),
            Command::Write(7),
            Command::Write(8),
            Command::Stop,
            Command::DelayMs(8),
        ]
    );
}

#[test]
fn write_is_omitted_for_absent_data_or_an_address_at_or_above_the_limit() {
    let current = [0; EEPROM_WRITE_SIZE];
    let changed = [1; EEPROM_WRITE_SIZE];

    assert_eq!(plan_write(0, None, &current), None);
    for address in [EEPROM_ADDRESS_LIMIT, EEPROM_ADDRESS_LIMIT + 1, u16::MAX] {
        assert_eq!(plan_write(address, Some(&changed), &current), None);
    }
}

#[test]
fn unchanged_bytes_do_not_produce_a_write_plan() {
    let current = [0x5a; EEPROM_WRITE_SIZE];

    assert_eq!(plan_write(0x1234, Some(&current), &current), None);
}

#[test]
fn legacy_boundary_accepts_address_just_below_limit_without_span_check() {
    let current = [0; EEPROM_WRITE_SIZE];
    let changed = [0xa5; EEPROM_WRITE_SIZE];

    let plan = plan_write(0x1fff, Some(&changed), &current).unwrap();
    let commands = plan.commands();
    assert_eq!(commands[2..4], [Command::Write(0x1f), Command::Write(0xff)]);
    assert_eq!(commands.len(), EEPROM_WRITE_SIZE + 6);
    assert_eq!(commands[commands.len() - 2], Command::Stop);
    assert_eq!(
        commands[commands.len() - 1],
        Command::DelayMs(EEPROM_WRITE_DELAY_MS)
    );
}

#[test]
fn read_plan_preserves_legacy_zero_size_and_full_width_address_behavior() {
    assert_eq!(
        plan_read(u16::MAX, 0).commands(),
        &[
            Command::Start,
            Command::Write(0xa0),
            Command::Write(0xff),
            Command::Write(0xff),
            Command::Start,
            Command::Write(0xa1),
            Command::ReadBuffer(0),
            Command::Stop,
        ]
    );
}
