//! Material Expressive card and carousel contracts.
//!
//! Cards are content surfaces for a single subject. Carousels are horizontal
//! collections of visual cards. Besides applying semantic classes, this module
//! installs browser-independent Material 3 keyline masking behavior on GTK
//! scrolled windows. Every card keeps an immutable outer slot; a nested viewport
//! owns the changing visible mask so scrolling never feeds deformed geometry
//! back into the next keyline calculation.

use gtk::prelude::*;
use std::rc::Rc;

const CARD_VARIANT_CLASSES: &[&str] = &[
    "material-card-elevated",
    "material-card-filled",
    "material-card-outlined",
];

const CAROUSEL_VARIANT_CLASSES: &[&str] = &[
    "material-carousel-multi-browse",
    "material-carousel-hero",
    "material-carousel-uncontained",
];

const CAROUSEL_STATE_CLASSES: &[&str] = &[
    "material-carousel-item-large",
    "material-carousel-item-medium",
    "material-carousel-item-small",
    "material-carousel-item-leading",
    "material-carousel-item-trailing",
];

const CAROUSEL_ITEM_CLASS: &str = "home-card-context-overlay";
const CAROUSEL_INSTALLED_CLASS: &str = "material-carousel-motion-installed";
const CAROUSEL_SLOT_CLASS: &str = "material-carousel-slot";
const CAROUSEL_MASK_CLASS: &str = "material-carousel-mask";
const CAROUSEL_CONTENT_CLASS: &str = "material-carousel-content";

const FEATURED_OUTER_WIDTH: i32 = 220;
const COMPACT_OUTER_WIDTH: i32 = 168;
const TRACK_ROW_OUTER_WIDTH: i32 = 312;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialCardVariant {
    Elevated,
    Filled,
    Outlined,
}

impl MaterialCardVariant {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Elevated => "material-card-elevated",
            Self::Filled => "material-card-filled",
            Self::Outlined => "material-card-outlined",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialCarouselVariant {
    MultiBrowse,
    Hero,
    Uncontained,
}

impl MaterialCarouselVariant {
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::MultiBrowse => "material-carousel-multi-browse",
            Self::Hero => "material-carousel-hero",
            Self::Uncontained => "material-carousel-uncontained",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialCardSpec {
    pub variant: MaterialCardVariant,
}

impl MaterialCardSpec {
    pub const fn new(variant: MaterialCardVariant) -> Self {
        Self { variant }
    }

    pub fn css_classes(self) -> Vec<&'static str> {
        vec![self.variant.css_class()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialCarouselSpec {
    pub variant: MaterialCarouselVariant,
}

impl MaterialCarouselSpec {
    pub const fn new(variant: MaterialCarouselVariant) -> Self {
        Self { variant }
    }

    pub fn css_classes(self) -> Vec<&'static str> {
        vec![self.variant.css_class()]
    }
}

pub fn apply_material_card(widget: &impl IsA<gtk::Widget>, spec: MaterialCardSpec) {
    let widget = widget.as_ref();
    widget.add_css_class("material-card");

    for class_name in CARD_VARIANT_CLASSES {
        widget.remove_css_class(class_name);
    }

    for class_name in spec.css_classes() {
        widget.add_css_class(class_name);
    }
}

pub fn apply_material_carousel(widget: &impl IsA<gtk::Widget>, spec: MaterialCarouselSpec) {
    let widget = widget.as_ref();
    widget.add_css_class("material-carousel");

    for class_name in CAROUSEL_VARIANT_CLASSES {
        widget.remove_css_class(class_name);
    }

    for class_name in spec.css_classes() {
        widget.add_css_class(class_name);
    }

    if let Ok(scroll) = widget.clone().downcast::<gtk::ScrolledWindow>() {
        install_material_carousel_motion(&scroll, spec.variant);
    }
}

fn install_material_carousel_motion(
    scroll: &gtk::ScrolledWindow,
    requested_variant: MaterialCarouselVariant,
) {
    if scroll.has_css_class(CAROUSEL_INSTALLED_CLASS) {
        return;
    }

    scroll.add_css_class(CAROUSEL_INSTALLED_CLASS);
    scroll.set_kinetic_scrolling(true);

    let weak_scroll = scroll.downgrade();
    let update: Rc<dyn Fn()> = Rc::new(move || {
        let Some(scroll) = weak_scroll.upgrade() else {
            return;
        };
        update_material_carousel_masks(&scroll, requested_variant);
    });

    {
        let update = update.clone();
        scroll.connect_map(move |_| update());
    }
    {
        let update = update.clone();
        scroll.connect_notify_local(Some("width"), move |_, _| update());
    }
    {
        let update = update.clone();
        scroll
            .hadjustment()
            .connect_value_changed(move |_| update());
    }
}

fn update_material_carousel_masks(
    scroll: &gtk::ScrolledWindow,
    requested_variant: MaterialCarouselVariant,
) {
    let viewport_width = scroll.width();
    if viewport_width <= 1 {
        return;
    }

    let Some(child) = scroll.child() else {
        return;
    };

    let mut items = Vec::new();
    collect_descendants_with_css(&child, CAROUSEL_ITEM_CLASS, &mut items);
    if items.is_empty() {
        return;
    }

    let material_theme_active = widget_or_ancestor_has_css(scroll, "theme-material-expressive");
    let effective_variant = infer_carousel_variant(scroll, &items, requested_variant);
    let viewport_width = f64::from(viewport_width);

    for item in items {
        let base_width = carousel_item_base_width(&item);
        restore_stable_slot_geometry(&item, base_width);

        if !material_theme_active || effective_variant == MaterialCarouselVariant::Uncontained {
            reset_carousel_item(&item, base_width);
            continue;
        }

        let Some(bounds) = item.compute_bounds(scroll) else {
            continue;
        };
        let center_x = f64::from(bounds.x() + bounds.width() / 2.0);
        let base_width_f64 = f64::from(base_width);

        let (visible_width, focal_x) = match effective_variant {
            MaterialCarouselVariant::MultiBrowse => (
                multi_browse_visible_width(center_x, viewport_width, base_width_f64),
                viewport_width / 2.0,
            ),
            MaterialCarouselVariant::Hero => (
                hero_visible_width(center_x, viewport_width, base_width_f64),
                viewport_width * 0.42,
            ),
            MaterialCarouselVariant::Uncontained => (base_width_f64, viewport_width / 2.0),
        };

        let Some(mask) = ensure_carousel_mask(&item, base_width) else {
            continue;
        };
        apply_carousel_mask(
            &item,
            &mask,
            base_width,
            visible_width.round() as i32,
            center_x,
            focal_x,
        );
    }
}

fn infer_carousel_variant(
    scroll: &gtk::ScrolledWindow,
    items: &[gtk::Widget],
    requested_variant: MaterialCarouselVariant,
) -> MaterialCarouselVariant {
    if requested_variant == MaterialCarouselVariant::Uncontained
        || items
            .iter()
            .any(|item| widget_or_descendant_has_css(item, "home-track-card"))
    {
        return MaterialCarouselVariant::Uncontained;
    }

    if requested_variant == MaterialCarouselVariant::Hero
        || widget_or_ancestor_has_css(scroll, "home-section-featured")
        || items
            .iter()
            .any(|item| widget_or_descendant_has_css(item, "home-card-featured"))
    {
        MaterialCarouselVariant::Hero
    } else {
        MaterialCarouselVariant::MultiBrowse
    }
}

fn carousel_item_base_width(item: &gtk::Widget) -> i32 {
    if widget_or_descendant_has_css(item, "home-track-card") {
        TRACK_ROW_OUTER_WIDTH
    } else if widget_or_descendant_has_css(item, "home-card-featured") {
        FEATURED_OUTER_WIDTH
    } else {
        COMPACT_OUTER_WIDTH
    }
}

fn multi_browse_visible_width(center_x: f64, viewport_width: f64, base_width: f64) -> f64 {
    let minimum_width = (base_width * 0.62).max(88.0);
    let edge_transition = (base_width * 0.92).min(viewport_width * 0.24).max(72.0);
    let distance_to_edge = center_x.min(viewport_width - center_x).max(0.0);
    let progress = smoothstep((distance_to_edge / edge_transition).clamp(0.0, 1.0));
    lerp(minimum_width, base_width, progress)
}

fn hero_visible_width(center_x: f64, viewport_width: f64, base_width: f64) -> f64 {
    let focal_x = viewport_width * 0.42;
    let minimum_width = (base_width * 0.58).max(96.0);
    let full_radius = base_width * 0.18;
    let transition_radius = (base_width * 1.35).max(full_radius + 1.0);
    let distance = (center_x - focal_x).abs();
    let collapse =
        smoothstep(((distance - full_radius) / (transition_radius - full_radius)).clamp(0.0, 1.0));
    lerp(base_width, minimum_width, collapse)
}

fn ensure_carousel_mask(item: &gtk::Widget, base_width: i32) -> Option<gtk::Viewport> {
    let overlay = item.clone().downcast::<gtk::Overlay>().ok()?;

    if item.has_css_class(CAROUSEL_SLOT_CLASS) {
        return overlay.child()?.downcast::<gtk::Viewport>().ok();
    }

    let main_child = overlay.child()?;
    overlay.set_child(None::<&gtk::Widget>);

    let content = gtk::Overlay::new();
    content.set_size_request(base_width, -1);
    content.set_hexpand(false);
    content.set_vexpand(false);
    content.set_halign(gtk::Align::Start);
    content.set_valign(gtk::Align::Start);
    content.add_css_class(CAROUSEL_CONTENT_CLASS);
    content.set_child(Some(&main_child));

    let mut overlay_children = Vec::new();
    let mut child = overlay.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        overlay_children.push(current);
    }

    for child in overlay_children {
        overlay.remove_overlay(&child);
        content.add_overlay(&child);
    }

    let mask = gtk::Viewport::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    mask.set_width_request(base_width);
    mask.set_hexpand(false);
    mask.set_vexpand(true);
    mask.set_halign(gtk::Align::Start);
    mask.set_valign(gtk::Align::Fill);
    mask.set_overflow(gtk::Overflow::Hidden);
    mask.add_css_class(CAROUSEL_MASK_CLASS);
    mask.set_child(Some(&content));

    overlay.set_child(Some(&mask));
    overlay.add_css_class(CAROUSEL_SLOT_CLASS);
    overlay.set_overflow(gtk::Overflow::Visible);

    Some(mask)
}

fn restore_stable_slot_geometry(item: &gtk::Widget, base_width: i32) {
    item.set_width_request(base_width);
    item.set_margin_start(0);
    item.set_margin_end(0);
    item.set_overflow(gtk::Overflow::Visible);
    item.remove_css_class(CAROUSEL_MASK_CLASS);

    for class_name in CAROUSEL_STATE_CLASSES {
        item.remove_css_class(class_name);
    }
}

fn configure_mask_adjustment(
    mask: &gtk::Viewport,
    base_width: i32,
    visible_width: i32,
    value: i32,
) {
    let Some(adjustment) = mask.hadjustment() else {
        return;
    };

    let visible_width = visible_width.clamp(1, base_width);
    let maximum_value = base_width.saturating_sub(visible_width);
    let value = value.clamp(0, maximum_value);

    adjustment.configure(
        f64::from(value),
        0.0,
        f64::from(base_width),
        1.0,
        f64::from(visible_width),
        f64::from(visible_width),
    );
}

fn apply_carousel_mask(
    item: &gtk::Widget,
    mask: &gtk::Viewport,
    base_width: i32,
    visible_width: i32,
    center_x: f64,
    focal_x: f64,
) {
    let visible_width = visible_width.clamp(1, base_width);
    let hidden_width = base_width.saturating_sub(visible_width);
    let ratio = f64::from(visible_width) / f64::from(base_width.max(1));

    restore_stable_slot_geometry(item, base_width);
    mask.set_overflow(gtk::Overflow::Hidden);
    mask.set_width_request(visible_width);

    for class_name in CAROUSEL_STATE_CLASSES {
        mask.remove_css_class(class_name);
    }

    if ratio >= 0.88 {
        mask.add_css_class("material-carousel-item-large");
    } else if ratio >= 0.64 {
        mask.add_css_class("material-carousel-item-medium");
    } else {
        mask.add_css_class("material-carousel-item-small");
    }

    if hidden_width == 0 {
        mask.set_halign(gtk::Align::Start);
        configure_mask_adjustment(mask, base_width, visible_width, 0);
        return;
    }

    if center_x < focal_x {
        mask.set_halign(gtk::Align::End);
        mask.add_css_class("material-carousel-item-leading");
        configure_mask_adjustment(mask, base_width, visible_width, hidden_width);
    } else {
        mask.set_halign(gtk::Align::Start);
        mask.add_css_class("material-carousel-item-trailing");
        configure_mask_adjustment(mask, base_width, visible_width, 0);
    }
}

fn reset_carousel_item(item: &gtk::Widget, base_width: i32) {
    restore_stable_slot_geometry(item, base_width);

    let Ok(overlay) = item.clone().downcast::<gtk::Overlay>() else {
        return;
    };
    let Some(mask) = overlay
        .child()
        .and_then(|child| child.downcast::<gtk::Viewport>().ok())
    else {
        return;
    };

    mask.set_width_request(base_width);
    mask.set_halign(gtk::Align::Start);
    mask.set_overflow(gtk::Overflow::Hidden);
    configure_mask_adjustment(&mask, base_width, base_width, 0);

    for class_name in CAROUSEL_STATE_CLASSES {
        mask.remove_css_class(class_name);
    }
}

fn collect_descendants_with_css(
    root: &gtk::Widget,
    class_name: &str,
    output: &mut Vec<gtk::Widget>,
) {
    if root.has_css_class(class_name) {
        output.push(root.clone());
        return;
    }

    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        collect_descendants_with_css(&current, class_name, output);
    }
}

fn widget_or_descendant_has_css(root: &gtk::Widget, class_name: &str) -> bool {
    if root.has_css_class(class_name) {
        return true;
    }

    let mut child = root.first_child();
    while let Some(current) = child {
        if widget_or_descendant_has_css(&current, class_name) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

fn widget_or_ancestor_has_css(widget: &impl IsA<gtk::Widget>, class_name: &str) -> bool {
    let mut current = Some(widget.as_ref().clone());
    while let Some(widget) = current {
        if widget.has_css_class(class_name) {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

fn lerp(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_variants_map_to_expected_classes() {
        let cases = [
            (MaterialCardVariant::Elevated, "material-card-elevated"),
            (MaterialCardVariant::Filled, "material-card-filled"),
            (MaterialCardVariant::Outlined, "material-card-outlined"),
        ];

        for (variant, expected) in cases {
            let spec = MaterialCardSpec::new(variant);
            let classes = spec.css_classes();
            assert_eq!(classes, vec![expected]);
        }
    }

    #[test]
    fn carousel_variants_map_to_expected_classes() {
        let cases = [
            (
                MaterialCarouselVariant::MultiBrowse,
                "material-carousel-multi-browse",
            ),
            (MaterialCarouselVariant::Hero, "material-carousel-hero"),
            (
                MaterialCarouselVariant::Uncontained,
                "material-carousel-uncontained",
            ),
        ];

        for (variant, expected) in cases {
            let spec = MaterialCarouselSpec::new(variant);
            let classes = spec.css_classes();
            assert_eq!(classes, vec![expected]);
        }
    }

    #[test]
    fn multi_browse_masks_edge_items_but_keeps_center_items_large() {
        let base = f64::from(COMPACT_OUTER_WIDTH);
        let viewport = 900.0;
        let edge = multi_browse_visible_width(8.0, viewport, base);
        let center = multi_browse_visible_width(viewport / 2.0, viewport, base);

        assert!(edge < base * 0.72);
        assert!((center - base).abs() < f64::EPSILON);
    }

    #[test]
    fn hero_has_one_strong_focal_keyline() {
        let base = f64::from(FEATURED_OUTER_WIDTH);
        let viewport = 900.0;
        let focal = viewport * 0.42;
        let focused = hero_visible_width(focal, viewport, base);
        let distant = hero_visible_width(focal + base * 1.5, viewport, base);

        assert!((focused - base).abs() < f64::EPSILON);
        assert!(distant < base * 0.68);
    }

    #[test]
    fn smoothstep_is_bounded_and_monotonic() {
        let start = smoothstep(0.0);
        let middle = smoothstep(0.5);
        let end = smoothstep(1.0);

        assert_eq!(start, 0.0);
        assert!(middle > start && middle < end);
        assert_eq!(end, 1.0);
    }
}
