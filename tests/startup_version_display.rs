use uv_k5_rust_firmware::startup_version_display::{
    LCD_WIDTH, STARTUP_VERSION_LINE, StartupDisplayMode, StartupVersionDisplayPlan,
    StartupVersionError, StartupVersionRenderError, StartupVersionRenderer, StartupVersionText,
    render_small_normal_text_at, startup_version_display_plan,
};

#[derive(Default)]
struct MockRenderer {
    calls: Vec<(Vec<u8>, usize, usize)>,
}

impl StartupVersionRenderer for MockRenderer {
    fn print_small_normal_at(&mut self, text: &[u8], start_column: usize, line: usize) {
        self.calls.push((text.to_vec(), start_column, line));
    }
}

fn text(mode: StartupDisplayMode, version: Option<&str>) -> StartupVersionText {
    match startup_version_display_plan(mode, version).unwrap() {
        StartupVersionDisplayPlan::Text(text) => text,
        StartupVersionDisplayPlan::Suppressed => panic!("expected version text"),
    }
}

fn plan(mode: StartupDisplayMode, version: Option<&str>) -> StartupVersionDisplayPlan {
    startup_version_display_plan(mode, version).unwrap()
}

fn patterned_pages() -> [[u8; LCD_WIDTH]; STARTUP_VERSION_LINE + 1] {
    std::array::from_fn(|page| std::array::from_fn(|column| (page * 17 + column) as u8))
}

#[test]
fn formats_brand_without_padding_or_container_suffix() {
    let plan = text(StartupDisplayMode::Custom, None);
    assert_eq!(plan.as_bytes(), b"M0HDY");
    assert_eq!(plan.start_column(), 47);
}

#[test]
fn appends_the_supplied_version_and_centers_the_complete_text() {
    let plan = text(StartupDisplayMode::Voltage, Some("dev"));
    assert_eq!(plan.as_bytes(), b"M0HDY dev");
    assert_eq!(plan.start_column(), 33);

    let mut renderer = MockRenderer::default();
    StartupVersionDisplayPlan::Text(plan).render_with(&mut renderer);
    assert_eq!(
        renderer.calls,
        [(b"M0HDY dev".to_vec(), 33, STARTUP_VERSION_LINE)]
    );
}

#[test]
fn accepts_the_widest_centered_text_that_fits() {
    let plan = text(StartupDisplayMode::Custom, Some("123456789012"));
    assert_eq!(plan.as_bytes(), b"M0HDY 123456789012");
    assert_eq!(plan.start_column(), 1);
}

#[test]
fn rejects_empty_unsupported_and_too_wide_versions() {
    assert_eq!(
        startup_version_display_plan(StartupDisplayMode::Custom, Some("")),
        Err(StartupVersionError::EmptyVersion)
    );
    assert_eq!(
        startup_version_display_plan(StartupDisplayMode::Custom, Some("dev\n")),
        Err(StartupVersionError::UnsupportedCharacter)
    );
    assert_eq!(
        startup_version_display_plan(StartupDisplayMode::Custom, Some("é")),
        Err(StartupVersionError::UnsupportedCharacter)
    );
    assert_eq!(
        startup_version_display_plan(StartupDisplayMode::Custom, Some("1234567890123")),
        Err(StartupVersionError::TextTooWide)
    );
}

#[test]
fn suppressed_modes_do_not_render_or_validate_unused_versions() {
    for mode in [StartupDisplayMode::None, StartupDisplayMode::FullScreen] {
        let plan = startup_version_display_plan(mode, Some("é")).unwrap();
        assert_eq!(plan, StartupVersionDisplayPlan::Suppressed);

        let mut renderer = MockRenderer::default();
        plan.render_with(&mut renderer);
        assert!(renderer.calls.is_empty());

        let mut pages = patterned_pages();
        let original = pages;
        plan.render_page(&mut pages).unwrap();
        assert_eq!(pages, original);
    }
}

#[test]
fn renders_m0hdy_golden_bytes_on_the_selected_page() {
    let plan = plan(StartupDisplayMode::Custom, None);
    let mut pages = patterned_pages();
    let mut expected = patterned_pages();
    expected[STARTUP_VERSION_LINE][48..54].copy_from_slice(&[0x7F, 0x02, 0x04, 0x04, 0x02, 0x7F]);
    expected[STARTUP_VERSION_LINE][55..61].copy_from_slice(&[0x3E, 0x41, 0x41, 0x41, 0x41, 0x3E]);
    expected[STARTUP_VERSION_LINE][62..68].copy_from_slice(&[0x7F, 0x08, 0x08, 0x08, 0x08, 0x7F]);
    expected[STARTUP_VERSION_LINE][69..75].copy_from_slice(&[0x7F, 0x41, 0x41, 0x41, 0x41, 0x3E]);
    expected[STARTUP_VERSION_LINE][76..82].copy_from_slice(&[0x07, 0x08, 0x70, 0x08, 0x07, 0x00]);

    plan.render_page(&mut pages).unwrap();

    assert_eq!(pages, expected);
}

#[test]
fn renders_m0hdy_dev_golden_bytes_and_preserves_spacing() {
    let plan = plan(StartupDisplayMode::Voltage, Some("dev"));
    let mut pages = patterned_pages();
    let mut expected = patterned_pages();
    expected[STARTUP_VERSION_LINE][34..40].copy_from_slice(&[0x7F, 0x02, 0x04, 0x04, 0x02, 0x7F]);
    expected[STARTUP_VERSION_LINE][41..47].copy_from_slice(&[0x3E, 0x41, 0x41, 0x41, 0x41, 0x3E]);
    expected[STARTUP_VERSION_LINE][48..54].copy_from_slice(&[0x7F, 0x08, 0x08, 0x08, 0x08, 0x7F]);
    expected[STARTUP_VERSION_LINE][55..61].copy_from_slice(&[0x7F, 0x41, 0x41, 0x41, 0x41, 0x3E]);
    expected[STARTUP_VERSION_LINE][62..68].copy_from_slice(&[0x07, 0x08, 0x70, 0x08, 0x07, 0x00]);
    expected[STARTUP_VERSION_LINE][76..82].copy_from_slice(&[0x38, 0x44, 0x44, 0x44, 0x7F, 0x00]);
    expected[STARTUP_VERSION_LINE][83..89].copy_from_slice(&[0x38, 0x54, 0x54, 0x54, 0x48, 0x00]);
    expected[STARTUP_VERSION_LINE][90..96].copy_from_slice(&[0x0C, 0x30, 0x40, 0x30, 0x0C, 0x00]);

    plan.render_page(&mut pages).unwrap();

    assert_eq!(pages, expected);
}

#[test]
fn skips_spaces_and_non_ascii_glyph_bytes() {
    let mut page: [u8; LCD_WIDTH] = std::array::from_fn(|column| column as u8);
    let mut expected = page;
    expected[11..17].copy_from_slice(&[0x7E, 0x09, 0x09, 0x09, 0x09, 0x7E]);

    render_small_normal_text_at(&mut page, b"A \x7f\x80", 10).unwrap();

    assert_eq!(page, expected);
}

#[test]
fn renders_widest_plan_without_touching_the_right_edge() {
    let plan = plan(StartupDisplayMode::Custom, Some("123456789012"));
    let mut pages = patterned_pages();
    let original_right_edge = pages[STARTUP_VERSION_LINE][127];

    plan.render_page(&mut pages).unwrap();

    assert_eq!(
        &pages[STARTUP_VERSION_LINE][121..127],
        &[0x62, 0x51, 0x51, 0x49, 0x49, 0x46]
    );
    assert_eq!(pages[STARTUP_VERSION_LINE][127], original_right_edge);
}

#[test]
fn rejects_invalid_page_geometry_without_partial_writes() {
    let mut short_page = [0xA5; LCD_WIDTH - 1];
    let original_short_page = short_page;
    assert_eq!(
        render_small_normal_text_at(&mut short_page, b"A", 0),
        Err(StartupVersionRenderError::InvalidPageWidth)
    );
    assert_eq!(short_page, original_short_page);

    let mut page = [0x5A; LCD_WIDTH];
    let original_page = page;
    assert_eq!(
        render_small_normal_text_at(&mut page, b"AB", 115),
        Err(StartupVersionRenderError::TextOutOfBounds)
    );
    assert_eq!(page, original_page);

    let mut too_few_pages = patterned_pages();
    let original_pages = too_few_pages;
    assert_eq!(
        plan(StartupDisplayMode::Custom, None)
            .render_page(&mut too_few_pages[..STARTUP_VERSION_LINE]),
        Err(StartupVersionRenderError::MissingPage)
    );
    assert_eq!(too_few_pages, original_pages);
}
