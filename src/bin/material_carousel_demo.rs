#[path = "../ui/widgets/material_carousel/mod.rs"]
#[allow(dead_code)]
mod material_carousel;

use gtk::prelude::*;
use material_carousel::{
    layout_items, CarouselGeometryInput, FeaturedCardMetrics, MaterialCarousel,
    MaterialCarouselStrategy, MaterialCarouselVariant,
};
use std::{cell::Cell, env, rc::Rc};

const ITEM_SPACING: i32 = 12;

fn main() -> gtk::glib::ExitCode {
    if env::args().any(|arg| arg == "--print-hero-geometry") {
        print_hero_geometry_table();
        return gtk::glib::ExitCode::SUCCESS;
    }

    let app = gtk::Application::builder()
        .application_id("io.github.maylton.Nocky.MaterialCarouselDemo")
        .build();

    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &gtk::Application) {
    install_demo_css();

    let initial_metrics =
        FeaturedCardMetrics::for_viewport(f64::from(demo_requested_width().unwrap_or(920)));
    let carousel = MaterialCarousel::new(MaterialCarouselVariant::Hero);
    carousel.set_base_item_extent(ceil_to_i32(initial_metrics.large_width));
    carousel.set_spacing(ITEM_SPACING);
    carousel.set_vexpand(false);
    carousel.add_css_class("demo-carousel");

    let cards = Rc::new(
        (1..=12)
            .map(|index| {
                let card = demo_card(index);
                carousel.append(&card.frame);
                card
            })
            .collect::<Vec<_>>(),
    );
    apply_featured_metrics(&cards, initial_metrics);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroll.set_overlay_scrolling(false);
    scroll.set_hexpand(true);
    scroll.set_vexpand(false);
    if let Some(width) = demo_requested_width() {
        scroll.set_hexpand(false);
        scroll.set_size_request(width, -1);
    }
    scroll.set_min_content_height(ceil_to_i32(initial_metrics.card_height) + 32);
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
    if let Some(width) = demo_requested_width() {
        overlay.set_size_request(width, -1);
    }
    overlay.set_child(Some(&scroll));
    overlay.add_overlay(&debug_area);

    let variant = gtk::DropDown::from_strings(&["Hero", "MultiBrowse", "Uncontained"]);
    variant.set_selected(0);

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
        let scroll = scroll.clone();
        let cards = cards.clone();
        variant.connect_selected_notify(move |dropdown| {
            let selected = match dropdown.selected() {
                0 => MaterialCarouselVariant::Hero,
                2 => MaterialCarouselVariant::Uncontained,
                _ => MaterialCarouselVariant::MultiBrowse,
            };
            carousel.set_variant(selected);
            if selected == MaterialCarouselVariant::Hero {
                update_featured_demo_metrics(&carousel, &scroll, &cards);
            }
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
        let cards = cards.clone();
        scroll.connect_notify_local(Some("width"), move |scroll, _| {
            carousel.set_viewport_width(demo_viewport_width(scroll));
            if carousel.variant() == MaterialCarouselVariant::Hero {
                update_featured_demo_metrics(&carousel, scroll, &cards);
            }
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

    let (default_width, default_height) = demo_window_size();
    let title = format!("Material Carousel Demo {default_width}");
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(&title)
        .default_width(default_width)
        .default_height(default_height)
        .child(&root)
        .build();

    {
        let carousel = carousel.clone();
        let debug_area = debug_area.clone();
        let update_status = update_status.clone();
        let cards = cards.clone();
        window.connect_map(move |_| {
            carousel.set_viewport_width(demo_viewport_width(&scroll));
            update_featured_demo_metrics(&carousel, &scroll, &cards);
            debug_area.queue_draw();
            update_status();

            let carousel = carousel.clone();
            let debug_area = debug_area.clone();
            let update_status = update_status.clone();
            let scroll = scroll.clone();
            let cards = cards.clone();
            let attempts = Cell::new(0);
            scroll.add_tick_callback(move |scroll, _| {
                attempts.set(attempts.get() + 1);
                let width = demo_viewport_width(scroll);
                if width > 0 {
                    carousel.set_viewport_width(width);
                    update_featured_demo_metrics(&carousel, scroll, &cards);
                    debug_area.queue_draw();
                    update_status();
                    return gtk::glib::ControlFlow::Break;
                }

                if attempts.get() > 60 {
                    gtk::glib::ControlFlow::Break
                } else {
                    gtk::glib::ControlFlow::Continue
                }
            });
        });
    }

    update_status();
    window.present();
}

fn demo_window_size() -> (i32, i32) {
    let width = demo_requested_width().unwrap_or(920);
    let height = env::var("NOCKY_CAROUSEL_DEMO_HEIGHT")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|height| *height >= 300)
        .unwrap_or(360);
    (width, height)
}

fn demo_requested_width() -> Option<i32> {
    env::var("NOCKY_CAROUSEL_DEMO_WIDTH")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|width| *width >= 360)
}

fn demo_viewport_width(scroll: &gtk::ScrolledWindow) -> i32 {
    demo_requested_width().unwrap_or_else(|| scroll.width())
}

#[derive(Clone)]
struct FeaturedDemoCard {
    frame: gtk::Frame,
    artwork: gtk::DrawingArea,
}

fn demo_card(index: i32) -> FeaturedDemoCard {
    let artwork = gtk::DrawingArea::new();
    artwork.add_css_class("demo-card-artwork");
    artwork.set_draw_func(move |_, cr, width, height| {
        let hue = f64::from(index % 6) / 6.0;
        cr.set_source_rgb(0.18 + hue * 0.28, 0.20 + hue * 0.18, 0.32 + hue * 0.20);
        let _ = cr.paint();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.18);
        cr.arc(
            f64::from(width) * 0.72,
            f64::from(height) * 0.35,
            f64::from(width.min(height)) * 0.22,
            0.0,
            std::f64::consts::TAU,
        );
        let _ = cr.fill();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.18);
        cr.rectangle(
            0.0,
            f64::from(height) * 0.68,
            f64::from(width),
            f64::from(height) * 0.32,
        );
        let _ = cr.fill();
    });

    let action = gtk::Button::builder()
        .icon_name("media-playback-start-symbolic")
        .build();
    action.add_css_class("demo-card-action");
    action.set_halign(gtk::Align::End);
    action.set_valign(gtk::Align::Start);
    action.set_margin_top(12);
    action.set_margin_end(12);

    let artwork_overlay = gtk::Overlay::new();
    artwork_overlay.set_child(Some(&artwork));
    artwork_overlay.add_overlay(&action);

    let title = gtk::Label::new(Some(&format!("Card {index}")));
    title.set_xalign(0.0);
    title.set_single_line_mode(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("demo-card-title");

    let subtitle = gtk::Label::new(Some("Featured playlist"));
    subtitle.set_xalign(0.0);
    subtitle.set_single_line_mode(true);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("demo-card-subtitle");

    let detail = gtk::Label::new(Some("YouTube Music"));
    detail.set_xalign(0.0);
    detail.set_single_line_mode(true);
    detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
    detail.add_css_class("demo-card-detail");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.add_css_class("demo-card-copy");
    content.append(&title);
    content.append(&subtitle);
    content.append(&detail);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&artwork_overlay);
    body.append(&content);

    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&body));
    frame.add_css_class("demo-card");
    frame.add_css_class("material-carousel-hero-card");

    FeaturedDemoCard { frame, artwork }
}

fn update_featured_demo_metrics(
    carousel: &MaterialCarousel,
    scroll: &gtk::ScrolledWindow,
    cards: &[FeaturedDemoCard],
) {
    let width = demo_viewport_width(scroll);
    if width <= 1 {
        return;
    }
    let metrics = FeaturedCardMetrics::for_viewport(f64::from(width));
    carousel.set_base_item_extent(ceil_to_i32(metrics.large_width));
    scroll.set_min_content_height(ceil_to_i32(metrics.card_height) + 32);
    apply_featured_metrics(cards, metrics);
}

fn apply_featured_metrics(cards: &[FeaturedDemoCard], metrics: FeaturedCardMetrics) {
    let card_width = ceil_to_i32(metrics.large_width);
    let card_height = ceil_to_i32(metrics.card_height);
    let artwork_width = ceil_to_i32(metrics.artwork_width);
    let artwork_height = ceil_to_i32(metrics.artwork_height);

    for card in cards {
        card.frame.set_size_request(card_width, card_height);
        card.artwork.set_size_request(artwork_width, artwork_height);
    }
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
            min-width: 0;
            min-height: 0;
            border-radius: 24px;
            background: color-mix(in srgb, @accent_bg_color 18%, @window_bg_color);
            border: 1px solid color-mix(in srgb, @accent_color 28%, transparent);
        }
        .material-carousel-hero-card {
            min-width: 0;
            min-height: 0;
        }
        .demo-card-artwork {
            border-radius: 22px 22px 14px 14px;
        }
        .demo-card-copy {
            margin: 14px 16px 16px;
        }
        .demo-card-action {
            min-width: 44px;
            min-height: 44px;
            border-radius: 999px;
            padding: 0;
        }
        .demo-card-title {
            font-size: 19px;
            font-weight: 700;
        }
        .demo-card-subtitle,
        .demo-card-detail,
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

fn print_hero_geometry_table() {
    for viewport_width in [480.0, 760.0, 1000.0, 1400.0] {
        let metrics = FeaturedCardMetrics::for_viewport(viewport_width);
        let items = layout_items(CarouselGeometryInput {
            item_count: 12,
            viewport_width,
            scroll_offset: 0.0,
            base_item_width: metrics.large_width,
            spacing: f64::from(ITEM_SPACING),
            leading_padding: MaterialCarouselStrategy::Hero.leading_padding(viewport_width),
            variant: MaterialCarouselStrategy::Hero,
        });
        println!("viewport {viewport_width:.0}");
        println!("index\tlogical_x\tvisual_x\twidth\tstate\tcontent_offset");
        for (index, item) in items.iter().take(6).enumerate() {
            let logical_x = MaterialCarouselStrategy::Hero.leading_padding(viewport_width)
                + index as f64 * (metrics.large_width + f64::from(ITEM_SPACING));
            println!(
                "{index}\t{logical_x:.1}\t{:.1}\t{:.1}\t{:?}\t{:.1}",
                item.viewport_x, item.visible_width, item.state, item.content_offset
            );
        }
    }
}

fn ceil_to_i32(value: f64) -> i32 {
    value.ceil().clamp(1.0, i32::MAX as f64) as i32
}
