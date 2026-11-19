#[path = "../src/app_scheduler_composition.rs"]
mod app_scheduler_composition;

use app_scheduler_composition::{AppSchedulerComposition, ApplicationEffects};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Update,
    Timeslice10ms,
    Timeslice500ms,
}

#[derive(Default)]
struct MockEffects {
    events: Vec<Event>,
    inject_tick_during_10ms: bool,
}

impl ApplicationEffects for MockEffects {
    fn update(&mut self) {
        self.events.push(Event::Update);
    }

    fn timeslice_10ms(&mut self, deliver_tick: &mut dyn FnMut()) {
        self.events.push(Event::Timeslice10ms);
        if self.inject_tick_during_10ms {
            self.inject_tick_during_10ms = false;
            deliver_tick();
        }
    }

    fn timeslice_500ms(&mut self) {
        self.events.push(Event::Timeslice500ms);
    }
}

#[test]
fn update_runs_when_idle_without_dispatching_a_timeslice() {
    let mut composition = AppSchedulerComposition::new();
    let mut effects = MockEffects::default();

    composition.run_iteration(&mut effects);

    assert_eq!(effects.events, [Event::Update]);
    assert!(!composition.pending_10ms());
    assert!(!composition.pending_500ms());
}

#[test]
fn one_pending_tick_dispatches_once_and_a_second_iteration_does_not_redispatch() {
    let mut composition = AppSchedulerComposition::new();
    let mut effects = MockEffects::default();
    composition.tick();

    composition.run_iteration(&mut effects);
    assert_eq!(effects.events, [Event::Update, Event::Timeslice10ms]);
    assert!(!composition.pending_10ms());

    effects.events.clear();
    composition.run_iteration(&mut effects);
    assert_eq!(effects.events, [Event::Update]);
}

#[test]
fn fiftieth_tick_does_not_raise_the_40ms_flag() {
    let mut composition = AppSchedulerComposition::new();
    let mut effects = MockEffects::default();

    for _ in 0..49 {
        composition.tick();
    }
    assert_eq!(composition.counter(), 49);
    assert!(composition.pending_10ms());
    assert!(composition.pending_40ms());
    assert!(!composition.pending_500ms());

    composition.run_iteration(&mut effects);
    assert_eq!(effects.events, [Event::Update, Event::Timeslice10ms]);
    assert!(!composition.pending_10ms());
    assert!(composition.pending_40ms());
    composition.acknowledge_40ms();

    composition.tick();
    assert_eq!(composition.counter(), 50);
    assert!(composition.pending_10ms());
    assert!(!composition.pending_40ms());
    assert!(composition.pending_500ms());

    effects.events.clear();
    composition.run_iteration(&mut effects);
    assert_eq!(
        effects.events,
        [Event::Update, Event::Timeslice10ms, Event::Timeslice500ms]
    );
    assert!(!composition.pending_10ms());
    assert!(!composition.pending_40ms());
    assert!(!composition.pending_500ms());
}

#[test]
fn shared_boundary_dispatch_leaves_the_new_40ms_flag_pending() {
    let mut composition = AppSchedulerComposition::new();
    let mut effects = MockEffects::default();
    for _ in 0..99 {
        composition.tick();
    }
    composition.run_iteration(&mut effects);
    composition.acknowledge_40ms();
    assert!(!composition.pending_40ms());
    assert!(!composition.pending_500ms());

    effects.events.clear();
    composition.tick();
    assert_eq!(composition.counter(), 100);
    assert!(composition.pending_40ms());
    assert!(composition.pending_500ms());

    composition.run_iteration(&mut effects);
    assert_eq!(
        effects.events,
        [Event::Update, Event::Timeslice10ms, Event::Timeslice500ms]
    );
    assert!(!composition.pending_10ms());
    assert!(composition.pending_40ms());
    assert!(!composition.pending_500ms());

    composition.acknowledge_40ms();
    assert!(!composition.pending_40ms());
}

#[test]
fn multiple_unserviced_ticks_coalesce_into_one_dispatch() {
    let mut composition = AppSchedulerComposition::new();
    let mut effects = MockEffects::default();
    for _ in 0..50 {
        composition.tick();
    }

    composition.run_iteration(&mut effects);

    assert_eq!(composition.counter(), 50);
    assert_eq!(
        effects.events,
        [Event::Update, Event::Timeslice10ms, Event::Timeslice500ms]
    );
    assert!(!composition.pending_10ms());
    assert!(composition.pending_40ms());
    assert!(!composition.pending_500ms());
}

#[test]
fn a_tick_during_the_10ms_effect_is_seen_by_the_later_500ms_query() {
    let mut composition = AppSchedulerComposition::new();
    let mut effects = MockEffects {
        inject_tick_during_10ms: true,
        ..MockEffects::default()
    };
    for _ in 0..49 {
        composition.tick();
    }

    composition.run_iteration(&mut effects);

    assert_eq!(composition.counter(), 50);
    assert_eq!(
        effects.events,
        [Event::Update, Event::Timeslice10ms, Event::Timeslice500ms]
    );
    assert!(composition.pending_10ms());
    assert!(composition.pending_40ms());
    assert!(!composition.pending_500ms());
}
