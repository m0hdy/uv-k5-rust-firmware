#![cfg_attr(not(test), no_std)]

//! Offline model of the pinned board battery ADC acquisition sequence.
//!
//! The sequence follows `BOARD_ADC_GetBatteryInfo()` in `board.c` and
//! `ADC_Start()`, `ADC_CheckEndOfConversion()`, and `ADC_GetValue()` in
//! `driver/adc.c`, using the DP32G030 SARADC definitions in `saradc.h` at
//! [`7607f0a`](https://github.com/egzumer/uv-k5-firmware-custom/tree/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3).
//! It has no MMIO implementation and is not connected to firmware startup.

#[path = "adc_samples.rs"]
#[allow(dead_code)]
pub(crate) mod adc_samples;

use adc_samples::{AdcRegisterSnapshot, decode_adc_sample};

const SARADC_START_START_MASK: u32 = 1;
const SARADC_IF_CHANNEL_4: u32 = 1 << 4;
const SARADC_IF_CHANNEL_9: u32 = 1 << 9;
const ADC_CHANNEL_4_MASK: u32 = 1 << 4;
const ADC_CHANNEL_9_MASK: u32 = 1 << 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadRegister {
    Start,
    Channel9Status,
    Channel4Data,
    Channel9Data,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteRegister {
    Start,
    InterruptFlags,
}

pub trait RegisterIo {
    fn read(&mut self, register: ReadRegister) -> u32;
    fn write(&mut self, register: WriteRegister, value: u32);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatterySamples {
    pub voltage: u16,
    pub current: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcquisitionError {
    PollBudgetExhausted,
}

fn read_sample(
    io: &mut impl RegisterIo,
    channel_mask: u32,
    interrupt_flag: u32,
    data: ReadRegister,
) -> u16 {
    io.write(WriteRegister::InterruptFlags, interrupt_flag);
    let data = io.read(data);
    decode_adc_sample(channel_mask, AdcRegisterSnapshot { status: 0, data }).value
}

pub fn acquire_battery_samples(
    io: &mut impl RegisterIo,
    poll_budget: usize,
) -> Result<BatterySamples, AcquisitionError> {
    if poll_budget == 0 {
        return Err(AcquisitionError::PollBudgetExhausted);
    }

    let start = io.read(ReadRegister::Start);
    io.write(
        WriteRegister::Start,
        (start & !SARADC_START_START_MASK) | SARADC_START_START_MASK,
    );

    let ready = (0..poll_budget).any(|_| {
        let status = io.read(ReadRegister::Channel9Status);
        decode_adc_sample(ADC_CHANNEL_9_MASK, AdcRegisterSnapshot { status, data: 0 })
            .end_of_conversion
    });
    if !ready {
        return Err(AcquisitionError::PollBudgetExhausted);
    }

    let voltage = read_sample(
        io,
        ADC_CHANNEL_4_MASK,
        SARADC_IF_CHANNEL_4,
        ReadRegister::Channel4Data,
    );
    let current = read_sample(
        io,
        ADC_CHANNEL_9_MASK,
        SARADC_IF_CHANNEL_9,
        ReadRegister::Channel9Data,
    );

    Ok(BatterySamples { voltage, current })
}
