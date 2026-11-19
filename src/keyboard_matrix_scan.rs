#![cfg_attr(not(test), no_std)]

//! Offline GPIO trace model of the pinned keyboard matrix poll.

/// Sampled keys, with the numeric codes from the pinned keyboard header.
/// PTT is deliberately absent; no key is represented by `None`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Key {
    Digit0 = 0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Menu,
    Up,
    Down,
    Exit,
    Star,
    F,
    Side2 = 17,
    Side1,
}

/// Infallible operations supplied by an offline mock, not a hardware driver.
pub trait MatrixIo {
    /// One GPIOA DATA read-modify-write: `(old & and_mask) | or_mask`.
    fn update_gpioa(&mut self, and_mask: u32, or_mask: u32);
    fn read_gpioa(&mut self) -> u32;
    fn delay_1us(&mut self);
    /// Opaque I2C STOP operation, including its own pin changes and delays.
    fn i2c_stop(&mut self);
}

const ROWS: [(u16, [Option<Key>; 4]); 5] = [
    (0xffff, [Some(Key::Side1), Some(Key::Side2), None, None]),
    (
        0xfbff,
        [
            Some(Key::Menu),
            Some(Key::Digit1),
            Some(Key::Digit4),
            Some(Key::Digit7),
        ],
    ),
    (
        0xf7ff,
        [
            Some(Key::Up),
            Some(Key::Digit2),
            Some(Key::Digit5),
            Some(Key::Digit8),
        ],
    ),
    (
        0xefff,
        [
            Some(Key::Down),
            Some(Key::Digit3),
            Some(Key::Digit6),
            Some(Key::Digit9),
        ],
    ),
    (
        0xdfff,
        [
            Some(Key::Exit),
            Some(Key::Star),
            Some(Key::Digit0),
            Some(Key::F),
        ],
    ),
];

/// Scans side keys, then four matrix rows, with first-low-column priority.
/// Every normal return performs STOP, voice-pin clear, then voice-pin set.
pub fn poll(io: &mut impl MatrixIo) -> Option<Key> {
    let key = scan(io);
    io.i2c_stop();
    io.update_gpioa(0xffff_efff, 0);
    io.update_gpioa(u32::MAX, 0x2000);
    key
}

fn scan(io: &mut impl MatrixIo) -> Option<Key> {
    for (select_mask, keys) in ROWS {
        io.update_gpioa(u32::MAX, 0x3c00);
        io.update_gpioa(u32::from(select_mask), 0);

        let mut previous = 0u16;
        let mut stable = 0;
        for _ in 0..8 {
            io.delay_1us();
            let reading = io.read_gpioa() as u16;
            stable = if reading == previous { stable + 1 } else { 1 };
            previous = reading;
            if stable == 3 {
                break;
            }
        }
        if stable < 3 {
            return None;
        }

        // The zero row's two padding entries both use PA4, not PA5/PA6.
        let pins = if select_mask == 0xffff {
            [3, 4, 4, 4]
        } else {
            [3, 4, 5, 6]
        };
        for (pin, key) in pins.into_iter().zip(keys) {
            if previous & (1 << pin) == 0 {
                if key.is_some() {
                    return key;
                }
                break;
            }
        }
    }
    None
}
