#[path = "../src/scheduler_countdowns.rs"]
mod scheduler_countdowns;

use scheduler_countdowns::{
    CountdownState, Features, ScanDirection, SchedulerFunction, TickContext, execute_countdowns,
};

fn state() -> CountdownState {
    CountdownState {
        noaa_countdown_10ms: 0,
        found_cdcss_countdown_10ms: 0,
        found_ctcss_countdown_10ms: 0,
        battery_save_countdown_10ms: 0,
        schedule_power_save: false,
        power_save_10ms: 0,
        power_save_countdown_expired: false,
        dual_watch_countdown_10ms: 0,
        schedule_dual_watch: false,
        noaa_schedule_countdown_10ms: 0,
        schedule_noaa: false,
        scan_pause_delay_in_10ms: 0,
        schedule_scan_listen: false,
        tail_tone_elimination_countdown_10ms: 0,
        tail_tone_elimination_complete: false,
        countdown_to_play_next_voice_10ms: 0,
        play_queued_voice: false,
        fm_play_countdown_10ms: 0,
        schedule_fm: false,
        vox_stop_countdown_10ms: 0,
        boot_counter_10ms: 0,
    }
}

fn context() -> TickContext {
    TickContext {
        features: Features {
            enable_noaa: false,
            enable_voice: false,
            enable_fmradio: false,
            enable_vox: false,
        },
        function: SchedulerFunction::BandScope,
        scan_direction: ScanDirection::Off,
        css_background_scan: false,
        dual_watch_enabled: false,
        is_noaa_mode: false,
        fm_scan_active: false,
    }
}

fn allowed_noaa_context() -> TickContext {
    TickContext {
        features: Features {
            enable_noaa: true,
            enable_voice: false,
            enable_fmradio: false,
            enable_vox: false,
        },
        function: SchedulerFunction::Incoming,
        scan_direction: ScanDirection::Off,
        css_background_scan: false,
        dual_watch_enabled: false,
        is_noaa_mode: true,
        fm_scan_active: false,
    }
}

#[test]
fn ordinary_countdowns_saturate_at_zero_and_use_the_declared_widths() {
    let mut countdowns = state();
    countdowns.noaa_countdown_10ms = u16::MAX;
    countdowns.found_cdcss_countdown_10ms = u8::MAX;
    countdowns.found_ctcss_countdown_10ms = 1;
    countdowns.vox_stop_countdown_10ms = u16::MAX;
    countdowns.boot_counter_10ms = u8::MAX;

    let mut tick = context();
    tick.features.enable_noaa = true;
    tick.features.enable_vox = true;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.noaa_countdown_10ms, u16::MAX - 1);
    assert_eq!(countdowns.found_cdcss_countdown_10ms, u8::MAX - 1);
    assert_eq!(countdowns.found_ctcss_countdown_10ms, 0);
    assert_eq!(countdowns.vox_stop_countdown_10ms, u16::MAX - 1);
    assert_eq!(countdowns.boot_counter_10ms, u8::MAX - 1);

    countdowns.found_ctcss_countdown_10ms = u8::MAX;
    execute_countdowns(&mut countdowns, tick);
    assert_eq!(countdowns.found_ctcss_countdown_10ms, u8::MAX - 1);

    countdowns.found_ctcss_countdown_10ms = 0;
    countdowns.noaa_countdown_10ms = 0;
    countdowns.vox_stop_countdown_10ms = 0;
    countdowns.boot_counter_10ms = 0;
    execute_countdowns(&mut countdowns, tick);
    assert_eq!(countdowns.noaa_countdown_10ms, 0);
    assert_eq!(countdowns.found_ctcss_countdown_10ms, 0);
    assert_eq!(countdowns.vox_stop_countdown_10ms, 0);
    assert_eq!(countdowns.boot_counter_10ms, 0);
}

#[test]
fn trigger_countdowns_only_set_flags_on_positive_to_zero_transitions() {
    let mut countdowns = state();
    countdowns.battery_save_countdown_10ms = 2;
    countdowns.power_save_10ms = 2;
    countdowns.dual_watch_countdown_10ms = 2;
    countdowns.noaa_schedule_countdown_10ms = 2;
    countdowns.scan_pause_delay_in_10ms = 2;
    countdowns.tail_tone_elimination_countdown_10ms = 2;
    countdowns.countdown_to_play_next_voice_10ms = 2;
    countdowns.fm_play_countdown_10ms = 2;

    let mut tick = allowed_noaa_context();
    tick.features.enable_voice = true;
    tick.features.enable_fmradio = true;
    tick.function = SchedulerFunction::Foreground;
    tick.scan_direction = ScanDirection::Forward;
    tick.fm_scan_active = true;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.battery_save_countdown_10ms, 1);
    assert_eq!(countdowns.power_save_10ms, 2);
    assert_eq!(countdowns.dual_watch_countdown_10ms, 2);
    assert_eq!(countdowns.noaa_schedule_countdown_10ms, 2);
    assert_eq!(countdowns.scan_pause_delay_in_10ms, 1);
    assert_eq!(countdowns.tail_tone_elimination_countdown_10ms, 1);
    assert_eq!(countdowns.countdown_to_play_next_voice_10ms, 1);
    assert_eq!(countdowns.fm_play_countdown_10ms, 1);
    assert!(!countdowns.schedule_power_save);
    assert!(!countdowns.power_save_countdown_expired);
    assert!(!countdowns.schedule_dual_watch);
    assert!(!countdowns.schedule_noaa);
    assert!(!countdowns.schedule_scan_listen);
    assert!(!countdowns.tail_tone_elimination_complete);
    assert!(!countdowns.play_queued_voice);
    assert!(!countdowns.schedule_fm);

    tick.function = SchedulerFunction::PowerSave;
    tick.scan_direction = ScanDirection::Off;
    tick.dual_watch_enabled = true;
    tick.is_noaa_mode = false;
    execute_countdowns(&mut countdowns, tick);
    assert_eq!(countdowns.power_save_10ms, 1);
    assert_eq!(countdowns.dual_watch_countdown_10ms, 1);
    assert_eq!(countdowns.noaa_schedule_countdown_10ms, 2);

    countdowns.battery_save_countdown_10ms = 1;
    countdowns.scan_pause_delay_in_10ms = 1;
    countdowns.tail_tone_elimination_countdown_10ms = 1;
    countdowns.countdown_to_play_next_voice_10ms = 1;
    countdowns.fm_play_countdown_10ms = 1;
    tick.function = SchedulerFunction::Foreground;
    tick.scan_direction = ScanDirection::Forward;
    execute_countdowns(&mut countdowns, tick);

    assert!(countdowns.schedule_power_save);
    assert!(!countdowns.power_save_countdown_expired);
    assert!(!countdowns.schedule_dual_watch);
    assert!(!countdowns.schedule_noaa);
    assert!(countdowns.schedule_scan_listen);
    assert!(countdowns.tail_tone_elimination_complete);
    assert!(countdowns.play_queued_voice);
    assert!(countdowns.schedule_fm);

    countdowns.power_save_10ms = 1;
    countdowns.dual_watch_countdown_10ms = 1;
    tick.function = SchedulerFunction::PowerSave;
    tick.scan_direction = ScanDirection::Off;
    execute_countdowns(&mut countdowns, tick);
    assert!(countdowns.power_save_countdown_expired);
    assert!(countdowns.schedule_dual_watch);

    let already_pending = countdowns;
    execute_countdowns(&mut countdowns, tick);
    assert_eq!(countdowns, already_pending);
}

#[test]
fn maximum_trigger_countdowns_decrement_without_premature_flags() {
    let mut countdowns = state();
    countdowns.battery_save_countdown_10ms = u16::MAX;
    countdowns.power_save_10ms = u16::MAX;
    countdowns.dual_watch_countdown_10ms = u16::MAX;
    countdowns.noaa_schedule_countdown_10ms = u16::MAX;
    countdowns.scan_pause_delay_in_10ms = u16::MAX;
    countdowns.tail_tone_elimination_countdown_10ms = u16::MAX;
    countdowns.countdown_to_play_next_voice_10ms = u16::MAX;
    countdowns.fm_play_countdown_10ms = u16::MAX;

    let mut tick = allowed_noaa_context();
    tick.features.enable_voice = true;
    tick.features.enable_fmradio = true;
    tick.function = SchedulerFunction::Foreground;
    tick.scan_direction = ScanDirection::Forward;
    tick.fm_scan_active = true;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.battery_save_countdown_10ms, u16::MAX - 1);
    assert_eq!(countdowns.power_save_10ms, u16::MAX);
    assert_eq!(countdowns.dual_watch_countdown_10ms, u16::MAX);
    assert_eq!(countdowns.noaa_schedule_countdown_10ms, u16::MAX);
    assert_eq!(countdowns.scan_pause_delay_in_10ms, u16::MAX - 1);
    assert_eq!(
        countdowns.tail_tone_elimination_countdown_10ms,
        u16::MAX - 1
    );
    assert_eq!(countdowns.countdown_to_play_next_voice_10ms, u16::MAX - 1);
    assert_eq!(countdowns.fm_play_countdown_10ms, u16::MAX - 1);
    assert!(!countdowns.schedule_power_save);
    assert!(!countdowns.power_save_countdown_expired);
    assert!(!countdowns.schedule_dual_watch);
    assert!(!countdowns.schedule_noaa);
    assert!(!countdowns.schedule_scan_listen);
    assert!(!countdowns.tail_tone_elimination_complete);
    assert!(!countdowns.play_queued_voice);
    assert!(!countdowns.schedule_fm);
}

#[test]
fn maximum_power_save_dual_watch_and_noaa_triggers_do_not_fire_early() {
    let mut countdowns = state();
    countdowns.power_save_10ms = u16::MAX;
    countdowns.dual_watch_countdown_10ms = u16::MAX;
    countdowns.noaa_schedule_countdown_10ms = u16::MAX;

    let mut tick = allowed_noaa_context();
    tick.function = SchedulerFunction::PowerSave;
    tick.dual_watch_enabled = true;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.power_save_10ms, u16::MAX - 1);
    assert_eq!(countdowns.dual_watch_countdown_10ms, u16::MAX - 1);
    assert_eq!(countdowns.noaa_schedule_countdown_10ms, u16::MAX);
    assert!(!countdowns.power_save_countdown_expired);
    assert!(!countdowns.schedule_dual_watch);
    assert!(!countdowns.schedule_noaa);

    countdowns.noaa_schedule_countdown_10ms = u16::MAX;
    tick.function = SchedulerFunction::Incoming;
    tick.dual_watch_enabled = false;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.noaa_schedule_countdown_10ms, u16::MAX - 1);
    assert!(!countdowns.schedule_noaa);
}

#[test]
fn zero_trigger_countdowns_do_not_create_new_flags() {
    let mut countdowns = state();
    let mut tick = allowed_noaa_context();
    tick.features.enable_voice = true;
    tick.features.enable_fmradio = true;
    tick.function = SchedulerFunction::Foreground;
    tick.scan_direction = ScanDirection::Forward;
    tick.fm_scan_active = true;

    execute_countdowns(&mut countdowns, tick);
    assert_eq!(countdowns, state());
}

#[test]
fn zero_power_save_countdowns_do_not_schedule_flags() {
    for dual_watch_enabled in [true, false] {
        let mut countdowns = state();
        let mut tick = allowed_noaa_context();
        tick.function = SchedulerFunction::PowerSave;
        tick.dual_watch_enabled = dual_watch_enabled;

        execute_countdowns(&mut countdowns, tick);

        assert_eq!(countdowns, state());
    }
}

#[test]
fn already_pending_flags_are_sticky_while_countdowns_continue() {
    let mut countdowns = state();
    countdowns.battery_save_countdown_10ms = 2;
    countdowns.schedule_power_save = true;
    countdowns.power_save_10ms = 2;
    countdowns.power_save_countdown_expired = true;
    countdowns.tail_tone_elimination_countdown_10ms = 2;
    countdowns.tail_tone_elimination_complete = true;

    let mut tick = context();
    tick.function = SchedulerFunction::Foreground;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.battery_save_countdown_10ms, 1);
    assert!(countdowns.schedule_power_save);
    assert_eq!(countdowns.power_save_10ms, 2);
    assert!(countdowns.power_save_countdown_expired);
    assert_eq!(countdowns.tail_tone_elimination_countdown_10ms, 1);
    assert!(countdowns.tail_tone_elimination_complete);
}

#[test]
fn every_optional_feature_is_an_explicit_gate() {
    let mut countdowns = state();
    countdowns.noaa_countdown_10ms = 3;
    countdowns.noaa_schedule_countdown_10ms = 1;
    countdowns.countdown_to_play_next_voice_10ms = 1;
    countdowns.fm_play_countdown_10ms = 1;
    countdowns.vox_stop_countdown_10ms = 3;

    let mut tick = allowed_noaa_context();
    tick.features.enable_noaa = false;
    tick.features.enable_voice = false;
    tick.features.enable_fmradio = false;
    tick.features.enable_vox = false;
    tick.fm_scan_active = true;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.noaa_countdown_10ms, 3);
    assert_eq!(countdowns.noaa_schedule_countdown_10ms, 1);
    assert_eq!(countdowns.countdown_to_play_next_voice_10ms, 1);
    assert_eq!(countdowns.fm_play_countdown_10ms, 1);
    assert_eq!(countdowns.vox_stop_countdown_10ms, 3);
    assert!(!countdowns.schedule_noaa);
    assert!(!countdowns.play_queued_voice);
    assert!(!countdowns.schedule_fm);

    tick.features.enable_noaa = true;
    tick.features.enable_voice = true;
    tick.features.enable_fmradio = true;
    tick.features.enable_vox = true;
    execute_countdowns(&mut countdowns, tick);
    assert_eq!(countdowns.noaa_countdown_10ms, 2);
    assert_eq!(countdowns.noaa_schedule_countdown_10ms, 0);
    assert!(countdowns.schedule_noaa);
    assert!(countdowns.play_queued_voice);
    assert!(countdowns.schedule_fm);
    assert_eq!(countdowns.countdown_to_play_next_voice_10ms, 0);
    assert_eq!(countdowns.fm_play_countdown_10ms, 0);
    assert_eq!(countdowns.vox_stop_countdown_10ms, 2);
}

#[test]
fn optional_feature_gates_are_independent_and_power_save_flags_stay_sticky() {
    for feature_bits in 0..16 {
        let features = Features {
            enable_noaa: feature_bits & 0b0001 != 0,
            enable_voice: feature_bits & 0b0010 != 0,
            enable_fmradio: feature_bits & 0b0100 != 0,
            enable_vox: feature_bits & 0b1000 != 0,
        };
        let mut countdowns = state();
        countdowns.noaa_countdown_10ms = 2;
        countdowns.noaa_schedule_countdown_10ms = 2;
        countdowns.countdown_to_play_next_voice_10ms = 1;
        countdowns.fm_play_countdown_10ms = 1;
        countdowns.vox_stop_countdown_10ms = 2;

        let mut tick = allowed_noaa_context();
        tick.features = features;
        tick.fm_scan_active = true;
        execute_countdowns(&mut countdowns, tick);

        let mut expected = state();
        expected.noaa_countdown_10ms = if features.enable_noaa { 1 } else { 2 };
        expected.noaa_schedule_countdown_10ms = if features.enable_noaa { 1 } else { 2 };
        expected.countdown_to_play_next_voice_10ms = u16::from(!features.enable_voice);
        expected.play_queued_voice = features.enable_voice;
        expected.fm_play_countdown_10ms = u16::from(!features.enable_fmradio);
        expected.schedule_fm = features.enable_fmradio;
        expected.vox_stop_countdown_10ms = if features.enable_vox { 1 } else { 2 };
        assert_eq!(
            countdowns, expected,
            "feature combination {feature_bits:04b}"
        );
    }

    let mut countdowns = state();
    countdowns.power_save_10ms = 2;
    countdowns.power_save_countdown_expired = true;
    let mut tick = context();
    tick.function = SchedulerFunction::PowerSave;
    execute_countdowns(&mut countdowns, tick);

    assert_eq!(countdowns.power_save_10ms, 1);
    assert!(countdowns.power_save_countdown_expired);
}

#[test]
fn foreground_and_power_save_countdowns_require_their_exact_function() {
    for function in [
        SchedulerFunction::Foreground,
        SchedulerFunction::Transmit,
        SchedulerFunction::Monitor,
        SchedulerFunction::Incoming,
        SchedulerFunction::Receive,
        SchedulerFunction::PowerSave,
        SchedulerFunction::BandScope,
    ] {
        let mut countdowns = state();
        countdowns.battery_save_countdown_10ms = 1;
        countdowns.power_save_10ms = 1;
        let mut tick = context();
        tick.function = function;
        execute_countdowns(&mut countdowns, tick);

        assert_eq!(
            countdowns.battery_save_countdown_10ms,
            u16::from(function != SchedulerFunction::Foreground)
        );
        assert_eq!(
            countdowns.schedule_power_save,
            function == SchedulerFunction::Foreground
        );
        assert_eq!(
            countdowns.power_save_10ms,
            u16::from(function != SchedulerFunction::PowerSave)
        );
        assert_eq!(
            countdowns.power_save_countdown_expired,
            function == SchedulerFunction::PowerSave
        );
    }
}

#[test]
fn dual_watch_requires_off_scan_no_css_background_scan_and_non_rx_function() {
    for function in [
        SchedulerFunction::Foreground,
        SchedulerFunction::Transmit,
        SchedulerFunction::Monitor,
        SchedulerFunction::Incoming,
        SchedulerFunction::Receive,
        SchedulerFunction::PowerSave,
        SchedulerFunction::BandScope,
    ] {
        let mut countdowns = state();
        countdowns.dual_watch_countdown_10ms = 1;
        let mut tick = context();
        tick.function = function;
        tick.dual_watch_enabled = true;
        execute_countdowns(&mut countdowns, tick);

        let allowed = !matches!(
            function,
            SchedulerFunction::Monitor | SchedulerFunction::Transmit | SchedulerFunction::Receive
        );
        assert_eq!(countdowns.dual_watch_countdown_10ms, u16::from(!allowed));
        assert_eq!(countdowns.schedule_dual_watch, allowed);
    }

    for (scan_direction, css_background_scan, dual_watch_enabled) in [
        (ScanDirection::Reverse, false, true),
        (ScanDirection::Forward, false, true),
        (ScanDirection::Off, true, true),
        (ScanDirection::Off, false, false),
    ] {
        let mut countdowns = state();
        countdowns.dual_watch_countdown_10ms = 1;
        let mut tick = context();
        tick.scan_direction = scan_direction;
        tick.css_background_scan = css_background_scan;
        tick.dual_watch_enabled = dual_watch_enabled;
        execute_countdowns(&mut countdowns, tick);
        assert_eq!(countdowns.dual_watch_countdown_10ms, 1);
        assert!(!countdowns.schedule_dual_watch);
    }
}

#[test]
fn noaa_scheduling_is_the_distinct_dual_watch_off_branch() {
    for function in [
        SchedulerFunction::Foreground,
        SchedulerFunction::Transmit,
        SchedulerFunction::Monitor,
        SchedulerFunction::Incoming,
        SchedulerFunction::Receive,
        SchedulerFunction::PowerSave,
        SchedulerFunction::BandScope,
    ] {
        let mut countdowns = state();
        countdowns.noaa_countdown_10ms = 3;
        countdowns.noaa_schedule_countdown_10ms = 1;
        let mut tick = allowed_noaa_context();
        tick.function = function;
        execute_countdowns(&mut countdowns, tick);

        let allowed = !matches!(
            function,
            SchedulerFunction::Monitor | SchedulerFunction::Transmit | SchedulerFunction::Receive
        );
        assert_eq!(countdowns.noaa_countdown_10ms, 2);
        assert_eq!(countdowns.noaa_schedule_countdown_10ms, u16::from(!allowed));
        assert_eq!(countdowns.schedule_noaa, allowed);
    }

    for (scan_direction, css_background_scan, dual_watch_enabled, is_noaa_mode) in [
        (ScanDirection::Reverse, false, false, true),
        (ScanDirection::Forward, false, false, true),
        (ScanDirection::Off, true, false, true),
        (ScanDirection::Off, false, true, true),
        (ScanDirection::Off, false, false, false),
    ] {
        let mut countdowns = state();
        countdowns.noaa_schedule_countdown_10ms = 1;
        let mut tick = allowed_noaa_context();
        tick.scan_direction = scan_direction;
        tick.css_background_scan = css_background_scan;
        tick.dual_watch_enabled = dual_watch_enabled;
        tick.is_noaa_mode = is_noaa_mode;
        execute_countdowns(&mut countdowns, tick);
        assert_eq!(countdowns.noaa_schedule_countdown_10ms, 1);
        assert!(!countdowns.schedule_noaa);
    }
}

#[test]
fn scan_listen_direction_and_function_guards_match_the_scheduler() {
    for scan_direction in [
        ScanDirection::Off,
        ScanDirection::Reverse,
        ScanDirection::Forward,
    ] {
        for function in [
            SchedulerFunction::Foreground,
            SchedulerFunction::Transmit,
            SchedulerFunction::Monitor,
            SchedulerFunction::Incoming,
            SchedulerFunction::Receive,
            SchedulerFunction::PowerSave,
            SchedulerFunction::BandScope,
        ] {
            let mut countdowns = state();
            countdowns.scan_pause_delay_in_10ms = 1;
            let mut tick = context();
            tick.scan_direction = scan_direction;
            tick.function = function;
            tick.css_background_scan = true;
            execute_countdowns(&mut countdowns, tick);

            let allowed = scan_direction != ScanDirection::Off
                && !matches!(
                    function,
                    SchedulerFunction::Monitor | SchedulerFunction::Transmit
                );
            assert_eq!(countdowns.scan_pause_delay_in_10ms, u16::from(!allowed));
            assert_eq!(countdowns.schedule_scan_listen, allowed);
        }
    }
}

#[test]
fn fm_play_requires_feature_scan_activity_and_non_rx_function() {
    for function in [
        SchedulerFunction::Foreground,
        SchedulerFunction::Transmit,
        SchedulerFunction::Monitor,
        SchedulerFunction::Incoming,
        SchedulerFunction::Receive,
        SchedulerFunction::PowerSave,
        SchedulerFunction::BandScope,
    ] {
        let mut countdowns = state();
        countdowns.fm_play_countdown_10ms = 1;
        let mut tick = context();
        tick.features.enable_fmradio = true;
        tick.fm_scan_active = true;
        tick.function = function;
        execute_countdowns(&mut countdowns, tick);

        let allowed = !matches!(
            function,
            SchedulerFunction::Monitor | SchedulerFunction::Transmit | SchedulerFunction::Receive
        );
        assert_eq!(countdowns.fm_play_countdown_10ms, u16::from(!allowed));
        assert_eq!(countdowns.schedule_fm, allowed);
    }

    let mut countdowns = state();
    countdowns.fm_play_countdown_10ms = 1;
    let mut tick = context();
    tick.features.enable_fmradio = true;
    tick.fm_scan_active = false;
    execute_countdowns(&mut countdowns, tick);
    assert_eq!(countdowns.fm_play_countdown_10ms, 1);
    assert!(!countdowns.schedule_fm);
}

#[test]
fn one_tick_trace_preserves_simultaneous_ordered_countdown_results() {
    let mut countdowns = state();
    countdowns.noaa_countdown_10ms = 2;
    countdowns.found_cdcss_countdown_10ms = 2;
    countdowns.found_ctcss_countdown_10ms = 2;
    countdowns.battery_save_countdown_10ms = 2;
    countdowns.power_save_10ms = 2;
    countdowns.dual_watch_countdown_10ms = 2;
    countdowns.noaa_schedule_countdown_10ms = 2;
    countdowns.scan_pause_delay_in_10ms = 2;
    countdowns.tail_tone_elimination_countdown_10ms = 2;
    countdowns.countdown_to_play_next_voice_10ms = 2;
    countdowns.fm_play_countdown_10ms = 2;
    countdowns.vox_stop_countdown_10ms = 2;
    countdowns.boot_counter_10ms = 2;

    let mut tick = allowed_noaa_context();
    tick.features.enable_voice = true;
    tick.features.enable_fmradio = true;
    tick.features.enable_vox = true;
    tick.function = SchedulerFunction::Foreground;
    tick.scan_direction = ScanDirection::Reverse;
    tick.fm_scan_active = true;
    let mut trace = [countdowns; 2];
    for snapshot in &mut trace {
        execute_countdowns(&mut countdowns, tick);
        *snapshot = countdowns;
    }

    assert_eq!(
        trace[0],
        CountdownState {
            noaa_countdown_10ms: 1,
            found_cdcss_countdown_10ms: 1,
            found_ctcss_countdown_10ms: 1,
            battery_save_countdown_10ms: 1,
            schedule_power_save: false,
            power_save_10ms: 2,
            power_save_countdown_expired: false,
            dual_watch_countdown_10ms: 2,
            schedule_dual_watch: false,
            noaa_schedule_countdown_10ms: 2,
            schedule_noaa: false,
            scan_pause_delay_in_10ms: 1,
            schedule_scan_listen: false,
            tail_tone_elimination_countdown_10ms: 1,
            tail_tone_elimination_complete: false,
            countdown_to_play_next_voice_10ms: 1,
            play_queued_voice: false,
            fm_play_countdown_10ms: 1,
            schedule_fm: false,
            vox_stop_countdown_10ms: 1,
            boot_counter_10ms: 1,
        }
    );
    assert_eq!(
        trace[1],
        CountdownState {
            noaa_countdown_10ms: 0,
            found_cdcss_countdown_10ms: 0,
            found_ctcss_countdown_10ms: 0,
            battery_save_countdown_10ms: 0,
            schedule_power_save: true,
            power_save_10ms: 2,
            power_save_countdown_expired: false,
            dual_watch_countdown_10ms: 2,
            schedule_dual_watch: false,
            noaa_schedule_countdown_10ms: 2,
            schedule_noaa: false,
            scan_pause_delay_in_10ms: 0,
            schedule_scan_listen: true,
            tail_tone_elimination_countdown_10ms: 0,
            tail_tone_elimination_complete: true,
            countdown_to_play_next_voice_10ms: 0,
            play_queued_voice: true,
            fm_play_countdown_10ms: 0,
            schedule_fm: true,
            vox_stop_countdown_10ms: 0,
            boot_counter_10ms: 0,
        }
    );

    countdowns = trace[1];
    let mut power_save_tick = tick;
    power_save_tick.function = SchedulerFunction::PowerSave;
    power_save_tick.scan_direction = ScanDirection::Off;
    power_save_tick.dual_watch_enabled = true;
    power_save_tick.is_noaa_mode = false;
    execute_countdowns(&mut countdowns, power_save_tick);
    assert_eq!(countdowns.power_save_10ms, 1);
    assert_eq!(countdowns.dual_watch_countdown_10ms, 1);
    assert_eq!(countdowns.noaa_schedule_countdown_10ms, 2);
}
