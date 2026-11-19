#[allow(unused_attributes)]
#[path = "../src/periodic_scheduler_flags.rs"]
mod periodic_scheduler_flags;

use periodic_scheduler_flags::PeriodicSchedulerFlags;

#[test]
fn first_tick_sets_only_the_10ms_flag() {
    let mut scheduler = PeriodicSchedulerFlags::new();
    scheduler.tick();

    assert_eq!(scheduler.counter(), 1);
    assert!(scheduler.pending_10ms());
    assert!(!scheduler.pending_40ms());
    assert!(!scheduler.pending_500ms());
}

#[test]
fn boundary_40ms_sets_10ms_and_40ms_flags() {
    let mut scheduler = PeriodicSchedulerFlags::new();
    for _ in 0..4 {
        scheduler.tick();
    }

    assert_eq!(scheduler.counter(), 4);
    assert!(scheduler.pending_10ms());
    assert!(scheduler.pending_40ms());
    assert!(!scheduler.pending_500ms());
}

#[test]
fn boundary_500ms_sets_10ms_and_500ms_flags() {
    let mut scheduler = PeriodicSchedulerFlags::with_counter(49);
    scheduler.tick();

    assert_eq!(scheduler.counter(), 50);
    assert!(scheduler.pending_10ms());
    assert!(!scheduler.pending_40ms());
    assert!(scheduler.pending_500ms());
}

#[test]
fn shared_boundary_sets_all_flags() {
    let mut scheduler = PeriodicSchedulerFlags::new();
    for _ in 0..100 {
        scheduler.tick();
    }

    assert_eq!(scheduler.counter(), 100);
    assert!(scheduler.pending_10ms());
    assert!(scheduler.pending_40ms());
    assert!(scheduler.pending_500ms());
}

#[test]
fn pending_flags_coalesce_and_survive_non_boundary_ticks() {
    let mut scheduler = PeriodicSchedulerFlags::new();
    for _ in 0..4 {
        scheduler.tick();
    }
    for _ in 0..45 {
        scheduler.tick();
    }

    assert_eq!(scheduler.counter(), 49);
    assert!(scheduler.pending_10ms());
    assert!(scheduler.pending_40ms());
    assert!(!scheduler.pending_500ms());

    scheduler.tick();
    scheduler.tick();
    assert_eq!(scheduler.counter(), 51);
    assert!(scheduler.pending_10ms());
    assert!(scheduler.pending_40ms());
    assert!(scheduler.pending_500ms());
}

#[test]
fn acknowledgements_clear_only_their_own_flags() {
    let mut scheduler = PeriodicSchedulerFlags::new();
    for _ in 0..100 {
        scheduler.tick();
    }

    scheduler.acknowledge_40ms();
    assert!(scheduler.pending_10ms());
    assert!(!scheduler.pending_40ms());
    assert!(scheduler.pending_500ms());

    scheduler.acknowledge_10ms();
    assert!(!scheduler.pending_10ms());
    assert!(!scheduler.pending_40ms());
    assert!(scheduler.pending_500ms());

    scheduler.acknowledge_500ms();
    assert!(!scheduler.pending_10ms());
    assert!(!scheduler.pending_40ms());
    assert!(!scheduler.pending_500ms());
}

#[test]
fn wrapping_to_zero_sets_periodic_flags_then_ordinary_tick_does_not() {
    let mut scheduler = PeriodicSchedulerFlags::with_counter(u32::MAX);
    scheduler.tick();

    assert_eq!(scheduler.counter(), 0);
    assert!(scheduler.pending_10ms());
    assert!(scheduler.pending_40ms());
    assert!(scheduler.pending_500ms());

    scheduler.acknowledge_10ms();
    scheduler.acknowledge_40ms();
    scheduler.acknowledge_500ms();
    scheduler.tick();

    assert_eq!(scheduler.counter(), 1);
    assert!(scheduler.pending_10ms());
    assert!(!scheduler.pending_40ms());
    assert!(!scheduler.pending_500ms());
}
