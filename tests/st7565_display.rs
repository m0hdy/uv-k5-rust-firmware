#[path = "../src/st7565_display.rs"]
mod st7565_display;

use st7565_display::{CommandStep, DisplayBlitPlan, FRAME_LINES, LCD_WIDTH};

fn flattened(plan: DisplayBlitPlan<'_>) -> Vec<(bool, u8)> {
    let mut trace = Vec::new();
    for step in plan.steps() {
        match step {
            CommandStep::Command(byte) => trace.push((false, byte)),
            CommandStep::Data(payload) => {
                trace.extend(payload.iter().copied().map(|byte| (true, byte)));
            }
        }
    }
    trace
}

#[test]
fn status_line_trace_uses_page_zero_and_preserves_payload_order() {
    let payload: Vec<u8> = (0..LCD_WIDTH as u8).collect();
    let trace = flattened(DisplayBlitPlan::status_line(&payload).unwrap());

    assert_eq!(
        &trace[..4],
        &[(false, 0x40), (false, 0xb0), (false, 0x10), (false, 0x04)]
    );
    assert_eq!(
        trace[4..],
        payload
            .iter()
            .copied()
            .map(|byte| (true, byte))
            .collect::<Vec<_>>()
    );
}

#[test]
fn framebuffer_line_traces_select_first_and_last_pages() {
    let first_payload = [0xa5; LCD_WIDTH];
    let last_payload = [0x5a; LCD_WIDTH];

    assert_eq!(
        &flattened(DisplayBlitPlan::framebuffer_line(0, &first_payload).unwrap())[..4],
        &[(false, 0x40), (false, 0xb1), (false, 0x10), (false, 0x04)]
    );
    assert_eq!(
        &flattened(DisplayBlitPlan::framebuffer_line(6, &last_payload).unwrap())[..4],
        &[(false, 0x40), (false, 0xb7), (false, 0x10), (false, 0x04)]
    );
}

#[test]
fn full_frame_preserves_page_order_and_excludes_status_payload() {
    let status = [0xee; LCD_WIDTH];
    let rows = [
        [0x10; LCD_WIDTH],
        [0x21; LCD_WIDTH],
        [0x32; LCD_WIDTH],
        [0x43; LCD_WIDTH],
        [0x54; LCD_WIDTH],
        [0x65; LCD_WIDTH],
        [0x76; LCD_WIDTH],
    ];
    let row_slices: [&[u8]; FRAME_LINES] = rows.each_ref().map(|row| row.as_slice());
    let trace = flattened(DisplayBlitPlan::full_frame(&row_slices).unwrap());

    assert_eq!(
        &trace[..4],
        &[(false, 0x40), (false, 0xb1), (false, 0x10), (false, 0x04)]
    );
    let mut cursor = 4;
    for (index, row) in rows.iter().enumerate() {
        if index != 0 {
            assert_eq!(
                &trace[cursor..cursor + 3],
                &[(false, 0xb1 + index as u8), (false, 0x10), (false, 0x04)]
            );
            cursor += 3;
        }
        assert_eq!(
            &trace[cursor..cursor + LCD_WIDTH],
            &row.iter()
                .copied()
                .map(|byte| (true, byte))
                .collect::<Vec<_>>()
        );
        cursor += LCD_WIDTH;
    }
    assert_eq!(trace.len(), 1 + FRAME_LINES * (3 + LCD_WIDTH));
    assert_eq!(cursor, trace.len());
    assert!(
        !trace
            .iter()
            .any(|(is_data, byte)| *is_data && *byte == status[0])
    );
    assert_eq!(trace[trace.len() - LCD_WIDTH..][0], (true, 0x76));
}

#[test]
fn draw_line_applies_column_offset_and_accepts_last_visible_column() {
    assert_eq!(
        flattened(DisplayBlitPlan::draw_line(7, 127, &[0xab]).unwrap()),
        [(false, 0xb7), (false, 0x18), (false, 0x03), (true, 0xab)]
    );
    assert_eq!(
        flattened(DisplayBlitPlan::draw_line(0, 0, &[1, 2]).unwrap()),
        [
            (false, 0xb0),
            (false, 0x10),
            (false, 0x04),
            (true, 1),
            (true, 2)
        ]
    );
}

#[test]
fn constructors_reject_invalid_page_column_and_payload_requests() {
    assert!(DisplayBlitPlan::draw_line(8, 0, &[1]).is_none());
    assert!(DisplayBlitPlan::draw_line(u16::MAX, 0, &[1]).is_none());
    assert!(DisplayBlitPlan::draw_line(0, 128, &[1]).is_none());
    assert!(DisplayBlitPlan::draw_line(0, u16::MAX, &[1]).is_none());
    assert!(DisplayBlitPlan::draw_line(0, 0, &[]).is_none());
    assert!(DisplayBlitPlan::draw_line(0, 127, &[1, 2]).is_none());
    assert!(DisplayBlitPlan::draw_line(0, 1, &[0; LCD_WIDTH]).is_none());
    assert!(DisplayBlitPlan::status_line(&[0; LCD_WIDTH - 1]).is_none());
    assert!(DisplayBlitPlan::status_line(&[0; LCD_WIDTH + 1]).is_none());
    assert!(DisplayBlitPlan::framebuffer_line(7, &[0; LCD_WIDTH]).is_none());
    assert!(DisplayBlitPlan::framebuffer_line(u16::MAX, &[0; LCD_WIDTH]).is_none());
    assert!(DisplayBlitPlan::framebuffer_line(0, &[0; LCD_WIDTH - 1]).is_none());
}

#[test]
fn full_frame_rejects_wrong_page_count_or_payload_width() {
    let short = [0; LCD_WIDTH - 1];
    let full = [0; LCD_WIDTH];
    let too_many: [&[u8]; FRAME_LINES + 1] = [&full[..]; FRAME_LINES + 1];
    let too_few: [&[u8]; FRAME_LINES - 1] = [&full[..]; FRAME_LINES - 1];
    let wrong_width = [
        &full[..],
        &full[..],
        &full[..],
        &full[..],
        &full[..],
        &full[..],
        &short[..],
    ];

    assert!(DisplayBlitPlan::full_frame(&too_many).is_none());
    assert!(DisplayBlitPlan::full_frame(&too_few).is_none());
    assert!(DisplayBlitPlan::full_frame(&wrong_width).is_none());
}
