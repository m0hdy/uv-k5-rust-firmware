#[path = "../src/startup_version_display.rs"]
mod startup_version_display;

use startup_version_display::{
    STARTUP_VERSION_LINE, StartupDisplayMode, StartupVersionDisplayPlan, StartupVersionError,
    StartupVersionRenderer, StartupVersionText, startup_version_display_plan,
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
    }
}
