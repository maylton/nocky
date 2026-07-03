//! Material Expressive card and carousel contracts.
//!
//! Cards keep their normal, stable geometry while carousels add bounded
//! elastic feedback when the user reaches either horizontal edge.

use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

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

const CAROUSEL_INSTALLED_CLASS: &str = "material-carousel-motion-installed";
const CAROUSEL_ITEM_CLASS: &str = "home-card-context-overlay";
const CAROUSEL_SURFACE_CLASS: &str = "collection-card";
const SPRING_CLASS: &str = "material-carousel-edge-spring";
const SPRING_SURFACE_CLASS: &str = "material-carousel-edge-spring-surface";
const SPRING_DURATION_MICROS: f64 = 520_000.0;
const SPRING_CARD_LIMIT: usize = 3;
const SPRING_STRENGTHS: [f64; SPRING_CARD_LIMIT] = [1.0, 0.60, 0.32];

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
        install_material_carousel_spring(&scroll, spec.variant);
    }
}

#[derive(Clone)]
struct SpringCard {
    widget: gtk::Widget,
    surface: Option<gtk::Widget>,
    original_width_request: i32,
    original_surface_width_request: Option<i32>,
    base_width: i32,
    base_surface_width: i32,
}

type SpringCards = Rc<RefCell<Vec<SpringCard>>>;

fn install_material_carousel_spring(
    scroll: &gtk::ScrolledWindow,
    requested_variant: MaterialCarouselVariant,
) {
    if scroll.has_css_class(CAROUSEL_INSTALLED_CLASS) {
        return;
    }

    scroll.add_css_class(CAROUSEL_INSTALLED_CLASS);
    scroll.set_kinetic_scrolling(true);

    let ready = Rc::new(Cell::new(false));
    let active = Rc::new(Cell::new(false));
    let generation = Rc::new(Cell::new(0_u64));
    let active_cards: SpringCards = Rc::new(RefCell::new(Vec::new()));

    let trigger: Rc<dyn Fn(gtk::PositionType)> = {
        let scroll_weak = scroll.downgrade();
        let ready = ready.clone();
        let active = active.clone();
        let generation = generation.clone();
        let active_cards = active_cards.clone();

        Rc::new(move |position| {
            if !ready.get() || active.replace(true) {
                return;
            }

            let from_start = match position {
                gtk::PositionType::Left => true,
                gtk::PositionType::Right => false,
                _ => {
                    active.set(false);
                    return;
                }
            };

            let Some(scroll) = scroll_weak.upgrade() else {
                active.set(false);
                return;
            };

            if !widget_or_ancestor_has_css(
                scroll.upcast_ref::<gtk::Widget>(),
                "theme-material-expressive",
            ) {
                active.set(false);
                return;
            }

            let Some(child) = scroll.child() else {
                active.set(false);
                return;
            };

            if requested_variant == MaterialCarouselVariant::Uncontained
                || widget_or_descendant_has_css(&child, "home-track-card")
            {
                active.set(false);
                return;
            }

            let cards = carousel_edge_cards(&child, from_start, SPRING_CARD_LIMIT);
            if cards.is_empty() {
                active.set(false);
                return;
            }

            restore_spring_cards(&active_cards);
            {
                let mut stored = active_cards.borrow_mut();
                for widget in cards {
                    let surface = first_descendant_with_css(&widget, CAROUSEL_SURFACE_CLASS);
                    let original_width_request = widget.width_request();
                    let original_surface_width_request =
                        surface.as_ref().map(|surface| surface.width_request());
                    let base_width = widget.width().max(original_width_request).max(1);
                    let base_surface_width = surface
                        .as_ref()
                        .map(|surface| surface.width().max(surface.width_request()).max(1))
                        .unwrap_or(base_width);

                    widget.add_css_class(SPRING_CLASS);
                    if let Some(surface) = surface.as_ref() {
                        surface.add_css_class(SPRING_SURFACE_CLASS);
                    }

                    stored.push(SpringCard {
                        widget,
                        surface,
                        original_width_request,
                        original_surface_width_request,
                        base_width,
                        base_surface_width,
                    });
                }
            }

            let token = generation.get().wrapping_add(1);
            generation.set(token);
            let start_time = Cell::new(None::<i64>);
            let active = active.clone();
            let generation = generation.clone();
            let active_cards = active_cards.clone();

            scroll.add_tick_callback(move |scroll, frame_clock| {
                if generation.get() != token {
                    restore_spring_cards(&active_cards);
                    active.set(false);
                    return glib::ControlFlow::Break;
                }

                let now = frame_clock.frame_time();
                let start = match start_time.get() {
                    Some(start) => start,
                    None => {
                        start_time.set(Some(now));
                        now
                    }
                };
                let progress = ((now - start) as f64 / SPRING_DURATION_MICROS).clamp(0.0, 1.0);
                let displacement = spring_displacement(progress);

                {
                    let stored = active_cards.borrow();
                    for (index, card) in stored.iter().enumerate() {
                        let strength = SPRING_STRENGTHS
                            .get(index)
                            .copied()
                            .unwrap_or(*SPRING_STRENGTHS.last().unwrap());
                        let stretch = (displacement * strength).round() as i32;

                        card.widget
                            .set_width_request((card.base_width + stretch).max(1));
                        if let Some(surface) = card.surface.as_ref() {
                            surface.set_width_request((card.base_surface_width + stretch).max(1));
                        }
                    }
                }

                if !from_start {
                    let adjustment = scroll.hadjustment();
                    let end = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
                    adjustment.set_value(end);
                }

                if progress >= 1.0 {
                    restore_spring_cards(&active_cards);
                    active.set(false);
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        })
    };

    {
        let ready = ready.clone();
        scroll.connect_map(move |scroll| {
            ready.set(false);
            let ready = ready.clone();
            let weak_scroll = scroll.downgrade();
            glib::timeout_add_local_once(Duration::from_millis(180), move || {
                if weak_scroll.upgrade().is_some() {
                    ready.set(true);
                }
            });
        });
    }

    {
        let trigger = trigger.clone();
        scroll.connect_edge_reached(move |_, position| trigger(position));
    }

    {
        let trigger = trigger.clone();
        scroll.connect_edge_overshot(move |_, position| trigger(position));
    }

    {
        let adjustment = scroll.hadjustment();
        let last_value = Rc::new(Cell::new(adjustment.value()));
        let ready = ready.clone();
        let trigger = trigger.clone();

        adjustment.connect_value_changed(move |adjustment| {
            let value = adjustment.value();
            let previous = last_value.replace(value);

            if !ready.get() {
                return;
            }

            let lower = adjustment.lower();
            let upper = (adjustment.upper() - adjustment.page_size()).max(lower);
            const EDGE_EPSILON: f64 = 0.75;

            if value <= lower + EDGE_EPSILON && previous > value + EDGE_EPSILON {
                trigger(gtk::PositionType::Left);
            } else if value >= upper - EDGE_EPSILON && previous < value - EDGE_EPSILON {
                trigger(gtk::PositionType::Right);
            }
        });
    }

    {
        let ready = ready.clone();
        let active = active.clone();
        let generation = generation.clone();
        let active_cards = active_cards.clone();

        scroll.connect_unmap(move |_| {
            ready.set(false);
            generation.set(generation.get().wrapping_add(1));
            restore_spring_cards(&active_cards);
            active.set(false);
        });
    }
}

fn carousel_edge_cards(root: &gtk::Widget, from_start: bool, limit: usize) -> Vec<gtk::Widget> {
    let mut cards = Vec::new();
    collect_descendants_with_css(root, CAROUSEL_ITEM_CLASS, &mut cards);

    if from_start {
        cards.into_iter().take(limit).collect()
    } else {
        cards.into_iter().rev().take(limit).collect()
    }
}

fn collect_descendants_with_css(
    root: &gtk::Widget,
    class_name: &str,
    matches: &mut Vec<gtk::Widget>,
) {
    if root.has_css_class(class_name) {
        matches.push(root.clone());
    }

    let mut child = root.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        collect_descendants_with_css(&current, class_name, matches);
    }
}

fn first_descendant_with_css(root: &gtk::Widget, class_name: &str) -> Option<gtk::Widget> {
    if root.has_css_class(class_name) {
        return Some(root.clone());
    }

    let mut child = root.first_child();
    while let Some(current) = child {
        if let Some(found) = first_descendant_with_css(&current, class_name) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

fn widget_or_descendant_has_css(root: &gtk::Widget, class_name: &str) -> bool {
    first_descendant_with_css(root, class_name).is_some()
}

fn widget_or_ancestor_has_css(widget: &gtk::Widget, class_name: &str) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget.has_css_class(class_name) {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn restore_spring_cards(cards: &SpringCards) {
    for card in cards.borrow_mut().drain(..) {
        card.widget.set_width_request(card.original_width_request);
        card.widget.remove_css_class(SPRING_CLASS);

        if let Some(surface) = card.surface {
            surface.set_width_request(card.original_surface_width_request.unwrap_or(-1));
            surface.remove_css_class(SPRING_SURFACE_CLASS);
        }
    }
}

fn spring_displacement(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    if progress < 0.20 {
        24.0 * ease_out_cubic(progress / 0.20)
    } else if progress < 0.48 {
        lerp(24.0, -7.0, ease_in_out_cubic((progress - 0.20) / 0.28))
    } else if progress < 0.73 {
        lerp(-7.0, 4.0, ease_in_out_cubic((progress - 0.48) / 0.25))
    } else {
        lerp(4.0, 0.0, ease_out_cubic((progress - 0.73) / 0.27))
    }
}

fn lerp(from: f64, to: f64, progress: f64) -> f64 {
    from + (to - from) * progress.clamp(0.0, 1.0)
}

fn ease_out_cubic(progress: f64) -> f64 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
}

fn ease_in_out_cubic(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    if progress < 0.5 {
        4.0 * progress.powi(3)
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_card_variants_map_to_expected_classes() {
        assert_eq!(
            MaterialCardVariant::Elevated.css_class(),
            "material-card-elevated"
        );
        assert_eq!(
            MaterialCardVariant::Filled.css_class(),
            "material-card-filled"
        );
        assert_eq!(
            MaterialCardVariant::Outlined.css_class(),
            "material-card-outlined"
        );
    }

    #[test]
    fn material_carousel_variants_map_to_expected_classes() {
        assert_eq!(
            MaterialCarouselVariant::MultiBrowse.css_class(),
            "material-carousel-multi-browse"
        );
        assert_eq!(
            MaterialCarouselVariant::Hero.css_class(),
            "material-carousel-hero"
        );
        assert_eq!(
            MaterialCarouselVariant::Uncontained.css_class(),
            "material-carousel-uncontained"
        );
    }

    #[test]
    fn spring_curve_starts_and_finishes_at_rest() {
        assert!(spring_displacement(0.0).abs() < f64::EPSILON);
        assert!(spring_displacement(1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn spring_curve_has_positive_and_negative_overshoot() {
        assert!(spring_displacement(0.20) > 20.0);
        assert!(spring_displacement(0.48) < 0.0);
        assert!(spring_displacement(0.73) > 0.0);
    }
}
