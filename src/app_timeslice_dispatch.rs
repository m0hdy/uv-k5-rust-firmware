#![cfg_attr(not(test), no_std)]

//! Offline dispatch order from pinned
//! [`main.c:222–233`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L222-L233).
//! The backend owns flags and effects; this module only orders calls and queries.

/// Operations and current flags used by one main-loop iteration.
pub trait Backend {
    fn update(&mut self);
    fn is_10ms_pending(&mut self) -> bool;
    fn timeslice_10ms(&mut self);
    fn is_500ms_pending(&mut self) -> bool;
    fn timeslice_500ms(&mut self);
}

/// Dispatches one iteration without managing flags or elapsed time.
pub fn run(backend: &mut impl Backend) {
    backend.update();
    if backend.is_10ms_pending() {
        backend.timeslice_10ms();
        if backend.is_500ms_pending() {
            backend.timeslice_500ms();
        }
    }
}
