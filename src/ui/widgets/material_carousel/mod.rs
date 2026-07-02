//! Isolated Material Expressive carousel component.

mod imp;
mod item;
mod keyline;
mod layout;
mod strategy;

use gtk::{glib, prelude::*, subclass::prelude::*};
use std::cell::Cell;

#[allow(unused_imports)]
pub(crate) use item::MaterialCarouselItem;
#[allow(unused_imports)]
pub(crate) use keyline::{layout_items, CarouselGeometryInput, KeylineKind, KeylineState};
#[allow(unused_imports)]
pub(crate) use layout::MaterialCarouselLayout;
#[allow(unused_imports)]
pub(crate) use strategy::{FeaturedCardMetrics, MaterialCarouselStrategy};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MaterialCarouselVariant {
    Hero,
    #[default]
    MultiBrowse,
    Uncontained,
}

glib::wrapper! {
    pub(crate) struct MaterialCarousel(ObjectSubclass<imp::MaterialCarousel>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MaterialCarousel {
    pub(crate) fn new(variant: MaterialCarouselVariant) -> Self {
        let carousel: Self = glib::Object::new();
        carousel.set_variant(variant);
        carousel
    }

    pub(crate) fn append(&self, child: &impl IsA<gtk::Widget>) {
        let item = MaterialCarouselItem::new(child);
        item.set_parent(self);

        let imp = self.imp();
        {
            let mut items = imp.items.borrow_mut();
            items.push(item);
            imp.item_count.set(items.len());
        }
        self.queue_resize();
    }

    pub(crate) fn remove(&self, child: &impl IsA<gtk::Widget>) {
        let child = child.as_ref();
        let imp = self.imp();
        let mut items = imp.items.borrow_mut();
        let Some(position) = items.iter().position(|item| {
            item.upcast_ref::<gtk::Widget>() == child
                || item
                    .child()
                    .as_ref()
                    .is_some_and(|item_child| item_child == child)
        }) else {
            return;
        };

        let item = items.remove(position);
        imp.item_count.set(items.len());
        drop(items);

        item.unparent();
        self.queue_resize();
    }

    pub(crate) fn set_adjustment(&self, adjustment: &gtk::Adjustment) {
        let imp = self.imp();
        imp.disconnect_adjustment();
        imp.scroll_offset
            .set(finite_non_negative(adjustment.value()));

        let weak = self.downgrade();
        let handler = adjustment.connect_value_changed(move |adjustment| {
            let Some(carousel) = weak.upgrade() else {
                return;
            };
            carousel
                .imp()
                .scroll_offset
                .set(finite_non_negative(adjustment.value()));
            carousel.request_coalesced_layout();
        });

        imp.adjustment.replace(Some(adjustment.clone()));
        imp.adjustment_handler.replace(Some(handler));
        self.queue_allocate();
    }

    pub(crate) fn set_viewport_width(&self, width: i32) {
        set_cell_if_changed(
            &self.imp().viewport_width,
            finite_non_negative(width as f64),
            self,
        );
    }

    pub(crate) fn set_base_item_extent(&self, extent: i32) {
        set_cell_if_changed(
            &self.imp().base_item_width,
            finite_positive(extent as f64),
            self,
        );
    }

    pub(crate) fn set_spacing(&self, spacing: i32) {
        set_cell_if_changed(
            &self.imp().spacing,
            finite_non_negative(spacing as f64),
            self,
        );
    }

    pub(crate) fn set_variant(&self, variant: MaterialCarouselVariant) {
        if self.imp().variant.replace(variant) != variant {
            self.queue_allocate();
        }
    }

    pub(crate) fn item_count(&self) -> usize {
        self.imp().item_count.get()
    }

    pub(crate) fn scroll_offset(&self) -> f64 {
        self.imp().scroll_offset.get()
    }

    pub(crate) fn viewport_width(&self) -> f64 {
        self.imp().viewport_width.get()
    }

    pub(crate) fn variant(&self) -> MaterialCarouselVariant {
        self.imp().variant.get()
    }

    pub(crate) fn debug_keyline_positions(&self) -> Vec<f64> {
        let imp = self.imp();
        let strategy = match imp.variant.get() {
            MaterialCarouselVariant::Hero => MaterialCarouselStrategy::Hero,
            MaterialCarouselVariant::MultiBrowse => MaterialCarouselStrategy::MultiBrowse,
            MaterialCarouselVariant::Uncontained => MaterialCarouselStrategy::Uncontained,
        };

        KeylineState::for_strategy(
            strategy,
            finite_non_negative(imp.viewport_width.get()),
            finite_positive(imp.base_item_width.get()),
            finite_non_negative(imp.spacing.get()),
        )
        .keylines
        .iter()
        .map(|keyline| keyline.position)
        .collect()
    }

    fn request_coalesced_layout(&self) {
        let imp = self.imp();
        if imp.pending_layout.replace(true) {
            return;
        }

        let weak = self.downgrade();
        self.add_tick_callback(move |_, _| {
            let Some(carousel) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let imp = carousel.imp();
            imp.pending_layout.set(false);
            if let Some(layout) = carousel.layout_manager() {
                layout.layout_changed();
            }
            carousel.queue_allocate();
            glib::ControlFlow::Break
        });
    }
}

fn set_cell_if_changed(cell: &Cell<f64>, value: f64, widget: &impl IsA<gtk::Widget>) {
    if (cell.get() - value).abs() > f64::EPSILON {
        cell.set(value);
        if let Some(layout) = widget.layout_manager() {
            layout.layout_changed();
        }
        widget.queue_allocate();
    }
}

fn finite_positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn carousel_modules_do_not_use_forbidden_layout_techniques() {
        let source = [
            include_str!("imp.rs"),
            include_str!("layout.rs"),
            include_str!("mod.rs"),
        ]
        .join("\n");

        for forbidden in [
            format!("{}{}", "compute", "_bounds"),
            format!("{}{}", "Gtk", "Fixed"),
            format!("{}{}", "move", "_"),
            format!("{}{}", "set_margin", "_start"),
            format!("{}{}", "set_margin", "_end"),
            format!("{}{}", "set_margin", "_top"),
            format!("{}{}", "set_margin", "_bottom"),
            format!("{}{}", "nearest", "_item"),
            format!("{} {}", "nearest", "anchor"),
            format!("{}{}", "anchor", "_index"),
        ] {
            assert!(
                !source.contains(&forbidden),
                "forbidden pattern: {forbidden}"
            );
        }
    }
}
