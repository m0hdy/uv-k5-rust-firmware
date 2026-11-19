#[path = "../src/numeric_input_box.rs"]
mod numeric_input_box;

use numeric_input_box::{Digit, InputEvent, NumericInputBox, SLOT_COUNT, Slot};

#[test]
fn initial_state_projects_zero_initialized_cells() {
    let input = NumericInputBox::new();

    assert_eq!(input.index(), 0);
    assert_eq!(*input.slots(), [Slot::Digit(Digit::Zero); SLOT_COUNT]);
    assert_eq!(input.ascii_projection(), *b"00000000");
}

#[test]
fn no_digit_at_index_zero_clears_to_placeholders_without_advancing() {
    let mut input = NumericInputBox::new();

    input.append(InputEvent::NoDigit);

    assert_eq!(input.index(), 0);
    assert_eq!(*input.slots(), [Slot::Placeholder; SLOT_COUNT]);
    assert_eq!(input.ascii_projection(), *b"--------");
}

#[test]
fn no_digit_mid_entry_preserves_cells_and_index() {
    let mut input = NumericInputBox::new();
    input.append(InputEvent::Digit(Digit::Four));
    let before = *input.slots();

    input.append(InputEvent::NoDigit);

    assert_eq!(input.index(), 1);
    assert_eq!(*input.slots(), before);
    assert_eq!(input.ascii_projection(), *b"4-------");
}

#[test]
fn leading_zero_is_a_digit_not_an_empty_slot() {
    let mut input = NumericInputBox::new();

    input.append(InputEvent::Digit(Digit::Zero));
    input.append(InputEvent::Digit(Digit::Seven));

    assert_eq!(input.index(), 2);
    assert_eq!(input.ascii_projection(), *b"07------");
}

#[test]
fn full_buffer_ignores_additional_digits_and_no_digit_events() {
    let mut input = NumericInputBox::new();
    for digit in [
        Digit::One,
        Digit::Two,
        Digit::Three,
        Digit::Four,
        Digit::Five,
        Digit::Six,
        Digit::Seven,
        Digit::Eight,
    ] {
        input.append(InputEvent::Digit(digit));
    }
    let before = *input.slots();

    input.append(InputEvent::Digit(Digit::Nine));
    input.append(InputEvent::NoDigit);

    assert_eq!(input.index(), SLOT_COUNT);
    assert_eq!(*input.slots(), before);
    assert_eq!(input.ascii_projection(), *b"12345678");
}

#[test]
fn resetting_index_preserves_cells_until_next_append() {
    let mut input = NumericInputBox::new();
    input.append(InputEvent::Digit(Digit::One));
    input.append(InputEvent::Digit(Digit::Two));

    input.reset_index();

    assert_eq!(input.index(), 0);
    assert_eq!(input.ascii_projection(), *b"12------");

    input.append(InputEvent::Digit(Digit::Nine));

    assert_eq!(input.index(), 1);
    assert_eq!(input.ascii_projection(), *b"9-------");
}
