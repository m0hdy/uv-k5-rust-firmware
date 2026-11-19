#[path = "../src/battery_status_composition.rs"]
mod battery_status_composition;

use battery_status_composition::{
    BatteryStatusComposition, BatteryStatusText, FillDirection, IconPatterns, RenderOperation,
    StatusInputs, compose_battery_status,
};

const OUTLINE: [u8; 17] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
    0x20,
];
const BAR: [u8; 2] = [0xa5, 0x5a];
const USB_ICON: [u8; 9] = [0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38];
const ICONS: IconPatterns<'static> = IconPatterns {
    usb: &USB_ICON,
    outline: &OUTLINE,
    bar: &BAR,
};

fn compose(
    mode: u8,
    occupied_prefix_end: usize,
    charging: bool,
    level: u8,
    blink: u8,
    direction: FillDirection,
) -> BatteryStatusComposition {
    compose_battery_status(
        StatusInputs {
            mode,
            voltage_hundredths: 725,
            percentage: 42,
            occupied_prefix_end,
            charging,
            level,
            blink,
            direction,
        },
        ICONS,
    )
}

#[test]
fn captures_text_usb_and_battery_operations_in_render_order() {
    let operations = compose(1, 0, true, 4, 0, FillDirection::Normal)
        .operations()
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        [
            RenderOperation::Text(BatteryStatusText {
                bytes: *b"7.25V",
                len: 5,
                start_column: 67,
            }),
            RenderOperation::UsbIcon {
                start_column: 102,
                bytes: USB_ICON,
            },
            RenderOperation::BatteryBitmap {
                start_column: 111,
                bytes: [
                    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0xa5, 0x5a, 0x1c,
                    0xa5, 0x5a, 0x1f, 0x20,
                ],
            },
        ]
    );
}

#[test]
fn text_shifts_nine_columns_only_when_charging() {
    let uncharged = compose(2, 0, false, 2, 0, FillDirection::Normal);
    let charged = compose(2, 0, true, 2, 0, FillDirection::Normal);
    let text_column = |composition: &BatteryStatusComposition| {
        composition
            .operations()
            .find_map(|operation| match operation {
                RenderOperation::Text(text) => Some(text.start_column),
                _ => None,
            })
    };

    assert_eq!(text_column(&uncharged), Some(90));
    assert_eq!(text_column(&charged), Some(81));
    assert_eq!(
        uncharged
            .operations()
            .find_map(|operation| match operation {
                RenderOperation::Text(text) => Some(text.as_bytes()),
                _ => None,
            }),
        Some(&b"42%"[..])
    );
    assert_eq!(
        uncharged
            .operations()
            .filter_map(|operation| match operation {
                RenderOperation::UsbIcon { .. } => Some(()),
                _ => None,
            })
            .count(),
        0
    );
    assert!(matches!(
        charged.operations().nth(1),
        Some(RenderOperation::UsbIcon {
            start_column: 102,
            bytes: USB_ICON,
        })
    ));
}

#[test]
fn text_omission_never_suppresses_other_operations() {
    for (mode, occupied_prefix_end, charging) in [(0, 0, false), (2, 100, true)] {
        let operations = compose(
            mode,
            occupied_prefix_end,
            charging,
            1,
            1,
            FillDirection::Normal,
        )
        .operations()
        .copied()
        .collect::<Vec<_>>();

        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation, RenderOperation::Text(_)))
                .count(),
            0
        );
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation, RenderOperation::UsbIcon { .. }))
                .count(),
            usize::from(charging)
        );
        match operations.last() {
            Some(RenderOperation::BatteryBitmap {
                start_column,
                bytes,
            }) => {
                assert_eq!(*start_column, 111);
                assert_eq!(*bytes, [0; 17]);
            }
            other => panic!("expected battery operation, got {other:?}"),
        }
    }
}

#[test]
fn non_charging_operations_keep_the_full_filled_bitmap() {
    let operations = compose(0, 0, false, 4, 0, FillDirection::Reversed)
        .operations()
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        [RenderOperation::BatteryBitmap {
            start_column: 111,
            bytes: [
                0x10, 0x11, 0x12, 0xa5, 0x5a, 0x15, 0xa5, 0x5a, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
                0x1e, 0x1f, 0x20,
            ],
        }]
    );
}
