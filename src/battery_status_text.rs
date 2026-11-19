#![cfg_attr(not(test), no_std)]

//! Offline formatting and placement of the battery status text.

pub const LCD_WIDTH: usize = 128;
pub const BATTERY_ICON_WIDTH: usize = 17;
pub const USB_ICON_WIDTH: usize = 9;
pub const PIXELS_PER_BYTE: usize = 7;
pub const MAX_BATTERY_STATUS_TEXT_BYTES: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryStatusText {
    pub bytes: [u8; MAX_BATTERY_STATUS_TEXT_BYTES],
    pub len: usize,
    pub start_column: usize,
}

impl BatteryStatusText {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Formats battery status text and returns its start column when it fits.
///
/// `percentage` must already be converted from voltage and be in `0..=100`.
/// Voltage is supplied in hundredths of a volt.
pub fn battery_status_text(
    mode: u8,
    voltage_hundredths: u16,
    percentage: u8,
    occupied_prefix_end: usize,
    charging: bool,
) -> Option<BatteryStatusText> {
    let mut bytes = [0; MAX_BATTERY_STATUS_TEXT_BYTES];
    let len = match mode {
        1 => {
            let voltage = core::cmp::min(voltage_hundredths, 999);
            bytes[0] = b'0' + (voltage / 100) as u8;
            bytes[1] = b'.';
            bytes[2] = b'0' + ((voltage / 10) % 10) as u8;
            bytes[3] = b'0' + (voltage % 10) as u8;
            bytes[4] = b'V';
            5
        }
        2 => {
            if percentage >= 100 {
                bytes[..3].copy_from_slice(b"100");
                bytes[3] = b'%';
                4
            } else if percentage >= 10 {
                bytes[0] = b'0' + percentage / 10;
                bytes[1] = b'0' + percentage % 10;
                bytes[2] = b'%';
                3
            } else {
                bytes[0] = b'0' + percentage;
                bytes[1] = b'%';
                2
            }
        }
        _ => return None,
    };

    let reserved_icons = BATTERY_ICON_WIDTH + if charging { USB_ICON_WIDTH } else { 0 };
    let text_right_edge = LCD_WIDTH - reserved_icons;
    let text_width = len * PIXELS_PER_BYTE;
    if occupied_prefix_end > text_right_edge || text_width > text_right_edge - occupied_prefix_end {
        return None;
    }

    Some(BatteryStatusText {
        bytes,
        len,
        start_column: text_right_edge - text_width,
    })
}
