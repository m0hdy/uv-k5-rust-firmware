#[path = "../src/app_numeric_input_flow.rs"]
mod app_numeric_input_flow;

use app_numeric_input_flow::{
    Digit, FlowEffect, Key, KeyEvent, NumericInputBox, process_memory_channel_key_event,
};

fn release(
    input_box: &mut NumericInputBox,
    key: Key,
    f_key_pressed: bool,
    selected_vfo: u8,
    is_valid_channel: impl FnMut(u16) -> bool,
) -> Vec<FlowEffect> {
    process_memory_channel_key_event(
        input_box,
        KeyEvent::Released {
            key,
            was_held: false,
        },
        f_key_pressed,
        selected_vfo,
        is_valid_channel,
    )
    .iter()
    .collect()
}

#[test]
fn digit_press_beeps_without_appending_then_release_appends() {
    let mut input_box = NumericInputBox::new();
    let press = process_memory_channel_key_event(
        &mut input_box,
        KeyEvent::Pressed(Key::Digit4),
        false,
        0,
        |_| panic!("validity is not queried on press"),
    );

    assert_eq!(
        press.iter().collect::<Vec<_>>(),
        [FlowEffect::RequestOptionalKeyBeep]
    );
    assert_eq!(input_box.index(), 0);
    assert_eq!(input_box.ascii_projection(), *b"00000000");

    assert_eq!(
        release(&mut input_box, Key::Digit4, false, 0, |_| {
            panic!("validity is not queried for a partial entry")
        }),
        [
            FlowEffect::RefreshKeyInputTimeout,
            FlowEffect::AppendDigit(Digit::Four),
            FlowEffect::RequestMainDisplay,
        ]
    );
    assert_eq!(input_box.index(), 1);
    assert_eq!(input_box.ascii_projection(), *b"4-------");
}

#[test]
fn leading_zeroes_are_composed_and_valid_selection_forwards_vfo_in_order() {
    let mut input_box = NumericInputBox::new();
    let mut queried = Vec::new();

    for key in [Key::Digit0, Key::Digit0] {
        assert_eq!(
            release(&mut input_box, key, false, 1, |_| {
                panic!("validity is not queried before three digits")
            }),
            [
                FlowEffect::RefreshKeyInputTimeout,
                FlowEffect::AppendDigit(Digit::Zero),
                FlowEffect::RequestMainDisplay,
            ]
        );
    }

    assert_eq!(input_box.ascii_projection(), *b"00------");
    assert_eq!(
        release(&mut input_box, Key::Digit7, false, 1, |channel| {
            queried.push(channel);
            channel == 6
        }),
        [
            FlowEffect::RefreshKeyInputTimeout,
            FlowEffect::AppendDigit(Digit::Seven),
            FlowEffect::RequestMainDisplay,
            FlowEffect::ResetInputIndex,
            FlowEffect::CheckChannelValidity(6),
            FlowEffect::UpdateSelectedVfoMemoryChannel { vfo: 1, channel: 6 },
            FlowEffect::UpdateSelectedVfoScreenChannel { vfo: 1, channel: 6 },
            FlowEffect::RequestSaveVfo,
            FlowEffect::ConfigureVfoReload,
        ]
    );
    assert_eq!(queried, [6]);
    assert_eq!(input_box.index(), 0);
    assert_eq!(input_box.ascii_projection(), *b"007-----");

    assert_eq!(
        release(&mut input_box, Key::Digit2, false, 1, |_| {
            panic!("validity is not queried after starting a subsequent entry")
        }),
        [
            FlowEffect::RefreshKeyInputTimeout,
            FlowEffect::AppendDigit(Digit::Two),
            FlowEffect::RequestMainDisplay,
        ]
    );
    assert_eq!(input_box.index(), 1);
    assert_eq!(input_box.ascii_projection(), *b"2-------");
}

#[test]
fn zero_entry_preserves_wrapping_c_unsigned_channel_candidate_and_invalid_beep() {
    let mut input_box = NumericInputBox::new();
    for key in [Key::Digit0, Key::Digit0] {
        release(&mut input_box, key, false, 0, |_| false);
    }

    let mut queried = None;
    assert_eq!(
        release(&mut input_box, Key::Digit0, false, 0, |channel| {
            queried = Some(channel);
            false
        }),
        [
            FlowEffect::RefreshKeyInputTimeout,
            FlowEffect::AppendDigit(Digit::Zero),
            FlowEffect::RequestMainDisplay,
            FlowEffect::ResetInputIndex,
            FlowEffect::CheckChannelValidity(u16::MAX),
            FlowEffect::RequestOptionalInvalidChannelBeep,
        ]
    );
    assert_eq!(queried, Some(u16::MAX));
    assert_eq!(input_box.index(), 0);
}

#[test]
fn lower_upper_and_out_of_range_channel_candidates_follow_query_result() {
    for (digits, expected, valid) in [
        ([Key::Digit0, Key::Digit0, Key::Digit1], 0, true),
        ([Key::Digit2, Key::Digit0, Key::Digit0], 199, true),
        ([Key::Digit2, Key::Digit0, Key::Digit1], 200, false),
        ([Key::Digit9, Key::Digit9, Key::Digit9], 998, false),
    ] {
        let mut input_box = NumericInputBox::new();
        for key in digits {
            let trace = release(&mut input_box, key, false, 0, |channel| {
                channel == expected && valid
            });
            if input_box.index() == 0 {
                assert!(trace.contains(&FlowEffect::CheckChannelValidity(expected)));
                if valid {
                    assert!(trace.contains(&FlowEffect::UpdateSelectedVfoMemoryChannel {
                        vfo: 0,
                        channel: expected as u8,
                    }));
                    assert!(trace.contains(&FlowEffect::UpdateSelectedVfoScreenChannel {
                        vfo: 0,
                        channel: expected as u8,
                    }));
                } else {
                    assert!(trace.contains(&FlowEffect::RequestOptionalInvalidChannelBeep));
                }
            } else {
                assert_eq!(trace.len(), 3);
                assert_eq!(trace[0], FlowEffect::RefreshKeyInputTimeout);
                assert!(matches!(trace[1], FlowEffect::AppendDigit(_)));
                assert_eq!(trace[2], FlowEffect::RequestMainDisplay);
            }
        }
        assert_eq!(input_box.index(), 0);
    }
}

#[test]
fn held_f_modified_and_non_digit_events_do_not_append() {
    let mut input_box = NumericInputBox::new();

    for event in [
        KeyEvent::Held(Key::Digit3),
        KeyEvent::Released {
            key: Key::Digit3,
            was_held: true,
        },
        KeyEvent::Released {
            key: Key::Digit3,
            was_held: false,
        },
        KeyEvent::Pressed(Key::Menu),
        KeyEvent::Released {
            key: Key::Menu,
            was_held: false,
        },
    ] {
        let f_key_pressed = matches!(
            event,
            KeyEvent::Released {
                key: Key::Digit3,
                was_held: false
            }
        );
        assert!(
            process_memory_channel_key_event(&mut input_box, event, f_key_pressed, 0, |_| {
                panic!("validity is not queried for excluded events")
            })
            .iter()
            .next()
            .is_none()
        );
    }

    assert_eq!(input_box.index(), 0);
    assert_eq!(input_box.ascii_projection(), *b"00000000");
}
