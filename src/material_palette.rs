use gdk_pixbuf::Pixbuf;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
struct Bucket {
    red: u64,
    green: u64,
    blue: u64,
    count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }

    fn mix(self, target: Self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let channel = |from: u8, to: u8| {
            (f64::from(from) + (f64::from(to) - f64::from(from)) * amount)
                .round()
                .clamp(0.0, 255.0) as u8
        };

        Self::new(
            channel(self.red, target.red),
            channel(self.green, target.green),
            channel(self.blue, target.blue),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MaterialPalette {
    primary: Rgb,
    on_primary: Rgb,
    primary_container: Rgb,
    on_primary_container: Rgb,
    secondary_container: Rgb,
    on_secondary_container: Rgb,
    tertiary: Rgb,
    tertiary_container: Rgb,
    on_tertiary_container: Rgb,
    surface: Rgb,
    surface_container: Rgb,
    surface_container_low: Rgb,
    surface_container_high: Rgb,
    surface_container_highest: Rgb,
    on_surface: Rgb,
    on_surface_variant: Rgb,
    outline: Rgb,
}

impl MaterialPalette {
    pub(crate) fn fallback() -> Self {
        Self::from_seed(Rgb::new(117, 130, 246))
    }

    pub(crate) fn from_cover(path: &Path) -> Option<Self> {
        dominant_seed(path).map(Self::from_seed)
    }
    pub(crate) fn interpolate(self, target: Self, amount: f64) -> Self {
        let primary = self.primary.mix(target.primary, amount);
        let primary_container = self.primary_container.mix(target.primary_container, amount);
        let secondary_container = self
            .secondary_container
            .mix(target.secondary_container, amount);
        let tertiary = self.tertiary.mix(target.tertiary, amount);
        let tertiary_container = self
            .tertiary_container
            .mix(target.tertiary_container, amount);
        let surface = self.surface.mix(target.surface, amount);

        Self {
            primary,
            on_primary: readable_on(primary),
            primary_container,
            on_primary_container: readable_on(primary_container),
            secondary_container,
            on_secondary_container: readable_on(secondary_container),
            tertiary,
            tertiary_container,
            on_tertiary_container: readable_on(tertiary_container),
            surface,
            surface_container: self.surface_container.mix(target.surface_container, amount),
            surface_container_low: self
                .surface_container_low
                .mix(target.surface_container_low, amount),
            surface_container_high: self
                .surface_container_high
                .mix(target.surface_container_high, amount),
            surface_container_highest: self
                .surface_container_highest
                .mix(target.surface_container_highest, amount),
            on_surface: readable_on(surface),
            on_surface_variant: self
                .on_surface_variant
                .mix(target.on_surface_variant, amount),
            outline: self.outline.mix(target.outline, amount),
        }
    }

    fn from_seed(seed: Rgb) -> Self {
        let (mut hue, saturation, _) = rgb_to_hsl(seed);

        if saturation < 0.10 {
            hue = 232.0;
        }

        let expressive_saturation = saturation.clamp(0.42, 0.82);
        let primary = hsl_to_rgb(hue, expressive_saturation, 0.78);
        let primary_container =
            hsl_to_rgb(hue, (expressive_saturation * 0.78).clamp(0.34, 0.66), 0.29);

        let secondary_hue = wrap_hue(hue + 24.0);
        let secondary_container = hsl_to_rgb(
            secondary_hue,
            (expressive_saturation * 0.36).clamp(0.18, 0.34),
            0.29,
        );

        let tertiary_hue = wrap_hue(hue + 72.0);
        let tertiary = hsl_to_rgb(
            tertiary_hue,
            (expressive_saturation * 0.72).clamp(0.34, 0.68),
            0.78,
        );
        let tertiary_container = hsl_to_rgb(
            tertiary_hue,
            (expressive_saturation * 0.60).clamp(0.28, 0.56),
            0.31,
        );

        let surface_saturation = (expressive_saturation * 0.12).clamp(0.045, 0.10);
        let surface = hsl_to_rgb(hue, surface_saturation, 0.085);
        let surface_container_low = hsl_to_rgb(hue, surface_saturation, 0.105);
        let surface_container = hsl_to_rgb(hue, surface_saturation, 0.135);
        let surface_container_high = hsl_to_rgb(hue, surface_saturation, 0.175);
        let surface_container_highest = hsl_to_rgb(hue, surface_saturation, 0.215);

        Self {
            primary,
            on_primary: readable_on(primary),
            primary_container,
            on_primary_container: readable_on(primary_container),
            secondary_container,
            on_secondary_container: readable_on(secondary_container),
            tertiary,
            tertiary_container,
            on_tertiary_container: readable_on(tertiary_container),
            surface,
            surface_container,
            surface_container_low,
            surface_container_high,
            surface_container_highest,
            on_surface: readable_on(surface),
            on_surface_variant: hsl_to_rgb(hue, 0.10, 0.78),
            outline: hsl_to_rgb(hue, 0.08, 0.58),
        }
    }

    pub(crate) fn to_css(self) -> String {
        let primary = self.primary.hex();
        let on_primary = self.on_primary.hex();
        let primary_container = self.primary_container.hex();
        let on_primary_container = self.on_primary_container.hex();
        let secondary_container = self.secondary_container.hex();
        let on_secondary_container = self.on_secondary_container.hex();
        let tertiary = self.tertiary.hex();
        let tertiary_container = self.tertiary_container.hex();
        let on_tertiary_container = self.on_tertiary_container.hex();
        let surface = self.surface.hex();
        let surface_container = self.surface_container.hex();
        let surface_container_low = self.surface_container_low.hex();
        let surface_container_high = self.surface_container_high.hex();
        let surface_container_highest = self.surface_container_highest.hex();
        let on_surface = self.on_surface.hex();
        let on_surface_variant = self.on_surface_variant.hex();
        let outline = self.outline.hex();

        format!(
            r#"
@define-color m3_primary {primary};
@define-color m3_on_primary {on_primary};
@define-color m3_primary_container {primary_container};
@define-color m3_on_primary_container {on_primary_container};
@define-color m3_secondary_container {secondary_container};
@define-color m3_on_secondary_container {on_secondary_container};
@define-color m3_tertiary {tertiary};
@define-color m3_tertiary_container {tertiary_container};
@define-color m3_on_tertiary_container {on_tertiary_container};
@define-color m3_surface {surface};
@define-color m3_surface_container {surface_container};
@define-color m3_surface_container_low {surface_container_low};
@define-color m3_surface_container_high {surface_container_high};
@define-color m3_surface_container_highest {surface_container_highest};
@define-color m3_on_surface {on_surface};
@define-color m3_on_surface_variant {on_surface_variant};
@define-color m3_outline {outline};

window.theme-material-expressive,
window.theme-material-expressive > toastoverlay,
window.theme-material-expressive .app-shell {{
  background-color: {surface};
  color: {on_surface};
}}

window.theme-material-expressive .noctalia-header,
window.theme-material-expressive .sidebar,
window.theme-material-expressive .library-panel,
window.theme-material-expressive .player-bar {{
  background-color: {surface_container};
}}

window.theme-material-expressive .expressive-player-card {{
  background-color: {surface_container};
  background-image:
    radial-gradient(circle at 18% 4%, alpha({primary}, 0.20), transparent 42%),
    linear-gradient(145deg, alpha({tertiary}, 0.08), transparent 58%);
  border-color: alpha({outline}, 0.30);
}}

window.theme-material-expressive .player-now-header {{
  background-color: alpha({secondary_container}, 0.78);
  border-color: alpha({outline}, 0.20);
}}

window.theme-material-expressive .player-eyebrow,
window.theme-material-expressive .player-header-icon,
window.theme-material-expressive .player-secondary-action {{
  color: {on_secondary_container};
}}

window.theme-material-expressive .player-secondary-action:checked {{
  color: {on_primary};
  background-color: {primary};
}}

window.theme-material-expressive .player-artwork {{
  border-color: alpha({primary}, 0.40);
  background-color: {surface_container_high};
  box-shadow:
    0 14px 34px alpha(black, 0.34),
    0 0 0 6px alpha({primary}, 0.075);
}}

window.theme-material-expressive .player-metadata-surface {{
  color: {on_primary_container};
  background-color: {primary_container};
  border-color: alpha({primary}, 0.28);
}}

window.theme-material-expressive .player-track-title,
window.theme-material-expressive .player-favorite-action {{
  color: {on_primary_container};
}}

window.theme-material-expressive .player-artist {{
  color: alpha({on_primary_container}, 0.84);
}}

window.theme-material-expressive .player-album {{
  color: alpha({on_primary_container}, 0.68);
}}

window.theme-material-expressive .player-transport-surface {{
  background-color: {surface_container_high};
  border-color: alpha({outline}, 0.22);
}}

window.theme-material-expressive .player-primary-control {{
  color: {on_primary};
  background-color: {primary};
  box-shadow: 0 8px 22px alpha({primary}, 0.30);
}}

window.theme-material-expressive .player-skip-control {{
  color: {on_secondary_container};
  background-color: alpha({secondary_container}, 0.88);
}}

window.theme-material-expressive .player-mode-control {{
  color: {on_surface_variant};
  background-color: alpha({surface_container_highest}, 0.76);
}}

window.theme-material-expressive .player-mode-control:checked {{
  color: {on_tertiary_container};
  background-color: {tertiary_container};
}}

window.theme-material-expressive .player-visualizer-surface,
window.theme-material-expressive .player-lyrics-surface {{
  background-color: alpha({surface_container_low}, 0.78);
  border-color: alpha({outline}, 0.16);
}}
window.theme-material-expressive .audio-visualizer {{
  color: {primary};
  background-color: alpha({surface_container_low}, 0.84);
  border-color: alpha({primary}, 0.24);
}}
window.theme-material-expressive .expressive-loading-indicator {{
  color: {primary};
}}
window.theme-material-expressive button .expressive-loading-indicator {{
  color: inherit;
}}
window.theme-material-expressive .expressive-footer {{
  background-color: {surface_container_low};
  background-image:
    radial-gradient(circle at 4% 50%, alpha({primary}, 0.16), transparent 32%),
    linear-gradient(110deg, transparent 48%, alpha({tertiary}, 0.065));
  border-color: alpha({outline}, 0.26);
}}

window.theme-material-expressive .footer-info-card {{
  color: {on_primary_container};
  background-color: {primary_container};
  border-color: alpha({primary}, 0.25);
}}

window.theme-material-expressive .footer-track-title,
window.theme-material-expressive .footer-favorite-action {{
  color: {on_primary_container};
}}

window.theme-material-expressive .footer-track-artist {{
  color: alpha({on_primary_container}, 0.78);
}}

window.theme-material-expressive .footer-source-pill,
window.theme-material-expressive .footer-mode-control:checked {{
  color: {on_tertiary_container};
  background-color: {tertiary_container};
  border-color: alpha({tertiary}, 0.22);
}}

window.theme-material-expressive .footer-transport-controls,
window.theme-material-expressive .footer-utility-group {{
  background-color: {surface_container_high};
  border-color: alpha({outline}, 0.20);
}}

window.theme-material-expressive .footer-primary-control {{
  color: {on_primary};
  background-color: {primary};
}}

window.theme-material-expressive .footer-skip-control {{
  color: {on_secondary_container};
  background-color: {secondary_container};
}}

window.theme-material-expressive .footer-mode-control,
window.theme-material-expressive .footer-utility-action {{
  color: {on_surface_variant};
  background-color: alpha({surface_container_highest}, 0.72);
}}

window.theme-material-expressive .footer-utility-action:checked {{
  color: {on_primary};
  background-color: {primary};
}}

window.theme-material-expressive .footer-progress-wave {{
  color: {primary};
}}

window.theme-material-expressive scale.footer-progress-track trough highlight,
window.theme-material-expressive scale.footer-volume-control trough highlight,
window.theme-material-expressive scale.footer-volume-control slider {{
  background-color: {primary};
}}

window.theme-material-expressive scale.footer-volume-control slider {{
  border-color: {surface_container_high};
}}
window.theme-material-expressive .player-progress-wave {{
  color: {primary};
}}

window.theme-material-expressive scale.player-progress-track trough highlight,
window.theme-material-expressive scale.player-progress-track slider {{
  background-color: {primary};
}}

window.theme-material-expressive scale.player-progress-track slider {{
  border-color: {surface_container_high};
}}
window.theme-material-expressive .expressive-header {{
  background-color: {surface_container};
  border-color: alpha({outline}, 0.24);
}}

window.theme-material-expressive .header-brand {{
  color: {on_primary_container};
  background-color: {primary_container};
}}

window.theme-material-expressive .header-navigation-button,
window.theme-material-expressive .header-action-button {{
  color: {on_surface_variant};
}}

window.theme-material-expressive .header-navigation-button:hover,
window.theme-material-expressive .header-action-button:hover {{
  color: {on_surface};
  background-color: {surface_container_high};
}}

window.theme-material-expressive .header-navigation-button:checked,
window.theme-material-expressive .header-action-button:checked {{
  color: {on_primary};
  background-color: {primary};
}}

window.theme-material-expressive .header-view-switcher,
window.theme-material-expressive .expressive-search-entry {{
  background-color: {surface_container_high};
  border-color: alpha({outline}, 0.18);
}}

window.theme-material-expressive .header-view-switcher button:checked {{
  color: {on_secondary_container};
  background-color: {secondary_container};
}}

window.theme-material-expressive .expressive-search-bar,
window.theme-material-expressive .sidebar,
window.theme-material-expressive .expressive-empty-state {{
  background-color: {surface_container_low};
  border-color: alpha({outline}, 0.19);
}}

window.theme-material-expressive .expressive-search-entry:focus {{
  border-color: {primary};
  box-shadow: 0 0 0 2px alpha({primary}, 0.18);
}}

window.theme-material-expressive button.sidebar-row {{
  color: {on_surface_variant};
}}

window.theme-material-expressive button.sidebar-row:hover {{
  color: {on_surface};
  background-color: {surface_container_high};
}}

window.theme-material-expressive button.sidebar-row.active,
window.theme-material-expressive button.sidebar-row:checked {{
  color: {on_primary_container};
  background-color: {primary_container};
}}

window.theme-material-expressive .expressive-empty-state .empty-icon {{
  color: {primary};
}}

window.theme-material-expressive .expressive-empty-action {{
  color: {on_primary};
  background-color: {primary};
  box-shadow: 0 6px 16px alpha({primary}, 0.24);
}}
window.theme-material-expressive .expressive-search-bar {{
  background-color: transparent;
  border-color: transparent;
  box-shadow: none;
}}
window.theme-material-expressive .home-section,
window.theme-material-expressive .collection-page,
window.theme-material-expressive .expressive-library-page {{
  background-color: alpha({surface_container_low}, 0.74);
  border-color: alpha({outline}, 0.16);
}}

window.theme-material-expressive .collection-card,
window.theme-material-expressive .artist-list-button,
window.theme-material-expressive .queue-list row.media-list-row,
window.theme-material-expressive .playlist-list row.playlist-card-row,
window.theme-material-expressive .search-result-button,
window.theme-material-expressive .playlist-editor-surface {{
  color: {on_surface};
  background-color: {surface_container};
  border-color: alpha({outline}, 0.16);
}}

window.theme-material-expressive .home-card-button:hover .collection-card,
window.theme-material-expressive .collection-card-button:hover .collection-card,
window.theme-material-expressive .artist-list-button:hover,
window.theme-material-expressive .queue-list row.media-list-row:hover,
window.theme-material-expressive .playlist-list row.playlist-card-row:hover,
window.theme-material-expressive .search-result-button:hover {{
  background-color: {surface_container_high};
  border-color: alpha({primary}, 0.30);
}}

window.theme-material-expressive .queue-list row.media-list-row:selected,
window.theme-material-expressive .playlist-list row.playlist-card-row:selected {{
  color: {on_primary_container};
  background-color: {primary_container};
  border-color: alpha({primary}, 0.36);
}}

window.theme-material-expressive .collection-artwork,
window.theme-material-expressive .expressive-artwork,
window.theme-material-expressive .playlist-editor-entry,
window.theme-material-expressive .playlist-editor-dropdown {{
  background-color: {surface_container_high};
  border-color: alpha({outline}, 0.18);
}}

window.theme-material-expressive .collection-page-header,
window.theme-material-expressive .expressive-page-header {{
  background-color: {surface_container};
  border-color: alpha({outline}, 0.16);
}}

window.theme-material-expressive .collection-page-icon {{
  color: {on_primary_container};
  background-color: {primary_container};
}}

window.theme-material-expressive .collection-card-title,
window.theme-material-expressive .expressive-card-title,
window.theme-material-expressive .track-title {{
  color: {on_surface};
}}

window.theme-material-expressive .collection-card .dim-label,
window.theme-material-expressive .expressive-card-subtitle,
window.theme-material-expressive .track-number {{
  color: {on_surface_variant};
}}

window.theme-material-expressive .track-number {{
  background-color: alpha({surface_container_highest}, 0.74);
}}

window.theme-material-expressive .source-badge {{
  color: {on_secondary_container};
  background-color: {secondary_container};
}}

window.theme-material-expressive .youtube-source-badge {{
  color: {on_tertiary_container};
  background-color: {tertiary_container};
}}

window.theme-material-expressive .youtube-collection-card,
window.theme-material-expressive row.youtube-media-row,
window.theme-material-expressive row.youtube-playlist-row {{
  background-image:
    linear-gradient(120deg, alpha({tertiary}, 0.075), transparent 56%);
}}

window.theme-material-expressive .playlist-create-action {{
  color: {on_primary};
  background-color: {primary};
}}
window.theme-material-expressive .search-section-card {{
  background-color: alpha({primary_container}, 0.32);
  background-image:
    linear-gradient(130deg, alpha({primary}, 0.065), transparent 54%);
  border-color: alpha({primary}, 0.27);
}}

window.theme-material-expressive .search-section-heading .home-section-title {{
  color: {on_primary_container};
}}

window.theme-material-expressive .search-section-heading .dim-label {{
  color: alpha({on_primary_container}, 0.70);
}}

window.theme-material-expressive .search-results-surface {{
  background-color: alpha({surface_container}, 0.88);
  border-color: alpha({outline}, 0.16);
}}

window.theme-material-expressive .search-result-button,
window.theme-material-expressive
  .search-results-surface
  row.media-list-row {{
  color: {on_surface};
  background-color: {surface_container_high};
  border-color: alpha({primary}, 0.19);
}}

window.theme-material-expressive .search-result-button:hover,
window.theme-material-expressive
  .search-results-surface
  row.media-list-row:hover {{
  background-color: {surface_container_highest};
  border-color: alpha({primary}, 0.38);
}}

window.theme-material-expressive .search-source-badge {{
  color: {on_tertiary_container};
  background-color: {tertiary_container};
}}

window.theme-material-expressive .search-result-arrow {{
  color: {on_surface_variant};
}}
.theme-material-expressive .material-dialog-toolbar,
.theme-material-expressive.settings-dialog,
.theme-material-expressive.youtube-settings-dialog,
.theme-material-expressive.startup-dialog {{
  color: {on_surface};
  background-color: {surface};
}}

.theme-material-expressive .material-dialog-toolbar headerbar,
.theme-material-expressive .settings-surface-row,
.theme-material-expressive .playlist-editor-surface {{
  color: {on_surface};
  background-color: {surface_container};
  border-color: alpha({outline}, 0.17);
}}

.theme-material-expressive .settings-surface-row:hover,
.theme-material-expressive .settings-dropdown,
.theme-material-expressive dropdown.settings-dropdown > button,
.theme-material-expressive .settings-row-action,
.theme-material-expressive dropdown > button,
.theme-material-expressive combobox > button,
.theme-material-expressive entry,
.theme-material-expressive spinbutton,
.theme-material-expressive textview,
.theme-material-expressive textview text {{
  color: {on_surface};
  background-color: {surface_container_high};
  border-color: alpha({outline}, 0.20);
}}

.theme-material-expressive .settings-row-title,
.theme-material-expressive .settings-title,
.theme-material-expressive .startup-dialog-title {{
  color: {on_surface};
}}

.theme-material-expressive .settings-row-subtitle,
.theme-material-expressive .settings-description,
.theme-material-expressive .startup-dialog-description {{
  color: {on_surface_variant};
}}

.theme-material-expressive .settings-primary-action,
.theme-material-expressive .source-choice-button.suggested-action,
.theme-material-expressive .suggested-action,
.theme-material-expressive switch:checked {{
  color: {on_primary};
  background-color: {primary};
}}

.theme-material-expressive .settings-scale trough highlight,
.theme-material-expressive .settings-scale slider,
.theme-material-expressive entry selection,
.theme-material-expressive textview text selection,
.theme-material-expressive checkbutton check:checked,
.theme-material-expressive checkbutton radio:checked {{
  color: {on_primary};
  background-color: {primary};
  border-color: {primary};
}}

.theme-material-expressive switch,
.theme-material-expressive .source-choice-button,
.theme-material-expressive .startup-choice-group,
.theme-material-expressive .expressive-media-list,
.theme-material-expressive .queue-list,
.theme-material-expressive .playlist-list {{
  color: {on_surface};
  background-color: {surface_container_low};
  border-color: alpha({outline}, 0.17);
}}

.theme-material-expressive switch slider {{
  background-color: {on_surface_variant};
}}

.theme-material-expressive switch:checked slider {{
  background-color: {on_primary};
}}

.theme-material-expressive popover.background > contents,
.theme-material-expressive popover > contents,
.theme-material-expressive tooltip.background,
.theme-material-expressive tooltip,
.theme-material-expressive toast {{
  color: {on_surface};
  background-color: {surface_container_highest};
  border-color: alpha({outline}, 0.22);
}}

.theme-material-expressive popover modelbutton,
.theme-material-expressive popover button,
.theme-material-expressive popover listview row {{
  color: {on_surface};
}}

.theme-material-expressive popover modelbutton:hover,
.theme-material-expressive popover button:hover,
.theme-material-expressive popover listview row:hover {{
  background-color: {surface_container_highest};
}}

.theme-material-expressive popover modelbutton:checked,
.theme-material-expressive popover listview row:selected {{
  color: {on_secondary_container};
  background-color: {secondary_container};
}}

.theme-material-expressive .expressive-media-list row:selected,
.theme-material-expressive .queue-list row:selected,
.theme-material-expressive .playlist-list row:selected {{
  color: {on_primary_container};
  background-color: {primary_container};
}}

.theme-material-expressive scrollbar slider {{
  background-color: alpha({on_surface_variant}, 0.34);
}}

.theme-material-expressive scrollbar slider:hover {{
  background-color: alpha({primary}, 0.64);
}}
.theme-material-expressive .settings-hero {{
  color: {on_primary_container};
  background-color: {primary_container};
  background-image:
    linear-gradient(125deg, alpha({primary}, 0.14), transparent 58%);
  border-color: alpha({primary}, 0.30);
}}

.theme-material-expressive .settings-hero-icon-container {{
  color: {on_primary};
  background-color: {primary};
}}

.theme-material-expressive .settings-hero-icon,
.theme-material-expressive .settings-hero-icon-container {{
  color: {on_primary};
}}

.theme-material-expressive .settings-hero .settings-title {{
  color: {on_primary_container};
}}

.theme-material-expressive .settings-hero .settings-description {{
  color: alpha({on_primary_container}, 0.76);
}}

.theme-material-expressive .settings-version-badge {{
  color: {on_tertiary_container};
  background-color: {tertiary_container};
  border-color: alpha({tertiary}, 0.24);
}}

.theme-material-expressive .settings-group {{
  color: {on_surface};
  background-color: {surface_container_low};
  border-color: alpha({outline}, 0.18);
}}

.theme-material-expressive .settings-group-icon-container {{
  color: {on_secondary_container};
  background-color: {secondary_container};
}}

.theme-material-expressive .settings-group-icon {{
  color: {on_secondary_container};
}}

.theme-material-expressive .settings-group-title {{
  color: {on_surface};
}}

.theme-material-expressive .settings-group-description {{
  color: {on_surface_variant};
}}

.theme-material-expressive .settings-group .settings-surface-row {{
  color: {on_surface};
  background-color: {surface_container};
  border-color: alpha({outline}, 0.14);
}}

.theme-material-expressive .settings-group .settings-surface-row:hover {{
  background-color: {surface_container_high};
  border-color: alpha({primary}, 0.27);
}}
.theme-material-expressive.settings-dialog,
.theme-material-expressive.youtube-settings-dialog,
.theme-material-expressive.startup-dialog,
window.theme-material-expressive.settings-dialog,
window.theme-material-expressive.youtube-settings-dialog,
window.theme-material-expressive.startup-dialog {{
  background-color: alpha({surface}, 0.80);
  background-image:
    linear-gradient(140deg, alpha({primary}, 0.08), transparent 62%);
}}

window.theme-material-expressive .collection-card {{
  border-color: alpha({outline}, 0.14);
}}

window.theme-material-expressive .home-card-button:hover .collection-card,
window.theme-material-expressive .collection-card-button:hover .collection-card {{
  border-color: alpha({primary}, 0.28);
}}

window.theme-material-expressive .collection-artwork,
window.theme-material-expressive .expressive-artwork,
window.theme-material-expressive .player-visualizer-surface {{
  border-color: alpha({outline}, 0.15);
}}

.theme-material-expressive popover.background > arrow,
.theme-material-expressive popover > arrow,
.theme-material-expressive popover.background.menu > contents,
.theme-material-expressive popover.menu > contents,
.theme-material-expressive popover.background.modelbutton > contents {{
  color: {on_surface};
  background-color: {surface_container_high};
}}
.theme-material-expressive.settings-dialog .material-dialog-toolbar,
.theme-material-expressive.youtube-settings-dialog .material-dialog-toolbar,
.theme-material-expressive.startup-dialog .material-dialog-toolbar {{
  color: {on_surface};
  background-color: {surface};
  border-color: alpha({outline}, 0.22);
}}

window.theme-material-expressive .collection-card {{
  border-color: alpha({outline}, 0.16);
}}

window.theme-material-expressive .home-card-button:hover .collection-card,
window.theme-material-expressive .collection-card-button:hover .collection-card {{
  border-color: alpha({primary}, 0.30);
}}

window.theme-material-expressive .collection-artwork,
window.theme-material-expressive .expressive-artwork {{
  border-color: alpha({outline}, 0.18);
}}

window.theme-material-expressive scrollbar.horizontal slider,
window.theme-material-expressive .home-carousel-scroll scrollbar.horizontal slider {{
  background-color: alpha({on_surface_variant}, 0.34);
}}

window.theme-material-expressive scrollbar.horizontal slider:hover,
window.theme-material-expressive .home-carousel-scroll scrollbar.horizontal slider:hover {{
  background-color: alpha({primary}, 0.68);
}}

.theme-material-expressive popover.background > arrow,
.theme-material-expressive popover > arrow {{
  color: {on_surface};
  background-color: {surface_container_high};
}}
dialog.settings-dialog.theme-material-expressive,
dialog.youtube-settings-dialog.theme-material-expressive,
dialog.startup-dialog.theme-material-expressive,
.settings-dialog.theme-material-expressive,
.youtube-settings-dialog.theme-material-expressive,
.startup-dialog.theme-material-expressive {{
  color: {on_surface};
  background-color: {surface_container_low};
  background-image:
    radial-gradient(circle at 12% 0%, alpha({primary}, 0.10), transparent 46%);
}}

dialog.settings-dialog.theme-material-expressive .material-dialog-toolbar,
dialog.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar,
dialog.startup-dialog.theme-material-expressive .material-dialog-toolbar,
.settings-dialog.theme-material-expressive .material-dialog-toolbar,
.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar,
.startup-dialog.theme-material-expressive .material-dialog-toolbar {{
  color: {on_surface};
  background-color: {surface_container_low};
  background-image:
    radial-gradient(circle at 12% 0%, alpha({primary}, 0.08), transparent 48%);
  border-color: alpha({outline}, 0.20);
}}

dialog.settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
dialog.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
dialog.startup-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.startup-dialog.theme-material-expressive .material-dialog-toolbar headerbar {{
  color: {on_surface};
  background-color: {surface_container};
  background-image: none;
  border-color: alpha({outline}, 0.18);
}}
window.theme-material-expressive
  .home-carousel-scroll
  scrollbar.horizontal
  slider {{
  color: {on_surface_variant};
  background-color: alpha({on_surface_variant}, 0.38);
}}

window.theme-material-expressive
  .home-carousel-scroll
  scrollbar.horizontal
  slider:hover {{
  background-color: alpha({primary}, 0.76);
}}

window.theme-material-expressive row.media-list-row:selected .track-position-indicator,
window.theme-material-expressive row.media-list-row.playing .track-position-indicator,
window.theme-material-expressive row.media-list-row.current-track .track-position-indicator,
window.theme-material-expressive row.media-list-row.now-playing .track-position-indicator,
window.theme-material-expressive .track-playing-indicator,
window.theme-material-expressive .playing-indicator,
window.theme-material-expressive .current-track-indicator,
window.theme-material-expressive .now-playing-indicator {{
  color: {primary};
  background-color: alpha({primary_container}, 0.86);
  -gtk-icon-shadow: 0 0 10px alpha({primary}, 0.28);
}}
"#
        )
    }
}

fn dominant_seed(path: &Path) -> Option<Rgb> {
    let pixbuf = Pixbuf::from_file_at_scale(path, 64, 64, true).ok()?;

    if pixbuf.bits_per_sample() != 8 {
        return None;
    }

    let width = usize::try_from(pixbuf.width()).ok()?;
    let height = usize::try_from(pixbuf.height()).ok()?;
    let rowstride = usize::try_from(pixbuf.rowstride()).ok()?;
    let channels = usize::try_from(pixbuf.n_channels()).ok()?;

    if width == 0 || height == 0 || channels < 3 {
        return None;
    }

    let bytes = pixbuf.read_pixel_bytes();
    let pixels = bytes.as_ref();
    let mut buckets = [Bucket::default(); 4096];

    for y in 0..height {
        let row = y.saturating_mul(rowstride);

        for x in 0..width {
            let offset = row.saturating_add(x.saturating_mul(channels));
            if offset.saturating_add(channels) > pixels.len() {
                continue;
            }

            if channels >= 4 && pixels[offset + 3] < 96 {
                continue;
            }

            let red = pixels[offset];
            let green = pixels[offset + 1];
            let blue = pixels[offset + 2];
            let (_, saturation, lightness) = rgb_to_hsl(Rgb::new(red, green, blue));

            if !(0.025..=0.975).contains(&lightness) {
                continue;
            }

            let index = ((usize::from(red) >> 4) << 8)
                | ((usize::from(green) >> 4) << 4)
                | (usize::from(blue) >> 4);
            let bucket = &mut buckets[index];
            bucket.red += u64::from(red);
            bucket.green += u64::from(green);
            bucket.blue += u64::from(blue);
            bucket.count += 1;

            if saturation > 0.45 {
                bucket.red += u64::from(red);
                bucket.green += u64::from(green);
                bucket.blue += u64::from(blue);
                bucket.count += 1;
            }
        }
    }

    buckets
        .iter()
        .filter(|bucket| bucket.count > 0)
        .map(|bucket| {
            let rgb = Rgb::new(
                (bucket.red / bucket.count) as u8,
                (bucket.green / bucket.count) as u8,
                (bucket.blue / bucket.count) as u8,
            );
            let (_, saturation, lightness) = rgb_to_hsl(rgb);
            let population = (bucket.count as f64).sqrt();
            let chroma = 0.42 + saturation * 2.2;
            let balance = (1.0 - (lightness - 0.52).abs() * 1.15).clamp(0.32, 1.0);
            (population * chroma * balance, rgb)
        })
        .max_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, rgb)| rgb)
}

fn readable_on(background: Rgb) -> Rgb {
    let dark = Rgb::new(0, 0, 0);
    let light = Rgb::new(255, 255, 255);

    if contrast_ratio(background, dark) >= contrast_ratio(background, light) {
        dark
    } else {
        light
    }
}

fn rgb_to_hsl(rgb: Rgb) -> (f64, f64, f64) {
    let red = f64::from(rgb.red) / 255.0;
    let green = f64::from(rgb.green) / 255.0;
    let blue = f64::from(rgb.blue) / 255.0;

    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let delta = maximum - minimum;
    let lightness = (maximum + minimum) / 2.0;

    if delta <= f64::EPSILON {
        return (0.0, 0.0, lightness);
    }

    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if (maximum - red).abs() <= f64::EPSILON {
        60.0 * (((green - blue) / delta) % 6.0)
    } else if (maximum - green).abs() <= f64::EPSILON {
        60.0 * (((blue - red) / delta) + 2.0)
    } else {
        60.0 * (((red - green) / delta) + 4.0)
    };

    (wrap_hue(hue), saturation.clamp(0.0, 1.0), lightness)
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> Rgb {
    let hue = wrap_hue(hue);
    let saturation = saturation.clamp(0.0, 1.0);
    let lightness = lightness.clamp(0.0, 1.0);

    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let segment = hue / 60.0;
    let secondary = chroma * (1.0 - (segment % 2.0 - 1.0).abs());

    let (red, green, blue) = match segment.floor() as i32 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };

    let match_value = lightness - chroma / 2.0;
    Rgb::new(
        ((red + match_value) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((green + match_value) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((blue + match_value) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn wrap_hue(hue: f64) -> f64 {
    ((hue % 360.0) + 360.0) % 360.0
}

fn contrast_ratio(first: Rgb, second: Rgb) -> f64 {
    let first = relative_luminance(first);
    let second = relative_luminance(second);
    let lighter = first.max(second);
    let darker = first.min(second);
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(rgb: Rgb) -> f64 {
    fn channel(value: u8) -> f64 {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * channel(rgb.red) + 0.7152 * channel(rgb.green) + 0.0722 * channel(rgb.blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_interpolation_keeps_exact_endpoints() {
        let start = MaterialPalette::from_seed(Rgb::new(220, 48, 80));
        let target = MaterialPalette::from_seed(Rgb::new(30, 110, 220));

        assert_eq!(start.interpolate(target, 0.0), start);
        assert_eq!(start.interpolate(target, 1.0), target);
    }

    #[test]
    fn palette_interpolation_keeps_accessible_core_pairs() {
        let start = MaterialPalette::from_seed(Rgb::new(220, 48, 80));
        let target = MaterialPalette::from_seed(Rgb::new(30, 180, 110));

        for step in 0..=20 {
            let palette = start.interpolate(target, f64::from(step) / 20.0);
            assert_accessible(palette.primary, palette.on_primary);
            assert_accessible(palette.primary_container, palette.on_primary_container);
            assert_accessible(palette.secondary_container, palette.on_secondary_container);
            assert_accessible(palette.tertiary_container, palette.on_tertiary_container);
            assert_accessible(palette.surface, palette.on_surface);
        }
    }

    fn assert_accessible(background: Rgb, foreground: Rgb) {
        assert!(
            contrast_ratio(background, foreground) >= 4.5,
            "contrast was {} for {:?} on {:?}",
            contrast_ratio(background, foreground),
            foreground,
            background
        );
    }

    #[test]
    fn fallback_pairs_are_accessible() {
        let palette = MaterialPalette::fallback();
        assert_accessible(palette.primary, palette.on_primary);
        assert_accessible(palette.primary_container, palette.on_primary_container);
        assert_accessible(palette.secondary_container, palette.on_secondary_container);
        assert_accessible(palette.tertiary_container, palette.on_tertiary_container);
        assert_accessible(palette.surface, palette.on_surface);
    }

    #[test]
    fn seed_hues_keep_accessible_pairs() {
        for seed in [
            Rgb::new(220, 48, 80),
            Rgb::new(30, 180, 110),
            Rgb::new(30, 110, 220),
            Rgb::new(210, 170, 40),
            Rgb::new(130, 130, 130),
        ] {
            let palette = MaterialPalette::from_seed(seed);
            assert_accessible(palette.primary, palette.on_primary);
            assert_accessible(palette.primary_container, palette.on_primary_container);
            assert_accessible(palette.surface, palette.on_surface);
        }
    }
}
