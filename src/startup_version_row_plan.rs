#![cfg_attr(not(test), no_std)]

//! Composes the startup version row renderer with the ST7565 framebuffer plan.

#[allow(dead_code, unused_attributes)]
#[path = "st7565_display.rs"]
mod st7565_display;
#[allow(dead_code, unused_attributes)]
#[path = "startup_version_display.rs"]
mod startup_version_display;

pub use st7565_display::{CommandStep, DisplayBlitPlan};
#[allow(unused_imports)]
pub use startup_version_display::StartupVersionError;
pub use startup_version_display::{
    LCD_WIDTH, StartupDisplayMode, StartupVersionDisplayPlan, StartupVersionRenderError,
    startup_version_display_plan,
};

/// Renders a visible version into caller-owned pages and plans its framebuffer row.
pub fn compose_startup_version_row<'a>(
    version: &StartupVersionDisplayPlan,
    pages: &'a mut [[u8; LCD_WIDTH]],
) -> Result<Option<DisplayBlitPlan<'a>>, StartupVersionRenderError> {
    if matches!(version, StartupVersionDisplayPlan::Suppressed) {
        return Ok(None);
    }

    version.render_page(pages)?;
    Ok(DisplayBlitPlan::framebuffer_line(
        startup_version_display::STARTUP_VERSION_LINE as u16,
        &pages[startup_version_display::STARTUP_VERSION_LINE],
    ))
}
