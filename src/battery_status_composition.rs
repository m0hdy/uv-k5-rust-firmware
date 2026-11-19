#![cfg_attr(not(test), no_std)]

//! Offline composition of battery status render operations.

#[allow(unused_attributes)]
#[path = "battery_indicator.rs"]
mod battery_indicator;
#[allow(unused_attributes)]
#[path = "battery_status_text.rs"]
mod battery_status_text;

pub use battery_indicator::FillDirection;
use battery_indicator::compose_battery_indicator;
pub use battery_status_text::BatteryStatusText;
use battery_status_text::battery_status_text;

use battery_indicator::BATTERY_BAR_WIDTH;
pub use battery_indicator::BATTERY_INDICATOR_WIDTH as BATTERY_ICON_WIDTH;
use battery_status_text::LCD_WIDTH;
pub use battery_status_text::USB_ICON_WIDTH;

const BATTERY_ICON_COLUMN: usize = LCD_WIDTH - BATTERY_ICON_WIDTH;
const USB_ICON_COLUMN: usize = BATTERY_ICON_COLUMN - USB_ICON_WIDTH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderOperation {
    Text(BatteryStatusText),
    UsbIcon {
        start_column: usize,
        bytes: [u8; USB_ICON_WIDTH],
    },
    BatteryBitmap {
        start_column: usize,
        bytes: [u8; BATTERY_ICON_WIDTH],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryStatusComposition {
    operations: [Option<RenderOperation>; 3],
    len: usize,
}

impl BatteryStatusComposition {
    pub fn operations(&self) -> impl Iterator<Item = &RenderOperation> {
        self.operations[..self.len]
            .iter()
            .filter_map(Option::as_ref)
    }
}

pub struct StatusInputs {
    pub mode: u8,
    pub voltage_hundredths: u16,
    pub percentage: u8,
    pub occupied_prefix_end: usize,
    pub charging: bool,
    pub level: u8,
    pub blink: u8,
    pub direction: FillDirection,
}

pub struct IconPatterns<'a> {
    pub usb: &'a [u8; USB_ICON_WIDTH],
    pub outline: &'a [u8; BATTERY_ICON_WIDTH],
    pub bar: &'a [u8; BATTERY_BAR_WIDTH],
}

pub fn compose_battery_status(
    status: StatusInputs,
    icons: IconPatterns<'_>,
) -> BatteryStatusComposition {
    let mut operations = [None; 3];
    let mut len = 0;

    if let Some(text) = battery_status_text(
        status.mode,
        status.voltage_hundredths,
        status.percentage,
        status.occupied_prefix_end,
        status.charging,
    ) {
        operations[len] = Some(RenderOperation::Text(text));
        len += 1;
    }

    if status.charging {
        operations[len] = Some(RenderOperation::UsbIcon {
            start_column: USB_ICON_COLUMN,
            bytes: *icons.usb,
        });
        len += 1;
    }

    let mut bitmap = [0; BATTERY_ICON_WIDTH];
    compose_battery_indicator(
        &mut bitmap,
        icons.outline,
        icons.bar,
        status.level,
        status.blink,
        status.direction,
    );
    operations[len] = Some(RenderOperation::BatteryBitmap {
        start_column: BATTERY_ICON_COLUMN,
        bytes: bitmap,
    });
    len += 1;

    BatteryStatusComposition { operations, len }
}
