use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

const FRAME_TIME: Duration = Duration::from_millis(16);
const MOTION_TIME_MS: f64 = 280.0;
const REVEAL_OFFSET: f64 = 10.0;
const DISMISS_OVERSHOOT: f64 = 4.0;

pub(crate) struct RevealBounce {
    source: Rc<RefCell<Option<glib::SourceId>>>,
    revealed: Cell<bool>,
}

impl RevealBounce {
    pub(crate) fn new(initial_revealed: bool) -> Rc<Self> {
        Rc::new(Self {
            source: Rc::new(RefCell::new(None)),
            revealed: Cell::new(initial_revealed),
        })
    }

    pub(crate) fn set_revealed<W>(
        &self,
        revealer: &gtk::Revealer,
        motion: &gtk::Fixed,
        child: &W,
        reveal: bool,
        hide_when_closed: bool,
    ) where
        W: IsA<gtk::Widget> + Clone + 'static,
    {
        let child = child.clone().upcast::<gtk::Widget>();

        if self.revealed.replace(reveal) == reveal {
            motion.move_(&child, 0.0, 0.0);
            child.set_opacity(1.0);
            revealer.set_reveal_child(reveal);
            if reveal {
                revealer.set_visible(true);
            } else if hide_when_closed {
                revealer.set_visible(false);
            }
            return;
        }

        if let Some(source) = self.source.borrow_mut().take() {
            source.remove();
        }

        if !adw::is_animations_enabled(revealer) {
            motion.move_(&child, 0.0, 0.0);
            child.set_opacity(1.0);
            revealer.set_visible(reveal || !hide_when_closed);
            revealer.set_reveal_child(reveal);
            return;
        }

        if reveal {
            revealer.set_visible(true);
            motion.move_(&child, -REVEAL_OFFSET, 0.0);
            child.set_opacity(0.94);
            revealer.set_reveal_child(true);
        } else {
            motion.move_(&child, 0.0, 0.0);
            child.set_opacity(1.0);
        }

        let revealer = revealer.clone();
        let motion = motion.clone();
        let source_slot = self.source.clone();
        let close_started = Cell::new(false);
        let started = Instant::now();

        let source = glib::timeout_add_local(FRAME_TIME, move || {
            let progress =
                (started.elapsed().as_secs_f64() * 1000.0 / MOTION_TIME_MS).clamp(0.0, 1.0);

            if reveal {
                let overshoot = 1.10;
                let shifted = progress - 1.0;
                let eased = 1.0 + (overshoot + 1.0) * shifted.powi(3) + overshoot * shifted.powi(2);
                let x = -REVEAL_OFFSET + REVEAL_OFFSET * eased;
                motion.move_(&child, x, 0.0);
                child.set_opacity(0.94 + 0.06 * progress);
            } else {
                if progress < 0.20 {
                    let local = progress / 0.20;
                    let x = DISMISS_OVERSHOOT * (local * std::f64::consts::FRAC_PI_2).sin();
                    motion.move_(&child, x, 0.0);
                } else {
                    if !close_started.replace(true) {
                        revealer.set_reveal_child(false);
                    }

                    let local = ((progress - 0.20) / 0.80).clamp(0.0, 1.0);
                    let x = DISMISS_OVERSHOOT - (REVEAL_OFFSET + DISMISS_OVERSHOOT) * local;
                    motion.move_(&child, x, 0.0);
                    child.set_opacity(1.0 - 0.05 * local);
                }
            }

            if progress >= 1.0 {
                motion.move_(&child, 0.0, 0.0);
                child.set_opacity(1.0);

                if !reveal {
                    revealer.set_reveal_child(false);
                    if hide_when_closed {
                        revealer.set_visible(false);
                    }
                }

                source_slot.borrow_mut().take();
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });

        self.source.borrow_mut().replace(source);
    }
}
