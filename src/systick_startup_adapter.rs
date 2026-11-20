#![cfg_attr(not(test), no_std)]

//! Typed SysTick startup operations and a restricted Cortex-M0 adapter.

use crate::systick_timing;

pub const SYS_TICK_PERIOD_TICKS: u32 = systick_timing::SYS_TICK_PERIOD_TICKS;
pub const SYS_TICK_DELAY_MULTIPLIER: u32 = systick_timing::SYS_TICK_MULTIPLIER;

const SHPR3_SYSTICK_PRIORITY_MASK: u32 = 0xff00_0000;
const SHPR3_SYSTICK_LOWEST_PRIORITY: u32 = 0xc000_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterOperation {
    WriteSysTickLoad(u32),
    UpdateSysTickPriorityWord { mask: u32, value: u32 },
    WriteSysTickValue(u32),
    WriteSysTickControl(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupSequence {
    pub operations: [RegisterOperation; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidReload;

pub const fn merge_priority_word(current: u32, mask: u32, value: u32) -> u32 {
    (current & !mask) | (value & mask)
}

pub fn startup_sequence(period_ticks: u32) -> Result<StartupSequence, InvalidReload> {
    let reload = systick_timing::reload_value(period_ticks).ok_or(InvalidReload)?;

    Ok(StartupSequence {
        operations: [
            RegisterOperation::WriteSysTickLoad(reload),
            RegisterOperation::UpdateSysTickPriorityWord {
                mask: SHPR3_SYSTICK_PRIORITY_MASK,
                value: SHPR3_SYSTICK_LOWEST_PRIORITY,
            },
            RegisterOperation::WriteSysTickValue(0),
            RegisterOperation::WriteSysTickControl(0b111),
        ],
    })
}

pub fn apply_with(
    period_ticks: u32,
    mut operation: impl FnMut(RegisterOperation),
) -> Result<(), InvalidReload> {
    let sequence = startup_sequence(period_ticks)?;
    for register_operation in sequence.operations {
        operation(register_operation);
    }
    Ok(())
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
/// Applies the startup sequence to the Cortex-M0 SysTick and SCB registers.
///
/// # Safety
///
/// The caller must ensure the target uses the DP32G030/Cortex-M0 core-register
/// mapping, this runs in exclusive early-startup context, and a valid SysTick
/// handler and vector are installed before the sequence enables interrupts.
pub unsafe fn apply_to_hardware(period_ticks: u32) -> Result<(), InvalidReload> {
    apply_with(period_ticks, |operation| unsafe {
        match operation {
            RegisterOperation::WriteSysTickLoad(value) => {
                core::ptr::write_volatile(0xe000_e014 as *mut u32, value);
            }
            RegisterOperation::UpdateSysTickPriorityWord { mask, value } => {
                let priority = 0xe000_ed20 as *mut u32;
                let current = core::ptr::read_volatile(priority);
                core::ptr::write_volatile(priority, merge_priority_word(current, mask, value));
            }
            RegisterOperation::WriteSysTickValue(value) => {
                core::ptr::write_volatile(0xe000_e018 as *mut u32, value);
            }
            RegisterOperation::WriteSysTickControl(value) => {
                core::ptr::write_volatile(0xe000_e010 as *mut u32, value);
            }
        }
    })
}
