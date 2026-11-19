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

pub const USB_ICON_COLUMN: usize = 102;
pub const BATTERY_ICON_COLUMN: usize = 111;
pub const USB_ICON_WIDTH: usize = 9;
pub const BATTERY_ICON_WIDTH: usize = 17;

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

pub fn compose_battery_status(
    mode: u8,
    voltage_hundredths: u16,
    percentage: u8,
    occupied_prefix_end: usize,
    charging: bool,
    usb_icon: &[u8; USB_ICON_WIDTH],
    outline: &[u8; BATTERY_ICON_WIDTH],
    bar: &[u8; 2],
    level: u8,
    blink: u8,
    direction: FillDirection,
) -> BatteryStatusComposition {
    let mut operations = [None; 3];
    let mut len = 0;

    if let Some(text) = battery_status_text(
        mode,
        voltage_hundredths,
        percentage,
        occupied_prefix_end,
        charging,
    ) {
        operations[len] = Some(RenderOperation::Text(text));
        len += 1;
    }

    if charging {
        operations[len] = Some(RenderOperation::UsbIcon {
            start_column: USB_ICON_COLUMN,
            bytes: *usb_icon,
        });
        len += 1;
    }

    let mut bitmap = [0; BATTERY_ICON_WIDTH];
    compose_battery_indicator(&mut bitmap, outline, bar, level, blink, direction);
    operations[len] = Some(RenderOperation::BatteryBitmap {
        start_column: BATTERY_ICON_COLUMN,
        bytes: bitmap,
    });
    len += 1;

    BatteryStatusComposition { operations, len }
}
