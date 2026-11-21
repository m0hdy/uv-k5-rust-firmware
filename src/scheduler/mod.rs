//! Offline periodic state, iteration order, and application effects.

pub mod composition;
pub mod dispatch;
pub mod flags;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod interrupt;
