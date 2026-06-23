use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    f64::consts::PI,
    rc::Rc,
    time::{Duration, Instant},
};

const SWITCHER_WIDTH: i32 = 252;
const SWITCHER_HEIGHT: i32 = 48;
const SEGMENT_WIDTH: i32 = 126;
// animated_switcher_edge_alignment_v1
const INDICATOR_WIDTH: i32 = 118;
const INDICATOR_HEIGHT: i32 = 40;
const INDICATOR_Y: f64 = 4.0;
const HOME_X: f64 = 4.0;
const LYRICS_X: f64 = 130.0;
const ANIMATION_MS: u64 = 220;

// animated_top_page_switcher_v2
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TopPage {
    Home,
    Lyrics,
}

pub(crate) struct AnimatedPageSwitcher {
    root: gtk::Fixed,
    indicator: gtk::Box,
    home_button: gtk::Button,
    lyrics_button: gtk::Button,
    home_label: gtk::Label,
    lyrics_label: gtk::Label,
    current_x: Rc<Cell<f64>>,
    animation_source: Rc<RefCell<Option<glib::SourceId>>>,
}

impl AnimatedPageSwitcher {
    pub(crate) fn new(home_text: &str, lyrics_text: &str) -> Rc<Self> {
        let root = gtk::Fixed::new();
        root.set_size_request(SWITCHER_WIDTH, SWITCHER_HEIGHT);
        root.set_overflow(gtk::Overflow::Hidden);
        root.add_css_class("animated-page-switcher");

        let track = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        track.set_size_request(SWITCHER_WIDTH, SWITCHER_HEIGHT);
        track.set_can_target(false);
        track.add_css_class("animated-page-switcher-track");
        root.put(&track, 0.0, 0.0);

        let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        indicator.set_size_request(INDICATOR_WIDTH, INDICATOR_HEIGHT);
        indicator.set_can_target(false);
        indicator.add_css_class("animated-page-switcher-indicator");
        root.put(&indicator, HOME_X, INDICATOR_Y);

        let (home_button, home_label) = navigation_button("folder-music-symbolic", home_text);
        home_button.add_css_class("active");
        root.put(&home_button, 0.0, 0.0);

        let (lyrics_button, lyrics_label) =
            navigation_button("audio-input-microphone-symbolic", lyrics_text);
        root.put(&lyrics_button, SEGMENT_WIDTH as f64, 0.0);

        let switcher = Rc::new(Self {
            root,
            indicator,
            home_button,
            lyrics_button,
            home_label,
            lyrics_label,
            current_x: Rc::new(Cell::new(HOME_X)),
            animation_source: Rc::new(RefCell::new(None)),
        });

        switcher.update_accessibility(TopPage::Home);
        switcher
    }

    pub(crate) fn root(&self) -> &gtk::Fixed {
        &self.root
    }

    pub(crate) fn connect_home_clicked<F>(&self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.home_button.connect_clicked(move |_| callback());
    }

    pub(crate) fn connect_lyrics_clicked<F>(&self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.lyrics_button.connect_clicked(move |_| callback());
    }

    pub(crate) fn set_labels(&self, home: &str, lyrics: &str) {
        self.home_label.set_text(home);
        self.lyrics_label.set_text(lyrics);
        self.home_button.set_tooltip_text(Some(home));
        self.lyrics_button.set_tooltip_text(Some(lyrics));
    }

    pub(crate) fn set_active_page(&self, page: TopPage, animate: bool) {
        let target_x = match page {
            TopPage::Home => HOME_X,
            TopPage::Lyrics => LYRICS_X,
        };

        self.home_button
            .set_css_classes(&page_button_classes(page == TopPage::Home));
        self.lyrics_button
            .set_css_classes(&page_button_classes(page == TopPage::Lyrics));
        self.update_accessibility(page);

        let animations_enabled = adw::is_animations_enabled(&self.root);
        if !animate || !animations_enabled {
            self.stop_animation();
            self.indicator
                .set_size_request(INDICATOR_WIDTH, INDICATOR_HEIGHT);
            self.root.move_(&self.indicator, target_x, INDICATOR_Y);
            self.current_x.set(target_x);
            return;
        }

        let from_x = self.current_x.get();
        if (from_x - target_x).abs() < f64::EPSILON {
            return;
        }

        self.stop_animation();

        let root = self.root.clone();
        let indicator = self.indicator.clone();
        let current_x = self.current_x.clone();
        let source_slot = self.animation_source.clone();
        let started = Instant::now();

        let source = glib::timeout_add_local(Duration::from_millis(16), move || {
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let progress = (elapsed_ms / ANIMATION_MS as f64).clamp(0.0, 1.0);

            // animated_switcher_centering_and_bounce_v1
            // Light ease-out-back motion adds a subtle bounce without becoming playful.
            let overshoot = 1.18;
            let shifted = progress - 1.0;
            let eased = 1.0 + (overshoot + 1.0) * shifted.powi(3) + overshoot * shifted.powi(2);
            let logical_x = from_x + (target_x - from_x) * eased;

            // Keep a small stretch so the pill feels alive while moving.
            let stretch = 8.0 * (PI * progress).sin();
            let width = INDICATOR_WIDTH as f64 + stretch;
            let center_x = logical_x + INDICATOR_WIDTH as f64 / 2.0;
            let visual_x = center_x - width / 2.0;

            indicator.set_size_request(width.round() as i32, INDICATOR_HEIGHT);
            root.move_(&indicator, visual_x, INDICATOR_Y);
            current_x.set(logical_x);

            if progress >= 1.0 {
                indicator.set_size_request(INDICATOR_WIDTH, INDICATOR_HEIGHT);
                root.move_(&indicator, target_x, INDICATOR_Y);
                current_x.set(target_x);
                source_slot.borrow_mut().take();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });

        self.animation_source.borrow_mut().replace(source);
    }

    fn stop_animation(&self) {
        if let Some(source) = self.animation_source.borrow_mut().take() {
            source.remove();
        }
    }

    fn update_accessibility(&self, page: TopPage) {
        self.home_button
            .update_state(&[gtk::accessible::State::Selected(Some(
                page == TopPage::Home,
            ))]);
        self.lyrics_button
            .update_state(&[gtk::accessible::State::Selected(Some(
                page == TopPage::Lyrics,
            ))]);
    }
}

// animated_switcher_outward_content_alignment_v2
// animated_switcher_centered_reference_style_v1
// animated_switcher_centering_and_bounce_v1
fn navigation_button(icon_name: &str, text: &str) -> (gtk::Button, gtk::Label) {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(17);
    icon.add_css_class("animated-page-switcher-icon");

    let label = gtk::Label::new(Some(text));
    label.add_css_class("animated-page-switcher-label");

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    content.append(&icon);
    content.append(&label);

    let slot = gtk::CenterBox::new();
    slot.set_size_request(INDICATOR_WIDTH, INDICATOR_HEIGHT);
    slot.set_halign(gtk::Align::Center);
    slot.set_valign(gtk::Align::Center);
    slot.set_center_widget(Some(&content));

    let button = gtk::Button::new();
    button.set_size_request(SEGMENT_WIDTH, SWITCHER_HEIGHT);
    button.set_child(Some(&slot));
    button.set_tooltip_text(Some(text));
    button.add_css_class("flat");
    button.add_css_class("animated-page-switcher-button");

    (button, label)
}

fn page_button_classes(active: bool) -> Vec<&'static str> {
    let mut classes = vec!["flat", "animated-page-switcher-button"];
    if active {
        classes.push("active");
    }
    classes
}
