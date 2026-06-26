use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    f64::consts::PI,
    rc::Rc,
    time::{Duration, Instant},
};

const SWITCHER_HEIGHT: i32 = 48;
const MIN_SEGMENT_WIDTH: i32 = 126;
const SEGMENT_HORIZONTAL_PADDING: i32 = 34;
const INDICATOR_INSET: i32 = 8;
const INDICATOR_HEIGHT: i32 = 40;
const INDICATOR_Y: f64 = 4.0;
const INDICATOR_MARGIN_X: f64 = 4.0;
const ANIMATION_MS: u64 = 220;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TopPage {
    Home,
    Lyrics,
}

#[derive(Clone, Copy)]
pub(crate) struct AnimatedPageSpec<'a> {
    pub icon_name: &'a str,
    pub label: &'a str,
}

pub(crate) struct AnimatedPageSwitcher {
    root: gtk::Fixed,
    indicator: gtk::Box,
    buttons: Vec<gtk::Button>,
    labels: Vec<gtk::Label>,
    segment_widths: Vec<i32>,
    segment_offsets: Vec<f64>,
    current_x: Rc<Cell<f64>>,
    current_width: Rc<Cell<f64>>,
    animation_source: Rc<RefCell<Option<glib::SourceId>>>,
}

impl AnimatedPageSwitcher {
    pub(crate) fn new(home_text: &str, lyrics_text: &str) -> Rc<Self> {
        Self::from_specs(&[
            AnimatedPageSpec {
                icon_name: "folder-music-symbolic",
                label: home_text,
            },
            AnimatedPageSpec {
                icon_name: "audio-input-microphone-symbolic",
                label: lyrics_text,
            },
        ])
    }

    pub(crate) fn from_specs(specs: &[AnimatedPageSpec<'_>]) -> Rc<Self> {
        assert!(
            !specs.is_empty(),
            "AnimatedPageSwitcher requires at least one page"
        );

        let root = gtk::Fixed::new();
        root.set_halign(gtk::Align::Center);
        root.set_overflow(gtk::Overflow::Hidden);
        root.add_css_class("animated-page-switcher");

        let mut buttons = Vec::with_capacity(specs.len());
        let mut labels = Vec::with_capacity(specs.len());
        let mut segment_widths = Vec::with_capacity(specs.len());

        for spec in specs {
            let (button, label) = navigation_button(spec.icon_name, spec.label);
            let (_, natural_width, _, _) = button.measure(gtk::Orientation::Horizontal, -1);
            let segment_width = (natural_width + SEGMENT_HORIZONTAL_PADDING).max(MIN_SEGMENT_WIDTH);
            button.set_size_request(segment_width, SWITCHER_HEIGHT);

            buttons.push(button);
            labels.push(label);
            segment_widths.push(segment_width);
        }

        let mut segment_offsets = Vec::with_capacity(segment_widths.len());
        let mut running_offset = 0_i32;
        for width in &segment_widths {
            segment_offsets.push(running_offset as f64);
            running_offset += *width;
        }

        let switcher_width = running_offset;
        root.set_size_request(switcher_width, SWITCHER_HEIGHT);

        let track = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        track.set_size_request(switcher_width, SWITCHER_HEIGHT);
        track.set_can_target(false);
        track.add_css_class("animated-page-switcher-track");
        root.put(&track, 0.0, 0.0);

        let initial_indicator_width = segment_widths[0] - INDICATOR_INSET;
        let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        indicator.set_size_request(initial_indicator_width, INDICATOR_HEIGHT);
        indicator.set_can_target(false);
        indicator.add_css_class("animated-page-switcher-indicator");
        root.put(&indicator, INDICATOR_MARGIN_X, INDICATOR_Y);

        for (index, button) in buttons.iter().enumerate() {
            let active = index == 0;
            if active {
                button.add_css_class("active");
            }
            button.update_state(&[gtk::accessible::State::Selected(Some(active))]);
            root.put(button, segment_offsets[index], 0.0);
        }

        Rc::new(Self {
            root,
            indicator,
            buttons,
            labels,
            segment_widths,
            segment_offsets,
            current_x: Rc::new(Cell::new(INDICATOR_MARGIN_X)),
            current_width: Rc::new(Cell::new(initial_indicator_width as f64)),
            animation_source: Rc::new(RefCell::new(None)),
        })
    }

    pub(crate) fn root(&self) -> &gtk::Fixed {
        &self.root
    }

    pub(crate) fn connect_selected<F>(&self, callback: F)
    where
        F: Fn(usize) + Clone + 'static,
    {
        for (index, button) in self.buttons.iter().enumerate() {
            let callback = callback.clone();
            button.connect_clicked(move |_| callback(index));
        }
    }

    pub(crate) fn connect_home_clicked<F>(&self, callback: F)
    where
        F: Fn() + 'static,
    {
        if let Some(button) = self.buttons.first() {
            button.connect_clicked(move |_| callback());
        }
    }

    pub(crate) fn connect_lyrics_clicked<F>(&self, callback: F)
    where
        F: Fn() + 'static,
    {
        if let Some(button) = self.buttons.get(1) {
            button.connect_clicked(move |_| callback());
        }
    }

    pub(crate) fn set_labels(&self, home: &str, lyrics: &str) {
        self.set_label(0, home);
        self.set_label(1, lyrics);
    }

    pub(crate) fn set_label(&self, index: usize, text: &str) {
        if let Some(label) = self.labels.get(index) {
            label.set_text(text);
        }
        if let Some(button) = self.buttons.get(index) {
            button.set_tooltip_text(Some(text));
        }
    }

    pub(crate) fn set_active_page(&self, page: TopPage, animate: bool) {
        self.set_active_index(
            match page {
                TopPage::Home => 0,
                TopPage::Lyrics => 1,
            },
            animate,
        );
    }

    pub(crate) fn set_active_index(&self, index: usize, animate: bool) {
        if index >= self.buttons.len() {
            return;
        }

        for (button_index, button) in self.buttons.iter().enumerate() {
            let active = button_index == index;
            button.set_css_classes(&page_button_classes(active));
            button.update_state(&[gtk::accessible::State::Selected(Some(active))]);
        }

        let target_x = self.segment_offsets[index] + INDICATOR_MARGIN_X;
        let target_width = (self.segment_widths[index] - INDICATOR_INSET) as f64;
        let animations_enabled = adw::is_animations_enabled(&self.root);

        if !animate || !animations_enabled {
            self.stop_animation();
            self.indicator
                .set_size_request(target_width.round() as i32, INDICATOR_HEIGHT);
            self.root.move_(&self.indicator, target_x, INDICATOR_Y);
            self.current_x.set(target_x);
            self.current_width.set(target_width);
            return;
        }

        let from_x = self.current_x.get();
        let from_width = self.current_width.get();

        if (from_x - target_x).abs() < f64::EPSILON
            && (from_width - target_width).abs() < f64::EPSILON
        {
            return;
        }

        self.stop_animation();

        let root = self.root.clone();
        let indicator = self.indicator.clone();
        let current_x = self.current_x.clone();
        let current_width = self.current_width.clone();
        let source_slot = self.animation_source.clone();
        let started = Instant::now();

        let source = glib::timeout_add_local(Duration::from_millis(16), move || {
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let progress = (elapsed_ms / ANIMATION_MS as f64).clamp(0.0, 1.0);

            let overshoot = 1.18;
            let shifted = progress - 1.0;
            let eased = 1.0 + (overshoot + 1.0) * shifted.powi(3) + overshoot * shifted.powi(2);

            let from_center = from_x + from_width / 2.0;
            let target_center = target_x + target_width / 2.0;
            let center = from_center + (target_center - from_center) * eased;

            let base_width = from_width + (target_width - from_width) * eased;
            let stretch = 8.0 * (PI * progress).sin();
            let width = (base_width + stretch).max(1.0);
            let visual_x = center - width / 2.0;

            indicator.set_size_request(width.round() as i32, INDICATOR_HEIGHT);
            root.move_(&indicator, visual_x, INDICATOR_Y);
            current_x.set(visual_x);
            current_width.set(width);

            if progress >= 1.0 {
                indicator.set_size_request(target_width.round() as i32, INDICATOR_HEIGHT);
                root.move_(&indicator, target_x, INDICATOR_Y);
                current_x.set(target_x);
                current_width.set(target_width);
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
}

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
    slot.set_height_request(INDICATOR_HEIGHT);
    slot.set_halign(gtk::Align::Fill);
    slot.set_hexpand(true);
    slot.set_valign(gtk::Align::Center);
    slot.set_center_widget(Some(&content));

    let button = gtk::Button::new();
    button.set_height_request(SWITCHER_HEIGHT);
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
