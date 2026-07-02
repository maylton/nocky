const NOCTALIA_CSS: &str = include_str!("../assets/themes/noctalia.css");

#[test]
fn noctalia_styles_shared_material_button_semantics() {
    for required in [
        "window.theme-noctalia button.material-button",
        "button.material-button-filled",
        "button.material-button-filled-tonal",
        "button.material-button-outlined",
        "button.material-button-destructive",
        "window.theme-noctalia button.material-icon-button",
        "window.theme-noctalia button.material-chip",
        "button.material-chip-selected",
    ] {
        assert!(
            NOCTALIA_CSS.contains(required),
            "missing Noctalia compatibility selector: {required}"
        );
    }
}
