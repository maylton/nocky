use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

const SLOT_WIDTH: i32 = 228;
const SLOT_HEIGHT: i32 = 224;
const CARD_X: f64 = 4.0;
const REST_Y: f64 = 7.0;

const HOVER_MS: u64 = 180;
const CLICK_MS: u64 = 600;
const COMPRESS_END: f64 = 0.17;
const EXPAND_END: f64 = 0.47;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HomeCardKind {
    Album,
    Artist,
    Playlist,
    Mix,
}

struct MotionState {
    motion: glib::WeakRef<gtk::Fixed>,
    card: glib::WeakRef<gtk::Box>,
    artwork: glib::WeakRef<gtk::Stack>,
    picture: Option<glib::WeakRef<gtk::Picture>>,
    placeholder: Option<glib::WeakRef<gtk::Image>>,
    hovered: Cell<bool>,
    current_y: Cell<f64>,
    current_artwork_delta: Cell<f64>,
    base_artwork_size: Cell<i32>,
    animation_source: RefCell<Option<glib::SourceId>>,
}

impl MotionState {
    fn stop_animation(&self) {
        if let Some(source) = self.animation_source.borrow_mut().take() {
            source.remove();
        }
    }

    fn refresh_base_size(&self) {
        if let Some(artwork) = self.artwork.upgrade() {
            let measured = artwork.width();
            if measured > 0 {
                self.base_artwork_size.set(measured);
            }
        }
    }

    fn apply(&self, y: f64, artwork_delta: f64) {
        if let (Some(motion), Some(card)) = (self.motion.upgrade(), self.card.upgrade()) {
            motion.move_(&card, CARD_X, y);
        }

        let base = self.base_artwork_size.get().max(124);
        let size = (base as f64 + artwork_delta).round().max(1.0) as i32;

        if let Some(picture) = self.picture.as_ref().and_then(|picture| picture.upgrade()) {
            picture.set_size_request(size, size);
        }

        if let Some(placeholder) = self
            .placeholder
            .as_ref()
            .and_then(|placeholder| placeholder.upgrade())
        {
            placeholder.set_pixel_size((size / 3).max(24));
        }

        self.current_y.set(y);
        self.current_artwork_delta.set(artwork_delta);
    }

    fn rest_target(&self) -> (f64, f64) {
        if self.hovered.get() {
            (REST_Y - 2.0, 4.0)
        } else {
            (REST_Y, 0.0)
        }
    }
}

/// Installs the first Home-card Expressive motion pass.
///
/// The outer button slot never changes size. Only the card surface moves
/// inside the slot, and only the artwork content zooms, preventing carousel
/// reflow or height changes.
pub(crate) fn install(
    button: &gtk::Button,
    card: &gtk::Box,
    artwork: &gtk::Stack,
    kind: HomeCardKind,
) {
    button.set_size_request(SLOT_WIDTH, SLOT_HEIGHT);
    button.set_halign(gtk::Align::Center);
    button.set_valign(gtk::Align::Start);
    button.set_overflow(gtk::Overflow::Visible);
    button.add_css_class("expressive-home-card-button");

    card.set_size_request(220, 210);
    card.add_css_class("expressive-home-card-motion");
    card.add_css_class(match kind {
        HomeCardKind::Album => "expressive-home-album-card",
        HomeCardKind::Artist => "expressive-home-artist-card",
        HomeCardKind::Playlist => "expressive-home-playlist-card",
        HomeCardKind::Mix => "expressive-home-mix-card",
    });

    artwork.add_css_class("expressive-home-card-artwork");
    artwork.add_css_class(match kind {
        HomeCardKind::Artist => "expressive-home-artist-artwork",
        _ => "expressive-home-media-artwork",
    });

    let picture = artwork
        .last_child()
        .and_then(|child| child.downcast::<gtk::Picture>().ok());
    if let Some(picture) = picture.as_ref() {
        picture.set_halign(gtk::Align::Center);
        picture.set_valign(gtk::Align::Center);
        picture.set_can_shrink(true);
        picture.add_css_class("expressive-home-card-picture");
    }

    let placeholder = artwork
        .first_child()
        .and_then(|child| child.downcast::<gtk::Image>().ok());

    let motion = gtk::Fixed::new();
    motion.set_size_request(SLOT_WIDTH, SLOT_HEIGHT);
    motion.set_halign(gtk::Align::Center);
    motion.set_valign(gtk::Align::Start);
    motion.set_overflow(gtk::Overflow::Visible);
    motion.add_css_class("expressive-home-card-slot");
    motion.put(card, CARD_X, REST_Y);
    button.set_child(Some(&motion));

    let state = Rc::new(MotionState {
        motion: motion.downgrade(),
        card: card.downgrade(),
        artwork: artwork.downgrade(),
        picture: picture.as_ref().map(|picture| picture.downgrade()),
        placeholder: placeholder
            .as_ref()
            .map(|placeholder| placeholder.downgrade()),
        hovered: Cell::new(false),
        current_y: Cell::new(REST_Y),
        current_artwork_delta: Cell::new(0.0),
        base_artwork_size: Cell::new(124),
        animation_source: RefCell::new(None),
    });

    state.refresh_base_size();
    state.apply(REST_Y, 0.0);

    let pointer = gtk::EventControllerMotion::new();

    {
        let weak_button = button.downgrade();
        let state = Rc::clone(&state);
        pointer.connect_enter(move |_, _, _| {
            state.hovered.set(true);
            if let Some(button) = weak_button.upgrade() {
                button.add_css_class("is-hovered");
                if adw::is_animations_enabled(&button) {
                    animate_to(&state, REST_Y - 2.0, 4.0, HOVER_MS);
                } else {
                    state.refresh_base_size();
                    state.apply(REST_Y, 0.0);
                }
            }
        });
    }

    {
        let weak_button = button.downgrade();
        let state = Rc::clone(&state);
        pointer.connect_leave(move |_| {
            state.hovered.set(false);
            if let Some(button) = weak_button.upgrade() {
                button.remove_css_class("is-hovered");
                if adw::is_animations_enabled(&button) {
                    animate_to(&state, REST_Y, 0.0, HOVER_MS);
                } else {
                    state.apply(REST_Y, 0.0);
                }
            }
        });
    }

    button.add_controller(pointer);

    {
        let weak_button = button.downgrade();
        let state = Rc::clone(&state);
        button.connect_clicked(move |_| {
            let Some(button) = weak_button.upgrade() else {
                return;
            };

            if !adw::is_animations_enabled(&button) {
                return;
            }

            button.add_css_class("is-clicking");
            animate_click(&state, &button);
        });
    }
}

fn animate_to(state: &Rc<MotionState>, target_y: f64, target_delta: f64, duration_ms: u64) {
    state.stop_animation();
    state.refresh_base_size();

    let from_y = state.current_y.get();
    let from_delta = state.current_artwork_delta.get();
    let started = Instant::now();
    let weak_state = Rc::downgrade(state);

    let source = glib::timeout_add_local(Duration::from_millis(16), move || {
        let Some(state) = weak_state.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let progress = (elapsed_ms / duration_ms as f64).clamp(0.0, 1.0);
        let eased = ease_out_cubic(progress);

        state.apply(
            lerp(from_y, target_y, eased),
            lerp(from_delta, target_delta, eased),
        );

        if progress >= 1.0 {
            state.animation_source.borrow_mut().take();
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });

    state.animation_source.borrow_mut().replace(source);
}

fn animate_click(state: &Rc<MotionState>, button: &gtk::Button) {
    state.stop_animation();
    state.refresh_base_size();

    let start_y = state.current_y.get();
    let start_delta = state.current_artwork_delta.get();
    let weak_state = Rc::downgrade(state);
    let weak_button = button.downgrade();
    let started = Instant::now();

    let source = glib::timeout_add_local(Duration::from_millis(16), move || {
        let Some(state) = weak_state.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let progress = (elapsed_ms / CLICK_MS as f64).clamp(0.0, 1.0);
        let (rest_y, rest_delta) = state.rest_target();

        let (y, delta) = if progress <= COMPRESS_END {
            let local = (progress / COMPRESS_END).clamp(0.0, 1.0);
            let eased = ease_out_cubic(local);
            (
                lerp(start_y, REST_Y + 2.0, eased),
                lerp(start_delta, -4.0, eased),
            )
        } else if progress <= EXPAND_END {
            let local = ((progress - COMPRESS_END) / (EXPAND_END - COMPRESS_END)).clamp(0.0, 1.0);
            let eased = ease_out_back(local, 0.78);
            (
                lerp(REST_Y + 2.0, REST_Y - 4.0, eased),
                lerp(-4.0, 7.0, eased),
            )
        } else {
            let local = ((progress - EXPAND_END) / (1.0 - EXPAND_END)).clamp(0.0, 1.0);
            let eased = ease_out_back(local, 1.04);
            (
                lerp(REST_Y - 4.0, rest_y, eased),
                lerp(7.0, rest_delta, eased),
            )
        };

        state.apply(y, delta);

        if progress >= 1.0 {
            state.animation_source.borrow_mut().take();
            state.apply(rest_y, rest_delta);
            if let Some(button) = weak_button.upgrade() {
                button.remove_css_class("is-clicking");
            }
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });

    state.animation_source.borrow_mut().replace(source);
}

fn lerp(from: f64, to: f64, amount: f64) -> f64 {
    from + (to - from) * amount
}

fn ease_out_cubic(value: f64) -> f64 {
    1.0 - (1.0 - value).powi(3)
}

fn ease_out_back(value: f64, overshoot: f64) -> f64 {
    let shifted = value - 1.0;
    1.0 + (overshoot + 1.0) * shifted.powi(3) + overshoot * shifted.powi(2)
}
