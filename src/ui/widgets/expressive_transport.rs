use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
const EXPAND_MS: u64 = 360;
const HOLD_MS: u64 = 160;
const RETURN_MS: u64 = 480;
const TOTAL_MS: u64 = EXPAND_MS + HOLD_MS + RETURN_MS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransportVariant {
    Main,
    Footer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PressTarget {
    Previous,
    Play,
    Next,
}

#[derive(Clone, Copy)]
struct LayoutSpec {
    root_width: i32,
    root_height: i32,
    previous_width: f64,
    play_width: f64,
    next_width: f64,
    secondary_height: f64,
    play_height: f64,
    gap: f64,
    classic_spacing: i32,
}

#[derive(Clone, Copy)]
struct Geometry {
    widths: [f64; 3],
    heights: [f64; 3],
    gaps: [f64; 2],
}

impl Geometry {
    fn lerp(self, other: Self, amount: f64) -> Self {
        let lerp = |from: f64, to: f64| from + (to - from) * amount;
        Self {
            widths: [
                lerp(self.widths[0], other.widths[0]),
                lerp(self.widths[1], other.widths[1]),
                lerp(self.widths[2], other.widths[2]),
            ],
            heights: [
                lerp(self.heights[0], other.heights[0]),
                lerp(self.heights[1], other.heights[1]),
                lerp(self.heights[2], other.heights[2]),
            ],
            gaps: [
                lerp(self.gaps[0], other.gaps[0]),
                lerp(self.gaps[1], other.gaps[1]),
            ],
        }
    }
}

impl TransportVariant {
    fn spec(self) -> LayoutSpec {
        match self {
            Self::Main => LayoutSpec {
                root_width: 232,
                root_height: 84,
                previous_width: 48.0,
                play_width: 76.0,
                next_width: 48.0,
                secondary_height: 48.0,
                play_height: 76.0,
                gap: 14.0,
                classic_spacing: 18,
            },
            Self::Footer => LayoutSpec {
                root_width: 176,
                root_height: 64,
                previous_width: 40.0,
                play_width: 58.0,
                next_width: 40.0,
                secondary_height: 40.0,
                play_height: 58.0,
                gap: 8.0,
                classic_spacing: 7,
            },
        }
    }

    fn root_class(self) -> &'static str {
        match self {
            Self::Main => "expressive-transport-main",
            Self::Footer => "expressive-transport-footer",
        }
    }
}

pub(crate) struct ExpressiveTransport {
    root: gtk::Stack,
    classic: gtk::Box,
    motion: gtk::Fixed,
    previous: gtk::Button,
    play: gtk::Button,
    next: gtk::Button,
    play_icon: gtk::Image,
    variant: TransportVariant,
    enabled: Cell<bool>,
    mounted_expressive: Cell<Option<bool>>,
    playing: Cell<bool>,
    hovered: Cell<Option<PressTarget>>,
    current_geometry: Cell<Geometry>,
    animation_source: RefCell<Option<glib::SourceId>>,
}

impl ExpressiveTransport {
    pub(crate) fn new(
        variant: TransportVariant,
        previous: &gtk::Button,
        play: &gtk::Button,
        next: &gtk::Button,
        play_icon: &gtk::Image,
        effects_enabled: bool,
    ) -> Rc<Self> {
        let spec = variant.spec();
        // Mouse clicks must not leave GTK's generic focus halo on the
        // animated surfaces. Keyboard navigation remains available.
        for button in [previous, play, next] {
            button.set_focus_on_click(false);
        }

        let root = gtk::Stack::new();
        root.set_transition_type(gtk::StackTransitionType::None);
        root.set_hhomogeneous(false);
        root.set_vhomogeneous(false);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::Center);
        root.add_css_class("transport-presentation-stack");
        root.add_css_class(variant.root_class());

        let classic = gtk::Box::new(gtk::Orientation::Horizontal, spec.classic_spacing);
        classic.set_halign(gtk::Align::Center);
        classic.set_valign(gtk::Align::Center);
        classic.add_css_class("classic-transport-core");

        let motion = gtk::Fixed::new();
        motion.set_size_request(spec.root_width, spec.root_height);
        motion.set_halign(gtk::Align::Center);
        motion.set_valign(gtk::Align::Center);
        motion.set_overflow(gtk::Overflow::Visible);
        motion.add_css_class("expressive-transport-motion");

        root.add_named(&classic, Some("classic"));
        root.add_named(&motion, Some("expressive"));

        let base = Self::base_geometry(spec);
        let transport = Rc::new(Self {
            root,
            classic,
            motion,
            previous: previous.clone(),
            play: play.clone(),
            next: next.clone(),
            play_icon: play_icon.clone(),
            variant,
            enabled: Cell::new(false),
            mounted_expressive: Cell::new(None),
            playing: Cell::new(false),
            hovered: Cell::new(None),
            current_geometry: Cell::new(base),
            animation_source: RefCell::new(None),
        });

        Self::install_hover(&transport, &transport.previous, PressTarget::Previous);
        Self::install_hover(&transport, &transport.play, PressTarget::Play);
        Self::install_hover(&transport, &transport.next, PressTarget::Next);

        {
            let weak = Rc::downgrade(&transport);
            transport.previous.connect_clicked(move |_| {
                if let Some(transport) = weak.upgrade() {
                    transport.animate_press(PressTarget::Previous);
                }
            });
        }

        {
            let weak = Rc::downgrade(&transport);
            transport.next.connect_clicked(move |_| {
                if let Some(transport) = weak.upgrade() {
                    transport.animate_press(PressTarget::Next);
                }
            });
        }

        {
            let weak = Rc::downgrade(&transport);
            transport
                .play_icon
                .connect_notify_local(Some("icon-name"), move |_, _| {
                    if let Some(transport) = weak.upgrade() {
                        transport.sync_playing_state(true);
                    }
                });
        }

        transport.sync_playing_state(false);
        transport.set_effects_enabled(effects_enabled);
        transport
    }

    pub(crate) fn root(&self) -> &gtk::Stack {
        &self.root
    }

    pub(crate) fn set_effects_enabled(self: &Rc<Self>, enabled: bool) {
        if self.mounted_expressive.get() == Some(enabled) {
            self.enabled.set(enabled);
            if enabled {
                self.update_state_classes();
                self.apply_rest_geometry();
            }
            return;
        }

        self.stop_animation();
        self.hovered.set(None);

        if let Some(was_expressive) = self.mounted_expressive.get() {
            if was_expressive {
                self.motion.remove(&self.previous);
                self.motion.remove(&self.play);
                self.motion.remove(&self.next);
            } else {
                self.classic.remove(&self.previous);
                self.classic.remove(&self.play);
                self.classic.remove(&self.next);
            }
        }

        self.enabled.set(enabled);
        self.mounted_expressive.set(Some(enabled));

        if enabled {
            self.install_expressive_classes();
            self.motion.put(&self.previous, 0.0, 0.0);
            self.motion.put(&self.play, 0.0, 0.0);
            self.motion.put(&self.next, 0.0, 0.0);
            self.root.set_visible_child_name("expressive");
            self.update_state_classes();
            self.apply_rest_geometry();
        } else {
            self.remove_expressive_classes();
            for button in [&self.previous, &self.play, &self.next] {
                button.set_size_request(-1, -1);
                button.set_halign(gtk::Align::Center);
                button.set_valign(gtk::Align::Center);
            }
            self.classic.append(&self.previous);
            self.classic.append(&self.play);
            self.classic.append(&self.next);
            self.root.set_visible_child_name("classic");
        }
    }

    fn install_hover(this: &Rc<Self>, button: &gtk::Button, target: PressTarget) {
        let controller = gtk::EventControllerMotion::new();

        {
            let weak = Rc::downgrade(this);
            controller.connect_enter(move |_, _, _| {
                let Some(transport) = weak.upgrade() else {
                    return;
                };
                transport.hovered.set(Some(target));
                if transport.enabled.get() && transport.animation_source.borrow().is_none() {
                    transport.apply_rest_geometry();
                }
            });
        }

        {
            let weak = Rc::downgrade(this);
            controller.connect_leave(move |_| {
                let Some(transport) = weak.upgrade() else {
                    return;
                };
                if transport.hovered.get() == Some(target) {
                    transport.hovered.set(None);
                }
                if transport.enabled.get() && transport.animation_source.borrow().is_none() {
                    transport.apply_rest_geometry();
                }
            });
        }

        button.add_controller(controller);
    }

    fn install_expressive_classes(&self) {
        self.root.add_css_class("expressive-transport-enabled");
        // The main player's pre-0.3 shell classes contain their own focus and
        // shadow treatment. Detach them while the fixed-slot Expressive
        // component owns the surface, preventing the legacy cross-shaped glow.
        if self.variant == TransportVariant::Main {
            self.play.remove_css_class("shell-play-button");
            self.play.remove_css_class("player-primary-control");
        }

        self.previous
            .add_css_class("expressive-transport-secondary");
        self.play.add_css_class("expressive-transport-primary");
        self.next.add_css_class("expressive-transport-secondary");

        self.previous.add_css_class("expressive-previous");
        self.play.add_css_class("expressive-play");
        self.next.add_css_class("expressive-next");
    }

    fn remove_expressive_classes(&self) {
        self.root.remove_css_class("expressive-transport-enabled");

        for (button, classes) in [
            (
                &self.previous,
                [
                    "expressive-transport-secondary",
                    "expressive-previous",
                    "is-playing",
                    "is-resting",
                ],
            ),
            (
                &self.play,
                [
                    "expressive-transport-primary",
                    "expressive-play",
                    "is-playing",
                    "is-resting",
                ],
            ),
            (
                &self.next,
                [
                    "expressive-transport-secondary",
                    "expressive-next",
                    "is-playing",
                    "is-resting",
                ],
            ),
        ] {
            for class_name in classes {
                button.remove_css_class(class_name);
            }
        }

        // Restore the exact classic player classes when the preference is
        // disabled, preserving the original fallback requested by the user.
        if self.variant == TransportVariant::Main {
            self.play.add_css_class("shell-play-button");
            self.play.add_css_class("player-primary-control");
        }
    }

    fn sync_playing_state(self: &Rc<Self>, animate: bool) {
        let playing =
            self.play_icon.icon_name().as_deref() == Some("media-playback-pause-symbolic");
        let changed = self.playing.replace(playing) != playing;

        self.update_state_classes();

        if changed && animate && self.enabled.get() {
            self.animate_press(PressTarget::Play);
        }
    }

    fn update_state_classes(&self) {
        if !self.enabled.get() {
            return;
        }

        self.play.remove_css_class("is-playing");
        self.play.remove_css_class("is-resting");
        self.play.add_css_class(if self.playing.get() {
            "is-playing"
        } else {
            "is-resting"
        });
    }

    fn base_geometry(spec: LayoutSpec) -> Geometry {
        Geometry {
            widths: [spec.previous_width, spec.play_width, spec.next_width],
            heights: [
                spec.secondary_height,
                spec.play_height,
                spec.secondary_height,
            ],
            gaps: [spec.gap, spec.gap],
        }
    }
    // Hover never changes geometry. The only size/position animation is the
    // coordinated PixelPlayer-style response triggered by an actual click.
    fn rest_geometry(&self) -> Geometry {
        Self::base_geometry(self.variant.spec())
    }

    fn peak_geometry(&self, target: PressTarget) -> Geometry {
        let spec = self.variant.spec();
        let mut geometry = Self::base_geometry(spec);

        match (self.variant, target) {
            (TransportVariant::Main, PressTarget::Play) => {
                geometry.widths = [42.0, 104.0, 42.0];
                geometry.heights = [46.0, 79.0, 46.0];
                geometry.gaps = [10.0, 10.0];
            }
            (TransportVariant::Footer, PressTarget::Play) => {
                geometry.widths = [34.0, 82.0, 34.0];
                geometry.heights = [38.0, 61.0, 38.0];
                geometry.gaps = [6.0, 6.0];
            }
            (TransportVariant::Main, PressTarget::Previous) => {
                geometry.widths = [72.0, 64.0, 42.0];
                geometry.heights = [51.0, 72.0, 45.0];
                geometry.gaps = [10.0, 10.0];
            }
            (TransportVariant::Footer, PressTarget::Previous) => {
                geometry.widths = [58.0, 50.0, 34.0];
                geometry.heights = [43.0, 54.0, 37.0];
                geometry.gaps = [6.0, 6.0];
            }
            (TransportVariant::Main, PressTarget::Next) => {
                geometry.widths = [42.0, 64.0, 72.0];
                geometry.heights = [45.0, 72.0, 51.0];
                geometry.gaps = [10.0, 10.0];
            }
            (TransportVariant::Footer, PressTarget::Next) => {
                geometry.widths = [34.0, 50.0, 58.0];
                geometry.heights = [37.0, 54.0, 43.0];
                geometry.gaps = [6.0, 6.0];
            }
        }

        geometry
    }

    fn animate_press(self: &Rc<Self>, target: PressTarget) {
        if !self.enabled.get() {
            return;
        }

        if !adw::is_animations_enabled(&self.root) {
            self.apply_rest_geometry();
            return;
        }

        self.stop_animation();

        let from = self.current_geometry.get();
        let peak = self.peak_geometry(target);
        let weak = Rc::downgrade(self);
        let started = Instant::now();

        let source = glib::timeout_add_local(Duration::from_millis(16), move || {
            let Some(transport) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            if !transport.enabled.get() {
                return glib::ControlFlow::Break;
            }

            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let expand_end = EXPAND_MS as f64;
            let hold_end = (EXPAND_MS + HOLD_MS) as f64;
            let total_end = TOTAL_MS as f64;
            let rest = transport.rest_geometry();

            let geometry = if elapsed_ms <= expand_end {
                let local = (elapsed_ms / expand_end).clamp(0.0, 1.0);

                // Slower expansion with a light overshoot.
                from.lerp(peak, ease_out_back(local, 0.72))
            } else if elapsed_ms <= hold_end {
                // Briefly preserve the stretched/pushed pose.
                peak
            } else {
                let local = ((elapsed_ms - hold_end) / RETURN_MS as f64).clamp(0.0, 1.0);

                // Longer return with a subtle spring settle.
                peak.lerp(rest, ease_out_back(local, 1.05))
            };

            transport.apply_geometry(geometry);

            if elapsed_ms >= total_end {
                transport.animation_source.borrow_mut().take();
                transport.apply_rest_geometry();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });

        self.animation_source.borrow_mut().replace(source);
    }

    fn apply_rest_geometry(&self) {
        self.apply_geometry(self.rest_geometry());
    }

    fn apply_geometry(&self, geometry: Geometry) {
        if !self.enabled.get() {
            return;
        }

        let spec = self.variant.spec();
        let buttons = [&self.previous, &self.play, &self.next];
        let total_width = geometry.widths.iter().sum::<f64>() + geometry.gaps.iter().sum::<f64>();
        let mut x = (spec.root_width as f64 - total_width) / 2.0;

        for (index, button) in buttons.into_iter().enumerate() {
            let width = geometry.widths[index].round().max(1.0) as i32;
            let height = geometry.heights[index].round().max(1.0) as i32;
            let y = (spec.root_height as f64 - geometry.heights[index]) / 2.0;

            button.set_size_request(width, height);
            button.set_halign(gtk::Align::Center);
            button.set_valign(gtk::Align::Center);
            self.motion.move_(button, x, y);

            x += geometry.widths[index];
            if index < 2 {
                x += geometry.gaps[index];
            }
        }

        self.current_geometry.set(geometry);
    }

    fn stop_animation(&self) {
        if let Some(source) = self.animation_source.borrow_mut().take() {
            source.remove();
        }
    }
}

fn ease_out_back(value: f64, overshoot: f64) -> f64 {
    let shifted = value - 1.0;
    1.0 + (overshoot + 1.0) * shifted.powi(3) + overshoot * shifted.powi(2)
}
