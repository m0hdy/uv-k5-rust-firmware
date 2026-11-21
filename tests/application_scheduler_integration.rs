use uv_k5_rust_firmware::app_500ms_service_timers::BacklightMenuItem;
use uv_k5_rust_firmware::app_500ms_timeouts::CenterLine;
use uv_k5_rust_firmware::application_runtime::{Context, Effects, State};
use uv_k5_rust_firmware::keyboard_poll_flow::{Key, KeyEvent};
use uv_k5_rust_firmware::scheduler::composition::{AppSchedulerComposition, ApplicationEffects};

fn context() -> Context {
    Context {
        enable_rssi_bar: false,
        center_line: CenterLine::None,
        enable_fmradio: false,
        fm_radio_mode: false,
        ask_to_save: false,
        css_background_scan: false,
        backlight_menu_item: BacklightMenuItem::Other,
        backlight_time: 3,
    }
}

struct Runtime<'a> {
    state: &'a mut State,
    context: Context,
    next_key: Option<KeyEvent>,
    timer_effects: Vec<Effects>,
}

impl ApplicationEffects for Runtime<'_> {
    fn update(&mut self) {}

    fn timeslice_10ms(&mut self, _deliver_tick: &mut dyn FnMut()) {
        if let Some(event) = self.next_key.take() {
            self.state.on_key_event(event, 2);
        }
    }

    fn timeslice_500ms(&mut self) {
        self.timer_effects
            .push(self.state.timeslice_500ms(self.context));
    }
}

#[test]
fn key_input_and_500ms_timeout_follow_the_live_dispatch_order() {
    let mut scheduler = AppSchedulerComposition::new();
    let mut state = State::new();
    let mut runtime = Runtime {
        state: &mut state,
        context: context(),
        next_key: Some(KeyEvent::Pressed(Key::Menu)),
        timer_effects: Vec::new(),
    };

    for _ in 0..50 {
        scheduler.tick();
    }
    scheduler.run_iteration(&mut runtime);

    assert_eq!(scheduler.counter(), 50);
    assert!(runtime.state.screen_is_menu());
    assert_eq!(runtime.timer_effects.len(), 1);
    assert!(!runtime.timer_effects[0].exit_menu);

    for _ in 0..50 {
        scheduler.tick();
    }
    scheduler.run_iteration(&mut runtime);

    assert_eq!(scheduler.counter(), 100);
    assert!(!runtime.state.screen_is_menu());
    assert!(runtime.timer_effects[1].exit_menu);
    assert!(runtime.state.update_display());
}
