use uv_k5_rust_firmware::scheduler::dispatch::{Backend, run};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Update,
    Take10ms(bool),
    Timeslice10ms,
    Take500ms(bool),
    Timeslice500ms,
}

struct FakeBackend {
    pending_10ms: bool,
    pending_500ms: bool,
    after_update: Option<(bool, bool)>,
    after_10ms: Option<bool>,
    tick_during_10ms: bool,
    tick_during_500ms: bool,
    events: Vec<Event>,
}

impl FakeBackend {
    fn new(pending_10ms: bool, pending_500ms: bool) -> Self {
        Self {
            pending_10ms,
            pending_500ms,
            after_update: None,
            after_10ms: None,
            tick_during_10ms: false,
            tick_during_500ms: false,
            events: Vec::new(),
        }
    }
}

impl Backend for FakeBackend {
    fn update(&mut self) {
        self.events.push(Event::Update);
        if let Some((pending_10ms, pending_500ms)) = self.after_update {
            self.pending_10ms = pending_10ms;
            self.pending_500ms = pending_500ms;
        }
    }

    fn take_10ms_pending(&mut self) -> bool {
        let pending = self.pending_10ms;
        self.events.push(Event::Take10ms(pending));
        self.pending_10ms = false;
        pending
    }

    fn timeslice_10ms(&mut self) {
        self.events.push(Event::Timeslice10ms);
        if self.tick_during_10ms {
            self.pending_10ms = true;
        }
        if let Some(pending_500ms) = self.after_10ms {
            self.pending_500ms = pending_500ms;
        }
    }

    fn take_500ms_pending(&mut self) -> bool {
        let pending = self.pending_500ms;
        self.events.push(Event::Take500ms(pending));
        self.pending_500ms = false;
        pending
    }

    fn timeslice_500ms(&mut self) {
        self.events.push(Event::Timeslice500ms);
        if self.tick_during_500ms {
            self.pending_500ms = true;
        }
    }
}

#[test]
fn all_flag_combinations_have_the_exact_query_and_call_order() {
    use Event::*;
    let cases: [(bool, bool, &[Event]); 4] = [
        (false, false, &[Update, Take10ms(false)]),
        (false, true, &[Update, Take10ms(false)]),
        (
            true,
            false,
            &[Update, Take10ms(true), Timeslice10ms, Take500ms(false)],
        ),
        (
            true,
            true,
            &[
                Update,
                Take10ms(true),
                Timeslice10ms,
                Take500ms(true),
                Timeslice500ms,
            ],
        ),
    ];

    for (pending_10ms, pending_500ms, expected) in cases {
        let mut backend = FakeBackend::new(pending_10ms, pending_500ms);
        run(&mut backend);
        assert_eq!(backend.events, expected, "{pending_10ms} {pending_500ms}");
        assert!(!backend.pending_10ms);
        assert_eq!(backend.pending_500ms, pending_500ms && !pending_10ms);
    }
}

#[test]
fn update_changes_are_observed_before_either_flag_is_queried() {
    use Event::*;
    let mut backend = FakeBackend::new(false, false);
    backend.after_update = Some((true, true));
    run(&mut backend);
    assert_eq!(
        backend.events,
        [
            Update,
            Take10ms(true),
            Timeslice10ms,
            Take500ms(true),
            Timeslice500ms,
        ]
    );

    let mut backend = FakeBackend::new(true, true);
    backend.after_update = Some((false, false));
    run(&mut backend);
    assert_eq!(backend.events, [Update, Take10ms(false)]);
}

#[test]
fn ten_ms_hook_changes_the_later_500ms_query_in_both_directions() {
    use Event::*;
    let mut backend = FakeBackend::new(true, false);
    backend.after_10ms = Some(true);
    run(&mut backend);
    assert_eq!(
        backend.events,
        [
            Update,
            Take10ms(true),
            Timeslice10ms,
            Take500ms(true),
            Timeslice500ms,
        ]
    );

    let mut backend = FakeBackend::new(true, true);
    backend.after_10ms = Some(false);
    run(&mut backend);
    assert_eq!(
        backend.events,
        [Update, Take10ms(true), Timeslice10ms, Take500ms(false)]
    );
}

#[test]
fn new_flags_set_during_a_timeslice_survive_that_timeslice() {
    use Event::*;
    let mut backend = FakeBackend::new(true, false);
    backend.tick_during_10ms = true;
    run(&mut backend);
    assert_eq!(
        backend.events,
        [Update, Take10ms(true), Timeslice10ms, Take500ms(false)]
    );
    assert!(backend.pending_10ms);

    let mut backend = FakeBackend::new(true, true);
    backend.tick_during_500ms = true;
    run(&mut backend);
    assert!(backend.pending_500ms);
}
