#![cfg_attr(not(test), no_std)]

//! Offline formatting and placement of the startup version text.

pub const LCD_WIDTH: usize = 128;
pub const SMALL_FONT_GLYPH_WIDTH: usize = 6;
pub const SMALL_FONT_CHARACTER_SPACING: usize = SMALL_FONT_GLYPH_WIDTH + 1;
pub const STARTUP_VERSION_LINE: usize = 6;
pub const MAX_STARTUP_VERSION_TEXT_BYTES: usize = LCD_WIDTH / SMALL_FONT_CHARACTER_SPACING;

const BRAND: &[u8] = b"M0HDY";
const VERSION_SEPARATOR: u8 = b' ';

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupDisplayMode {
    Voltage,
    Custom,
    None,
    FullScreen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupVersionError {
    EmptyVersion,
    UnsupportedCharacter,
    TextTooWide,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupVersionText {
    bytes: [u8; MAX_STARTUP_VERSION_TEXT_BYTES],
    len: usize,
    start_column: usize,
}

impl StartupVersionText {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    pub const fn start_column(&self) -> usize {
        self.start_column
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupVersionDisplayPlan {
    Text(StartupVersionText),
    Suppressed,
}

impl StartupVersionDisplayPlan {
    pub fn render_with<R: StartupVersionRenderer>(&self, renderer: &mut R) {
        if let Self::Text(text) = self {
            renderer.print_small_normal_at(
                text.as_bytes(),
                text.start_column(),
                STARTUP_VERSION_LINE,
            );
        }
    }
}

pub trait StartupVersionRenderer {
    /// Prints at the literal start column; implementations must not recenter.
    fn print_small_normal_at(&mut self, text: &[u8], start_column: usize, line: usize);
}

pub fn startup_version_display_plan(
    mode: StartupDisplayMode,
    version: Option<&str>,
) -> Result<StartupVersionDisplayPlan, StartupVersionError> {
    if matches!(
        mode,
        StartupDisplayMode::None | StartupDisplayMode::FullScreen
    ) {
        return Ok(StartupVersionDisplayPlan::Suppressed);
    }

    let version = match version {
        Some("") => return Err(StartupVersionError::EmptyVersion),
        Some(version) => Some(version.as_bytes()),
        None => None,
    };
    let version_len = version.map_or(0, <[u8]>::len);
    let len = BRAND
        .len()
        .checked_add(usize::from(version.is_some()))
        .and_then(|length| length.checked_add(version_len))
        .ok_or(StartupVersionError::TextTooWide)?;
    let text_width = len
        .checked_mul(SMALL_FONT_CHARACTER_SPACING)
        .ok_or(StartupVersionError::TextTooWide)?;
    if text_width > LCD_WIDTH {
        return Err(StartupVersionError::TextTooWide);
    }

    if version.is_some_and(|bytes| bytes.iter().any(|byte| !(b' '..=b'~').contains(byte))) {
        return Err(StartupVersionError::UnsupportedCharacter);
    }

    let mut bytes = [0; MAX_STARTUP_VERSION_TEXT_BYTES];
    bytes[..BRAND.len()].copy_from_slice(BRAND);
    if let Some(version) = version {
        bytes[BRAND.len()] = VERSION_SEPARATOR;
        bytes[BRAND.len() + 1..len].copy_from_slice(version);
    }

    Ok(StartupVersionDisplayPlan::Text(StartupVersionText {
        bytes,
        len,
        start_column: (LCD_WIDTH - text_width).div_ceil(2),
    }))
}
