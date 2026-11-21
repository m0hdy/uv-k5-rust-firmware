use uv_k5_rust_firmware::scheduler::dispatch::{Backend, run};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Update,
    Query10ms(bool),
    Timeslice10ms,
    Query500ms(bool),
    Timeslice500ms,
}

struct FakeBackend {
    pending_10ms: bool,
    pending_500ms: bool,
    after_update: Option<(bool, bool)>,
    after_10ms: Option<bool>,
    events: Vec<Event>,
}

impl FakeBackend {
    fn new(pending_10ms: bool, pending_500ms: bool) -> Self {
        Self {
            pending_10ms,
            pending_500ms,
            after_update: None,
            after_10ms: None,
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
        self.events.push(Event::Query10ms(pending));
        self.pending_10ms = false;
        pending
    }

    fn timeslice_10ms(&mut self) {
        self.events.push(Event::Timeslice10ms);
        if let Some(pending_500ms) = self.after_10ms {
            self.pending_500ms = pending_500ms;
        }
    }

    fn take_500ms_pending(&mut self) -> bool {
        let pending = self.pending_500ms;
        self.events.push(Event::Query500ms(pending));
        self.pending_500ms = false;
        pending
    }

    fn timeslice_500ms(&mut self) {
        self.events.push(Event::Timeslice500ms);
    }
}

#[test]
fn all_flag_combinations_have_the_exact_query_and_call_order() {
    use Event::*;
    let cases: [(bool, bool, &[Event]); 4] = [
        (false, false, &[Update, Query10ms(false)]),
        (false, true, &[Update, Query10ms(false)]),
        (
            true,
            false,
            &[Update, Query10ms(true), Timeslice10ms, Query500ms(false)],
        ),
        (
            true,
            true,
            &[
                Update,
                Query10ms(true),
                Timeslice10ms,
                Query500ms(true),
                Timeslice500ms,
            ],
        ),
    ];

    for (pending_10ms, pending_500ms, expected) in cases {
        let mut backend = FakeBackend::new(pending_10ms, pending_500ms);
        run(&mut backend);
        assert_eq!(backend.events, expected, "{pending_10ms} {pending_500ms}");
        assert_eq!(backend.pending_10ms, pending_10ms);
        assert_eq!(backend.pending_500ms, pending_500ms);
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
            Query10ms(true),
            Timeslice10ms,
            Query500ms(true),
            Timeslice500ms,
        ]
    );

    let mut backend = FakeBackend::new(true, true);
    backend.after_update = Some((false, false));
    run(&mut backend);
    assert_eq!(backend.events, [Update, Query10ms(false)]);
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
            Query10ms(true),
            Timeslice10ms,
            Query500ms(true),
            Timeslice500ms,
        ]
    );

    let mut backend = FakeBackend::new(true, true);
    backend.after_10ms = Some(false);
    run(&mut backend);
    assert_eq!(
        backend.events,
        [Update, Query10ms(true), Timeslice10ms, Query500ms(false)]
    );
}
