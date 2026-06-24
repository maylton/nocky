use gtk::{glib, prelude::*};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

/// Shared generation clock for cancel-safe UI transitions.
///
/// Starting a new transition invalidates every timeout from the previous one,
/// preventing stale metadata or artwork animations after rapid track changes.
#[derive(Clone, Default)]
pub(crate) struct TransitionClock {
    generation: Rc<Cell<u64>>,
}

impl TransitionClock {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn next(&self) -> u64 {
        let token = self.generation.get().wrapping_add(1);
        self.generation.set(token);
        token
    }

    pub(crate) fn fade<W>(
        &self,
        token: u64,
        widget: &W,
        from: f64,
        to: f64,
        delay_ms: u64,
        duration_ms: u64,
    ) where
        W: IsA<gtk::Widget> + Clone + 'static,
    {
        let generation = self.generation.clone();
        let widget = widget.clone().upcast::<gtk::Widget>();

        glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || {
            if generation.get() != token {
                return;
            }

            widget.set_opacity(from.clamp(0.0, 1.0));

            if duration_ms == 0 {
                widget.set_opacity(to.clamp(0.0, 1.0));
                return;
            }

            let started = Instant::now();
            let generation = generation.clone();
            glib::timeout_add_local(Duration::from_millis(16), move || {
                if generation.get() != token {
                    return glib::ControlFlow::Break;
                }

                let progress = (started.elapsed().as_secs_f64() / (duration_ms as f64 / 1000.0))
                    .clamp(0.0, 1.0);
                let eased = 1.0 - (1.0 - progress).powi(3);
                widget.set_opacity(from + (to - from) * eased);

                if progress >= 1.0 {
                    widget.set_opacity(to.clamp(0.0, 1.0));
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        });
    }

    pub(crate) fn after<F>(&self, token: u64, delay_ms: u64, callback: F)
    where
        F: FnOnce() + 'static,
    {
        let generation = self.generation.clone();
        glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || {
            if generation.get() == token {
                callback();
            }
        });
    }
}
