use uv_k5_rust_firmware::app_channel_controls::{self, Context, Direction, Effect, Effects};
use uv_k5_rust_firmware::keyboard_poll_flow::{Key, KeyEvent};

fn context() -> Context {
    Context {
        screen_channel: 12,
        frequency_mode: false,
        input_box_index: 0,
        scan_active: false,
        noaa_enabled: false,
    }
}

fn collect(effects: Effects) -> Vec<Effect> {
    effects.iter().collect()
}

#[test]
fn frequency_step_applies_valid_frequency_and_requests_channel_save() {
    let mut stepped = None;
    let effects = app_channel_controls::process(
        Context {
            frequency_mode: true,
            ..context()
        },
        KeyEvent::Pressed(Key::Up),
        |direction| {
            stepped = Some(direction);
            (145_525_000, true)
        },
        |_, _| panic!("frequency mode must not search memory channels"),
    );
    assert_eq!(stepped, Some(Direction::Up));
    assert_eq!(
        collect(effects),
        [
            Effect::ShortKeyBeep,
            Effect::SetFrequency(145_525_000),
            Effect::TurnReceiverOn,
            Effect::RequestSaveChannel,
        ]
    );
}

#[test]
fn invalid_frequency_stops_before_radio_or_persistence_effects() {
    let effects = app_channel_controls::process(
        Context {
            frequency_mode: true,
            ..context()
        },
        KeyEvent::Pressed(Key::Down),
        |_| (0, false),
        |_, _| panic!("frequency mode must not search memory channels"),
    );
    assert_eq!(
        collect(effects),
        [Effect::ShortKeyBeep, Effect::InvalidFrequencyBeep]
    );
}

#[test]
fn memory_step_uses_signed_candidate_and_requests_reload() {
    let mut candidate = None;
    let effects = app_channel_controls::process(
        context(),
        KeyEvent::Pressed(Key::Down),
        |_| panic!("memory mode must not step frequency"),
        |channel, direction| {
            candidate = Some((channel, direction));
            Some(11)
        },
    );
    assert_eq!(candidate, Some((11, Direction::Down)));
    assert_eq!(
        collect(effects),
        [
            Effect::ShortKeyBeep,
            Effect::SetMemoryChannel(11),
            Effect::RequestSaveVfo,
            Effect::ConfigureVfoReload,
        ]
    );
}

#[test]
fn unchanged_or_missing_memory_channel_does_not_reload() {
    for next in [Some(12), None] {
        let effects = app_channel_controls::process(
            context(),
            KeyEvent::Pressed(Key::Up),
            |direction| panic!("unexpected frequency step: {direction:?}"),
            |_, _| next,
        );
        assert_eq!(collect(effects), [Effect::ShortKeyBeep]);
    }
}

#[test]
fn noaa_channels_wrap_at_both_ends() {
    for (channel, key, expected) in [(207, Key::Down, 216), (216, Key::Up, 207)] {
        let effects = app_channel_controls::process(
            Context {
                screen_channel: channel,
                noaa_enabled: true,
                ..context()
            },
            KeyEvent::Pressed(key),
            |_| panic!("NOAA mode must not step frequency"),
            |_, _| panic!("NOAA mode must not search memory channels"),
        );
        assert_eq!(
            collect(effects),
            [
                Effect::ShortKeyBeep,
                Effect::SetNoaaChannel(expected),
                Effect::RequestSaveVfo,
                Effect::ConfigureVfoReload,
            ]
        );
    }
}

#[test]
fn scan_key_starts_directional_scan_and_resets_pause_state() {
    let effects = app_channel_controls::process(
        Context {
            scan_active: true,
            ..context()
        },
        KeyEvent::Held(Key::Down),
        |direction| panic!("scan mode must not step frequency: {direction:?}"),
        |_, direction| panic!("scan mode must not search channels: {direction:?}"),
    );
    assert_eq!(
        collect(effects),
        [
            Effect::StartScan(Direction::Down),
            Effect::SetScanPauseCountdown(1),
            Effect::ClearScheduledScanListen,
            Effect::MarkPttReleased,
        ]
    );
}

#[test]
fn input_box_blocks_up_down_and_long_release_can_announce_memory_channel() {
    let blocked = app_channel_controls::process(
        Context {
            input_box_index: 1,
            ..context()
        },
        KeyEvent::Pressed(Key::Up),
        |_| panic!("input box must block frequency changes"),
        |_, _| panic!("input box must block memory changes"),
    );
    assert_eq!(collect(blocked), [Effect::OptionalKeyBeep]);

    let announce = app_channel_controls::process(
        context(),
        KeyEvent::Released {
            key: Key::Up,
            was_held: true,
        },
        |_| panic!("release must not tune"),
        |_, _| panic!("release must not change channel"),
    );
    assert_eq!(collect(announce), [Effect::RequestVoiceChannelNumber(13)]);
}
