use uv_k5_rust_firmware::st7565_display::{CommandStep, DisplayBlitPlan};
use uv_k5_rust_firmware::startup_version_display::{
    LCD_WIDTH, STARTUP_VERSION_LINE, StartupDisplayMode, StartupVersionDisplayPlan,
    StartupVersionRenderError, startup_version_display_plan,
};
use uv_k5_rust_firmware::startup_version_row_plan::{
    StartupVersionRowError, compose_startup_version_row,
};

fn patterned_pages() -> [[u8; LCD_WIDTH]; STARTUP_VERSION_LINE + 1] {
    std::array::from_fn(|page| std::array::from_fn(|column| (page * 17 + column) as u8))
}

#[test]
fn rendered_version_row_produces_the_matching_framebuffer_trace() {
    let version = startup_version_display_plan(StartupDisplayMode::Custom, None).unwrap();
    let mut pages = patterned_pages();
    let mut expected = patterned_pages();
    expected[STARTUP_VERSION_LINE][48..54].copy_from_slice(&[0x7f, 0x02, 0x04, 0x04, 0x02, 0x7f]);
    expected[STARTUP_VERSION_LINE][55..61].copy_from_slice(&[0x3e, 0x41, 0x41, 0x41, 0x41, 0x3e]);
    expected[STARTUP_VERSION_LINE][62..68].copy_from_slice(&[0x7f, 0x08, 0x08, 0x08, 0x08, 0x7f]);
    expected[STARTUP_VERSION_LINE][69..75].copy_from_slice(&[0x7f, 0x41, 0x41, 0x41, 0x41, 0x3e]);
    expected[STARTUP_VERSION_LINE][76..82].copy_from_slice(&[0x07, 0x08, 0x70, 0x08, 0x07, 0x00]);

    let blit: DisplayBlitPlan<'_> = compose_startup_version_row(&version, &mut pages)
        .unwrap()
        .unwrap();
    assert_eq!(
        blit.steps().collect::<Vec<_>>(),
        [
            CommandStep::Command(0x40),
            CommandStep::Command(0xb7),
            CommandStep::Command(0x10),
            CommandStep::Command(0x04),
            CommandStep::Data(&expected[STARTUP_VERSION_LINE]),
        ]
    );
    assert_eq!(pages, expected);
}

#[test]
fn suppressed_modes_return_no_row_plan_or_modify_the_buffer() {
    for mode in [StartupDisplayMode::None, StartupDisplayMode::FullScreen] {
        let version = startup_version_display_plan(mode, Some("ignored")).unwrap();
        assert_eq!(version, StartupVersionDisplayPlan::Suppressed);

        let mut pages = patterned_pages();
        let original = pages;
        assert!(
            compose_startup_version_row(&version, &mut pages)
                .unwrap()
                .is_none()
        );
        assert_eq!(pages, original);
    }
}

#[test]
fn visible_version_without_its_framebuffer_page_is_rejected() {
    let version = startup_version_display_plan(StartupDisplayMode::Custom, None).unwrap();
    let mut pages = patterned_pages();
    let original = pages;

    assert!(matches!(
        compose_startup_version_row(&version, &mut pages[..STARTUP_VERSION_LINE]),
        Err(StartupVersionRowError::Render(
            StartupVersionRenderError::MissingPage
        ))
    ));
    assert_eq!(pages, original);
}
