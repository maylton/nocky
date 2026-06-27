//! Visual construction of the footer progress area.
//!
//! Position updates, seeking callbacks, theme switching and playback state
//! remain owned by `AppController`.

use crate::ui::widgets::WaveProgress;
use gtk::prelude::*;

const CLASSIC_PAGE: &str = "classic";
const MATERIAL_PAGE: &str = "m3";
const TRANSITION_DURATION_MS: u32 = 160;
const INITIAL_TIME_TEXT: &str = "0:00";
const CENTER_WIDTH: i32 = 500;
const CENTER_HEIGHT: i32 = 60;

pub(crate) struct FooterProgressParts {
    pub(crate) root: gtk::Box,
    pub(crate) stack: gtk::Stack,
    pub(crate) traditional: gtk::Scale,
    pub(crate) wave: WaveProgress,
    pub(crate) elapsed: gtk::Label,
    pub(crate) duration: gtk::Label,
}

pub(crate) fn build_footer_progress(transport: &gtk::Box) -> FooterProgressParts {
    let wave = WaveProgress::new();
    wave.widget().add_css_class("footer-progress-wave");

    let traditional = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
    traditional.set_draw_value(false);
    traditional.set_hexpand(true);
    traditional.add_css_class("footer-classic-progress");
    traditional.add_css_class("footer-progress-track");

    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.add_css_class("footer-progress-stack");
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(TRANSITION_DURATION_MS);
    stack.add_named(&traditional, Some(CLASSIC_PAGE));
    stack.add_named(wave.widget(), Some(MATERIAL_PAGE));

    let elapsed = gtk::Label::new(Some(INITIAL_TIME_TEXT));
    elapsed.add_css_class("time-label");

    let duration = gtk::Label::new(Some(INITIAL_TIME_TEXT));
    duration.add_css_class("time-label");

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_hexpand(true);
    row.add_css_class("footer-progress-row");
    row.append(&elapsed);
    row.append(&stack);
    row.append(&duration);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
    root.set_size_request(CENTER_WIDTH, CENTER_HEIGHT);
    root.set_halign(gtk::Align::Center);
    root.set_valign(gtk::Align::Center);
    root.add_css_class("footer-center-surface");
    root.append(transport);
    root.append(&row);

    FooterProgressParts {
        root,
        stack,
        traditional,
        wave,
        elapsed,
        duration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_names_match_the_existing_theme_switch_contract() {
        assert_eq!(CLASSIC_PAGE, "classic");
        assert_eq!(MATERIAL_PAGE, "m3");
    }

    #[test]
    fn transition_and_geometry_match_the_approved_footer() {
        assert_eq!(TRANSITION_DURATION_MS, 160);
        assert_eq!((CENTER_WIDTH, CENTER_HEIGHT), (500, 60));
        assert_eq!(INITIAL_TIME_TEXT, "0:00");
    }
}
