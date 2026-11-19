#![cfg_attr(not(test), no_std)]

//! Composes the startup version row renderer with the ST7565 framebuffer plan.

use crate::st7565_display::DisplayBlitPlan;
use crate::startup_version_display::{
    LCD_WIDTH, STARTUP_VERSION_LINE, StartupVersionDisplayPlan, StartupVersionRenderError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupVersionRowError {
    Render(StartupVersionRenderError),
    InvalidFramebufferPlan,
}

impl From<StartupVersionRenderError> for StartupVersionRowError {
    fn from(error: StartupVersionRenderError) -> Self {
        Self::Render(error)
    }
}

/// Renders a visible version into caller-owned pages and plans its framebuffer row.
pub fn compose_startup_version_row<'a>(
    version: &StartupVersionDisplayPlan,
    pages: &'a mut [[u8; LCD_WIDTH]],
) -> Result<Option<DisplayBlitPlan<'a>>, StartupVersionRowError> {
    if matches!(version, StartupVersionDisplayPlan::Suppressed) {
        return Ok(None);
    }

    version.render_page(pages)?;
    let plan = DisplayBlitPlan::framebuffer_line(
        STARTUP_VERSION_LINE as u16,
        &pages[STARTUP_VERSION_LINE],
    )
    .ok_or(StartupVersionRowError::InvalidFramebufferPlan)?;
    Ok(Some(plan))
}
