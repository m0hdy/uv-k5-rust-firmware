#![cfg_attr(not(test), no_std)]

//! Caller-state model of the countdown mutations in the pinned scheduler tick.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountdownState {
    pub noaa_countdown_10ms: u16,
    pub found_cdcss_countdown_10ms: u8,
    pub found_ctcss_countdown_10ms: u8,
    pub battery_save_countdown_10ms: u16,
    pub schedule_power_save: bool,
    pub power_save_10ms: u16,
    pub power_save_countdown_expired: bool,
    pub dual_watch_countdown_10ms: u16,
    pub schedule_dual_watch: bool,
    pub noaa_schedule_countdown_10ms: u16,
    pub schedule_noaa: bool,
    pub scan_pause_delay_in_10ms: u16,
    pub schedule_scan_listen: bool,
    pub tail_tone_elimination_countdown_10ms: u16,
    pub tail_tone_elimination_complete: bool,
    pub countdown_to_play_next_voice_10ms: u16,
    pub play_queued_voice: bool,
    pub fm_play_countdown_10ms: u16,
    pub schedule_fm: bool,
    pub vox_stop_countdown_10ms: u16,
    pub boot_counter_10ms: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerFunction {
    Foreground,
    Transmit,
    Monitor,
    Incoming,
    Receive,
    PowerSave,
    BandScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanDirection {
    Off,
    Reverse,
    Forward,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Features {
    pub enable_noaa: bool,
    pub enable_voice: bool,
    pub enable_fmradio: bool,
    pub enable_vox: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TickContext {
    pub features: Features,
    pub function: SchedulerFunction,
    pub scan_direction: ScanDirection,
    pub css_background_scan: bool,
    pub dual_watch_enabled: bool,
    pub is_noaa_mode: bool,
    pub fm_scan_active: bool,
}

pub fn execute_countdowns(state: &mut CountdownState, context: TickContext) {
    if context.features.enable_noaa {
        decrement_u16(&mut state.noaa_countdown_10ms);
    }

    decrement_u8(&mut state.found_cdcss_countdown_10ms);
    decrement_u8(&mut state.found_ctcss_countdown_10ms);

    if context.function == SchedulerFunction::Foreground {
        decrement_and_trigger_u16(
            &mut state.battery_save_countdown_10ms,
            &mut state.schedule_power_save,
        );
    }

    if context.function == SchedulerFunction::PowerSave {
        decrement_and_trigger_u16(
            &mut state.power_save_10ms,
            &mut state.power_save_countdown_expired,
        );
    }

    if context.scan_direction == ScanDirection::Off
        && !context.css_background_scan
        && context.dual_watch_enabled
        && !matches!(
            context.function,
            SchedulerFunction::Monitor | SchedulerFunction::Transmit | SchedulerFunction::Receive
        )
    {
        decrement_and_trigger_u16(
            &mut state.dual_watch_countdown_10ms,
            &mut state.schedule_dual_watch,
        );
    }

    if context.features.enable_noaa
        && context.scan_direction == ScanDirection::Off
        && !context.css_background_scan
        && !context.dual_watch_enabled
        && context.is_noaa_mode
        && !matches!(
            context.function,
            SchedulerFunction::Monitor | SchedulerFunction::Transmit | SchedulerFunction::Receive
        )
    {
        decrement_and_trigger_u16(
            &mut state.noaa_schedule_countdown_10ms,
            &mut state.schedule_noaa,
        );
    }

    if context.scan_direction != ScanDirection::Off
        && !matches!(
            context.function,
            SchedulerFunction::Monitor | SchedulerFunction::Transmit
        )
    {
        decrement_and_trigger_u16(
            &mut state.scan_pause_delay_in_10ms,
            &mut state.schedule_scan_listen,
        );
    }

    decrement_and_trigger_u16(
        &mut state.tail_tone_elimination_countdown_10ms,
        &mut state.tail_tone_elimination_complete,
    );

    if context.features.enable_voice {
        decrement_and_trigger_u16(
            &mut state.countdown_to_play_next_voice_10ms,
            &mut state.play_queued_voice,
        );
    }

    if context.features.enable_fmradio
        && context.fm_scan_active
        && !matches!(
            context.function,
            SchedulerFunction::Monitor | SchedulerFunction::Transmit | SchedulerFunction::Receive
        )
    {
        decrement_and_trigger_u16(&mut state.fm_play_countdown_10ms, &mut state.schedule_fm);
    }

    if context.features.enable_vox {
        decrement_u16(&mut state.vox_stop_countdown_10ms);
    }

    decrement_u8(&mut state.boot_counter_10ms);
}

fn decrement_u8(countdown: &mut u8) {
    if *countdown > 0 {
        *countdown -= 1;
    }
}

fn decrement_u16(countdown: &mut u16) {
    if *countdown > 0 {
        *countdown -= 1;
    }
}

fn decrement_and_trigger_u16(countdown: &mut u16, flag: &mut bool) {
    if *countdown > 0 {
        *countdown -= 1;
        if *countdown == 0 {
            *flag = true;
        }
    }
}
