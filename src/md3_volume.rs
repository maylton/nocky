use gtk::{gdk, glib, prelude::*};
use std::{cell::Cell, f64::consts::PI, rc::Rc};

pub(crate) struct Md3VolumeSlider {
    root: gtk::DrawingArea,
}

impl Md3VolumeSlider {
    pub(crate) fn new(model: &gtk::Adjustment) -> Self {
        let root = gtk::DrawingArea::new();
        root.set_size_request(116, 42);
        root.set_hexpand(false);
        root.set_vexpand(false);
        root.set_focusable(true);
        root.set_cursor_from_name(Some("pointer"));
        root.add_css_class("footer-volume-md3-canvas");

        {
            let model = model.clone();
            root.set_draw_func(move |widget, context, width, height| {
                draw_slider(
                    widget,
                    context,
                    width,
                    height,
                    model.value().clamp(0.0, 1.0),
                );
            });
        }

        {
            let weak_root = root.downgrade();
            model.connect_value_changed(move |_| {
                if let Some(root) = weak_root.upgrade() {
                    root.queue_draw();
                }
            });
        }

        {
            let click = gtk::GestureClick::new();
            click.set_button(gdk::BUTTON_PRIMARY);

            let model = model.clone();
            let weak_root = root.downgrade();
            click.connect_pressed(move |_, _, x, _| {
                if let Some(root) = weak_root.upgrade() {
                    set_model_from_x(&model, &root, x);
                    root.grab_focus();
                }
            });

            root.add_controller(click);
        }

        {
            let drag = gtk::GestureDrag::new();
            let origin = Rc::new(Cell::new(0.0_f64));

            {
                let origin = origin.clone();
                let model = model.clone();
                let weak_root = root.downgrade();

                drag.connect_drag_begin(move |_, x, _| {
                    origin.set(x);

                    if let Some(root) = weak_root.upgrade() {
                        set_model_from_x(&model, &root, x);
                    }
                });
            }

            {
                let origin = origin.clone();
                let model = model.clone();
                let weak_root = root.downgrade();

                drag.connect_drag_update(move |_, offset_x, _| {
                    if let Some(root) = weak_root.upgrade() {
                        set_model_from_x(&model, &root, origin.get() + offset_x);
                    }
                });
            }

            root.add_controller(drag);
        }

        {
            let scroll = gtk::EventControllerScroll::new(
                gtk::EventControllerScrollFlags::VERTICAL
                    | gtk::EventControllerScrollFlags::HORIZONTAL
                    | gtk::EventControllerScrollFlags::DISCRETE,
            );
            let model = model.clone();

            scroll.connect_scroll(move |_, dx, dy| {
                let direction = if dy.abs() >= dx.abs() { -dy } else { dx };

                if direction.abs() <= f64::EPSILON {
                    return glib::Propagation::Proceed;
                }

                model.set_value((model.value() + direction.signum() * 0.05).clamp(0.0, 1.0));
                glib::Propagation::Stop
            });

            root.add_controller(scroll);
        }

        Self { root }
    }

    pub(crate) fn widget(&self) -> &gtk::DrawingArea {
        &self.root
    }
}

fn set_model_from_x(model: &gtk::Adjustment, widget: &gtk::DrawingArea, x: f64) {
    const SIDE_PADDING: f64 = 8.0;

    let width = f64::from(widget.width().max(1));
    let available = (width - SIDE_PADDING * 2.0).max(1.0);
    let value = ((x - SIDE_PADDING) / available).clamp(0.0, 1.0);

    model.set_value(value);
}

fn draw_slider(
    widget: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    value: f64,
) {
    const SIDE_PADDING: f64 = 8.0;
    const TRACK_HEIGHT: f64 = 5.0;
    const THUMB_RADIUS: f64 = 5.0;

    let width = f64::from(width.max(1));
    let height = f64::from(height.max(1));
    let center_y = height / 2.0;
    let left = SIDE_PADDING;
    let right = (width - SIDE_PADDING).max(left + 1.0);
    let track_width = right - left;
    let thumb_x = left + track_width * value;

    // GTK 4.10+: use the widget foreground color for custom drawing.
    // CSS assigns @m3_primary to this canvas, so the slider follows the
    // active Material palette without deprecated StyleContext lookups.
    let primary = widget.color();

    rounded_rectangle(
        context,
        left,
        center_y - TRACK_HEIGHT / 2.0,
        track_width,
        TRACK_HEIGHT,
        TRACK_HEIGHT / 2.0,
    );
    set_source_rgba(context, &primary, 0.22);
    let _ = context.fill();

    let active_width = (thumb_x - left).max(TRACK_HEIGHT);
    rounded_rectangle(
        context,
        left,
        center_y - TRACK_HEIGHT / 2.0,
        active_width,
        TRACK_HEIGHT,
        TRACK_HEIGHT / 2.0,
    );
    set_source_rgba(context, &primary, 1.0);
    let _ = context.fill();

    context.arc(thumb_x, center_y, THUMB_RADIUS, 0.0, PI * 2.0);
    set_source_rgba(context, &primary, 1.0);
    let _ = context.fill();
}

fn rounded_rectangle(
    context: &gtk::cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);

    context.new_sub_path();
    context.arc(x + width - radius, y + radius, radius, -PI / 2.0, 0.0);
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        PI / 2.0,
    );
    context.arc(x + radius, y + height - radius, radius, PI / 2.0, PI);
    context.arc(x + radius, y + radius, radius, PI, PI * 3.0 / 2.0);
    context.close_path();
}

fn set_source_rgba(context: &gtk::cairo::Context, color: &gdk::RGBA, alpha_multiplier: f64) {
    context.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha()) * alpha_multiplier,
    );
}
