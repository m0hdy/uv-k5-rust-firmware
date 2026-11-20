use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use uv_k5_rust_firmware::{
    i2c_bitbang_flow::target_gpio::Delay1us,
    st7565_startup_executor::target_spi0::DelayMs,
    systick_delay_adapter::{SysTickCounter, SysTickDelay, delay_ms_target_ticks},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Read {
    Load(u32),
    Value(u32),
}

struct ScriptedCounter {
    load: u32,
    values: VecDeque<u32>,
    reads: Rc<RefCell<Vec<Read>>>,
}

impl ScriptedCounter {
    fn new(load: u32, values: impl IntoIterator<Item = u32>) -> (Self, Rc<RefCell<Vec<Read>>>) {
        let reads = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                load,
                values: values.into_iter().collect(),
                reads: Rc::clone(&reads),
            },
            reads,
        )
    }
}

impl SysTickCounter for ScriptedCounter {
    fn read_load(&mut self) -> u32 {
        self.reads.borrow_mut().push(Read::Load(self.load));
        self.load
    }

    fn read_value(&mut self) -> u32 {
        let value = self.values.pop_front().expect("scripted counter value");
        self.reads.borrow_mut().push(Read::Value(value));
        value
    }
}

#[test]
fn delay_1us_polls_repeated_samples_and_accumulates_decrements() {
    let (counter, reads) = ScriptedCounter::new(9, [5, 5, 4, 4, 3]);
    let mut delay = SysTickDelay::new(counter, std::num::NonZeroU32::new(2).unwrap());

    Delay1us::delay_1us(&mut delay);

    assert_eq!(
        reads.borrow().as_slice(),
        &[
            Read::Load(9),
            Read::Value(5),
            Read::Value(5),
            Read::Value(4),
            Read::Value(4),
            Read::Value(3),
        ]
    );
}

#[test]
fn each_delay_takes_a_fresh_load_then_value_snapshot() {
    let (counter, reads) = ScriptedCounter::new(9, [4, 3, 2, 1]);
    let mut delay = SysTickDelay::new(counter, std::num::NonZeroU32::new(1).unwrap());

    Delay1us::delay_1us(&mut delay);
    Delay1us::delay_1us(&mut delay);

    assert_eq!(
        reads.borrow().as_slice(),
        &[
            Read::Load(9),
            Read::Value(4),
            Read::Value(3),
            Read::Load(9),
            Read::Value(2),
            Read::Value(1),
        ]
    );
}

#[test]
fn delay_1us_uses_non_48_calibration_for_polling_duration() {
    let (counter, reads) = ScriptedCounter::new(9, [7, 6, 5, 4]);
    let mut delay = SysTickDelay::new(counter, std::num::NonZeroU32::new(3).unwrap());

    Delay1us::delay_1us(&mut delay);

    assert_eq!(
        reads.borrow().as_slice(),
        &[
            Read::Load(9),
            Read::Value(7),
            Read::Value(6),
            Read::Value(5),
            Read::Value(4),
        ]
    );
}

#[test]
fn delay_1us_preserves_the_pinned_one_tick_short_wrap_sample() {
    let (counter, reads) = ScriptedCounter::new(9, [1, 9, 8]);
    let mut delay = SysTickDelay::new(counter, std::num::NonZeroU32::new(2).unwrap());

    Delay1us::delay_1us(&mut delay);

    assert_eq!(
        reads.borrow().as_slice(),
        &[
            Read::Load(9),
            Read::Value(1),
            Read::Value(9),
            Read::Value(8),
        ]
    );
}

#[test]
fn delay_ms_waits_for_a_changed_sample_even_when_target_is_zero() {
    let (counter, reads) = ScriptedCounter::new(9, [3, 3, 2]);
    let mut delay = SysTickDelay::new(counter, std::num::NonZeroU32::new(48).unwrap());

    DelayMs::delay_ms(&mut delay, 0);

    assert_eq!(
        reads.borrow().as_slice(),
        &[
            Read::Load(9),
            Read::Value(3),
            Read::Value(3),
            Read::Value(2)
        ]
    );
}

#[test]
fn delay_ms_waits_for_a_changed_sample_when_target_wraps_to_zero() {
    let (counter, reads) = ScriptedCounter::new(9, [3, 2]);
    let mut delay = SysTickDelay::new(counter, std::num::NonZeroU32::new(0x2000_0000).unwrap());

    DelayMs::delay_ms(&mut delay, 1);

    assert_eq!(
        reads.borrow().as_slice(),
        &[Read::Load(9), Read::Value(3), Read::Value(2)]
    );
}

#[test]
fn delay_ms_waits_for_requested_milliseconds_at_the_selected_tick_rate() {
    let values = std::iter::once(2_000).chain((1..=1_000).map(|elapsed| 2_000 - elapsed));
    let (counter, reads) = ScriptedCounter::new(4_095, values);
    let mut delay = SysTickDelay::new(counter, std::num::NonZeroU32::new(1).unwrap());

    DelayMs::delay_ms(&mut delay, 1);

    assert_eq!(reads.borrow().len(), 1 + 1_001);
    assert_eq!(reads.borrow().first(), Some(&Read::Load(4_095)));
}

#[test]
fn millisecond_conversion_preserves_both_u32_wrapping_multiplications() {
    let ticks_per_microsecond = std::num::NonZeroU32::new(48).unwrap();
    assert_eq!(delay_ms_target_ticks(2, ticks_per_microsecond), 96_000);
    assert_eq!(
        delay_ms_target_ticks(u32::MAX, ticks_per_microsecond),
        4_294_919_296
    );
}
