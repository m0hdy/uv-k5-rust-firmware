//! Offline dispatch order from pinned
//! [`main.c:222–233`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/main.c#L222-L233).
//! The backend atomically consumes pending flags and owns effects; this module
//! only orders calls and checks.

/// Operations and current flags used by one main-loop iteration.
pub trait Backend {
    fn update(&mut self);
    /// Atomically checks and consumes the pending 10 ms flag.
    fn take_10ms_pending(&mut self) -> bool;
    fn timeslice_10ms(&mut self);
    /// Atomically checks and consumes the pending 500 ms flag.
    fn take_500ms_pending(&mut self) -> bool;
    fn timeslice_500ms(&mut self);
}

/// Dispatches one iteration without managing flags or elapsed time.
pub fn run(backend: &mut impl Backend) {
    backend.update();
    if backend.take_10ms_pending() {
        backend.timeslice_10ms();
        if backend.take_500ms_pending() {
            backend.timeslice_500ms();
        }
    }
}
