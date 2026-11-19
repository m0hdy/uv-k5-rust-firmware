#![cfg_attr(not(test), no_std)]

//! Offline ST7565 command/data plans derived from the pinned display routines.
//!
//! The command order follows `DrawLine`, `SelectColumnAndLine`,
//! `BlitFullScreen`, `BlitLine`, and `BlitStatusLine` in
//! [`driver/st7565.c`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.c)
//! at revision `7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3`; dimensions come
//! from [`driver/st7565.h`](https://github.com/egzumer/uv-k5-firmware-custom/blob/7607f0a4bd6203d1f06b70556fc1ce0d7399d6b3/driver/st7565.h).
//! This is an independently authored, allocation-free logical plan, not SPI
//! access, application wiring, or evidence of hardware parity.
//!
//! Unlike the C routines, constructors reject invalid pages, columns, and
//! payload lengths instead of relying on unchecked array indexing or allowing
//! command-byte truncation. The column is a visible-screen offset; the plan
//! adds the controller's four-column offset before encoding its commands.

pub const LCD_WIDTH: usize = 128;
pub const FRAME_LINES: usize = 7;
pub const COLUMN_OFFSET: u8 = 4;

const DISPLAY_PAGES: u16 = 8;
const START_LINE_COMMAND: u8 = 0x40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandStep<'a> {
    Command(u8),
    Data(&'a [u8]),
}

#[derive(Clone, Copy)]
enum Payloads<'a> {
    One(&'a [u8]),
    Frame(&'a [&'a [u8]]),
}

impl<'a> Payloads<'a> {
    fn get(self, index: usize) -> Option<&'a [u8]> {
        match self {
            Self::One(payload) if index == 0 => Some(payload),
            Self::Frame(payloads) => payloads.get(index).copied(),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct DisplayBlitPlan<'a> {
    payloads: Payloads<'a>,
    first_page: u8,
    page_count: usize,
    column: u8,
    start_line: bool,
}

impl<'a> DisplayBlitPlan<'a> {
    /// Plans a partial line write using visible-screen coordinates.
    pub const fn draw_line(page: u16, column: u16, payload: &'a [u8]) -> Option<Self> {
        if page >= DISPLAY_PAGES
            || column >= LCD_WIDTH as u16
            || payload.is_empty()
            || payload.len() > LCD_WIDTH - column as usize
        {
            return None;
        }

        Some(Self {
            payloads: Payloads::One(payload),
            first_page: page as u8,
            page_count: 1,
            column: column as u8,
            start_line: false,
        })
    }

    /// Plans the status row on display page zero.
    pub const fn status_line(payload: &'a [u8]) -> Option<Self> {
        if payload.len() != LCD_WIDTH {
            return None;
        }

        Some(Self {
            payloads: Payloads::One(payload),
            first_page: 0,
            page_count: 1,
            column: 0,
            start_line: true,
        })
    }

    /// Plans one framebuffer row; `line` is zero-based in the seven-row buffer.
    pub const fn framebuffer_line(line: u16, payload: &'a [u8]) -> Option<Self> {
        if line >= FRAME_LINES as u16 || payload.len() != LCD_WIDTH {
            return None;
        }

        Some(Self {
            payloads: Payloads::One(payload),
            first_page: line as u8 + 1,
            page_count: 1,
            column: 0,
            start_line: true,
        })
    }

    /// Plans all seven framebuffer rows in order, without the status row.
    pub const fn full_frame(payloads: &'a [&'a [u8]]) -> Option<Self> {
        if payloads.len() != FRAME_LINES {
            return None;
        }
        let mut line = 0;
        while line < FRAME_LINES {
            if payloads[line].len() != LCD_WIDTH {
                return None;
            }
            line += 1;
        }

        Some(Self {
            payloads: Payloads::Frame(payloads),
            first_page: 1,
            page_count: FRAME_LINES,
            column: 0,
            start_line: true,
        })
    }

    pub const fn steps(&self) -> Steps<'a> {
        Steps {
            payloads: self.payloads,
            first_page: self.first_page,
            page_count: self.page_count,
            column: self.column,
            start_line: self.start_line,
            page_index: 0,
            phase: 0,
        }
    }
}

pub struct Steps<'a> {
    payloads: Payloads<'a>,
    first_page: u8,
    page_count: usize,
    column: u8,
    start_line: bool,
    page_index: usize,
    phase: u8,
}

impl<'a> Iterator for Steps<'a> {
    type Item = CommandStep<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let page = self.first_page + self.page_index as u8;
            let column = u16::from(self.column) + u16::from(COLUMN_OFFSET);
            match self.phase {
                0 => {
                    self.phase = 1;
                    if self.start_line {
                        return Some(CommandStep::Command(START_LINE_COMMAND));
                    }
                }
                1 => {
                    self.phase = 2;
                    return Some(CommandStep::Command(0xb0 + page));
                }
                2 => {
                    self.phase = 3;
                    return Some(CommandStep::Command(0x10 | (((column >> 4) as u8) & 0x0f)));
                }
                3 => {
                    self.phase = 4;
                    return Some(CommandStep::Command((column as u8) & 0x0f));
                }
                4 => {
                    let payload = self.payloads.get(self.page_index)?;
                    self.page_index += 1;
                    self.phase = if self.page_index == self.page_count {
                        5
                    } else {
                        1
                    };
                    return Some(CommandStep::Data(payload));
                }
                _ => return None,
            }
        }
    }
}
