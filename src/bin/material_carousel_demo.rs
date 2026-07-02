#[path = "../ui/widgets/material_carousel/mod.rs"]
#[allow(dead_code)]
mod material_carousel;

use gtk::prelude::*;
use material_carousel::{MaterialCarousel, MaterialCarouselVariant};
use std::{cell::Cell, rc::Rc};

const ITEM_WIDTH: i32 = 176;
const ITEM_HEIGHT: i32 = 152;
const ITEM_SPACING: i32 = 12;

fn main() -> gtk::glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id("io.github.maylton.Nocky.MaterialCarouselDemo")
        .build();

    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &gtk::Application) {
    install_demo_css();

    let carousel = MaterialCarousel::new(MaterialCarouselVariant::MultiBrowse);
    carousel.set_base_item_extent(ITEM_WIDTH);
    carousel.set_spacing(ITEM_SPACING);
    carousel.set_vexpand(false);
    carousel.add_css_class("demo-carousel");

    for index in 1..=12 {
        carousel.append(&demo_card(index));
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroll.set_overlay_scrolling(false);
    scroll.set_hexpand(true);
    scroll.set_vexpand(false);
    scroll.set_min_content_height(ITEM_HEIGHT + 32);
    scroll.set_child(Some(&carousel));

    let adjustment = scroll.hadjustment();
    carousel.set_adjustment(&adjustment);

    let debug_enabled = Rc::new(Cell::new(false));
    let debug_area = gtk::DrawingArea::new();
    debug_area.set_can_target(false);
    debug_area.set_hexpand(true);
    debug_area.set_vexpand(true);

    {
        let carousel = carousel.clone();
        let debug_enabled = debug_enabled.clone();
        debug_area.set_draw_func(move |_, cr, _, height| {
            if !debug_enabled.get() {
                return;
            }
            cr.set_source_rgba(0.1, 0.35, 1.0, 0.55);
            cr.set_line_width(1.0);
            for x in carousel.debug_keyline_positions() {
                cr.move_to(x, 0.0);
                cr.line_to(x, height as f64);
            }
            let _ = cr.stroke();
        });
    }

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scroll));
    overlay.add_overlay(&debug_area);

    let variant = gtk::DropDown::from_strings(&["Hero", "MultiBrowse", "Uncontained"]);
    variant.set_selected(1);

    let debug_button = gtk::ToggleButton::with_label("Keylines");
    let status = gtk::Label::new(None);
    status.set_xalign(0.0);
    status.add_css_class("demo-status");

    let update_status: Rc<dyn Fn()> = {
        let carousel = carousel.clone();
        let status = status.clone();
        Rc::new(move || {
            status.set_label(&format!(
                "viewport width: {:.0} px    scroll offset: {:.1} px    variant: {}",
                carousel.viewport_width(),
                carousel.scroll_offset(),
                variant_name(carousel.variant())
            ));
        })
    };

    {
        let carousel = carousel.clone();
        let debug_area = debug_area.clone();
        let update_status = update_status.clone();
        variant.connect_selected_notify(move |dropdown| {
            let selected = match dropdown.selected() {
                0 => MaterialCarouselVariant::Hero,
                2 => MaterialCarouselVariant::Uncontained,
                _ => MaterialCarouselVariant::MultiBrowse,
            };
            carousel.set_variant(selected);
            debug_area.queue_draw();
            update_status();
        });
    }

    {
        let debug_enabled = debug_enabled.clone();
        let debug_area = debug_area.clone();
        debug_button.connect_toggled(move |button| {
            debug_enabled.set(button.is_active());
            debug_area.queue_draw();
        });
    }

    {
        let carousel = carousel.clone();
        let debug_area = debug_area.clone();
        let update_status = update_status.clone();
        scroll.connect_notify_local(Some("width"), move |scroll, _| {
            carousel.set_viewport_width(scroll.width());
            debug_area.queue_draw();
            update_status();
        });
    }

    {
        let debug_area = debug_area.clone();
        let update_status = update_status.clone();
        adjustment.connect_value_changed(move |_| {
            debug_area.queue_draw();
            update_status();
        });
    }

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    controls.set_halign(gtk::Align::Start);
    controls.append(&variant);
    controls.append(&debug_button);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(18);
    root.set_margin_bottom(18);
    root.set_margin_start(18);
    root.set_margin_end(18);
    root.append(&controls);
    root.append(&overlay);
    root.append(&status);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Material Carousel Demo")
        .default_width(920)
        .default_height(360)
        .child(&root)
        .build();

    {
        let carousel = carousel.clone();
        let debug_area = debug_area.clone();
        let update_status = update_status.clone();
        window.connect_map(move |_| {
            carousel.set_viewport_width(scroll.width());
            debug_area.queue_draw();
            update_status();
        });
    }

    update_status();
    window.present();
}

fn demo_card(index: i32) -> gtk::Frame {
    let title = gtk::Label::new(Some(&format!("Card {index}")));
    title.set_xalign(0.0);
    title.add_css_class("demo-card-title");

    let subtitle = gtk::Label::new(Some("Material carousel item"));
    subtitle.set_xalign(0.0);
    subtitle.add_css_class("demo-card-subtitle");

    let number = gtk::Label::new(Some(&index.to_string()));
    number.set_xalign(0.0);
    number.add_css_class("demo-card-number");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.append(&number);
    content.append(&title);
    content.append(&subtitle);

    let frame = gtk::Frame::new(None);
    frame.set_size_request(ITEM_WIDTH, ITEM_HEIGHT);
    frame.set_child(Some(&content));
    frame.add_css_class("demo-card");
    frame
}

fn variant_name(variant: MaterialCarouselVariant) -> &'static str {
    match variant {
        MaterialCarouselVariant::Hero => "Hero",
        MaterialCarouselVariant::MultiBrowse => "MultiBrowse",
        MaterialCarouselVariant::Uncontained => "Uncontained",
    }
}

fn install_demo_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        "
        .demo-card {
            border-radius: 24px;
            background: color-mix(in srgb, @accent_bg_color 18%, @window_bg_color);
            border: 1px solid color-mix(in srgb, @accent_color 28%, transparent);
        }
        .demo-card-number {
            font-size: 42px;
            font-weight: 700;
        }
        .demo-card-title {
            font-size: 18px;
            font-weight: 700;
        }
        .demo-card-subtitle,
        .demo-status {
            color: color-mix(in srgb, @window_fg_color 68%, transparent);
        }
        ",
    );

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
