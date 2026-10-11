use truff_ui::overlays::{WhichKeyCategory, WhichKeyMode, WhichKeyOverlay};

#[test]
fn test_which_key_modes_and_help_categories() {
    let mut overlay = WhichKeyOverlay::new();
    assert!(!overlay.is_visible());
    assert_eq!(overlay.mode(), WhichKeyMode::Prefix);

    let default_cats = WhichKeyOverlay::default_help_categories();
    assert!(!default_cats.is_empty());
    assert!(
        default_cats
            .iter()
            .any(|c| c.name.contains("Навигация") || c.name.contains("Motions"))
    );

    overlay.show_help(default_cats);
    assert!(overlay.is_visible());
    assert_eq!(overlay.mode(), WhichKeyMode::Help);
    assert!(!overlay.categories().is_empty());

    overlay.hide();
    assert!(!overlay.is_visible());
    assert!(overlay.hints().is_empty());
    assert!(overlay.categories().is_empty());
}
