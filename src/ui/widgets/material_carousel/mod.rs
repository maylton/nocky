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
pub(crate) use keyline::KeylineState;
#[allow(unused_imports)]
pub(crate) use layout::MaterialCarouselLayout;
#[allow(unused_imports)]
pub(crate) use strategy::MaterialCarouselStrategy;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaterialCarouselVariant {
    Hero,
    MultiBrowse,
    Uncontained,
}

impl Default for MaterialCarouselVariant {
    fn default() -> Self {
        Self::MultiBrowse
    }
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
        child.as_ref().set_parent(self);
        self.queue_allocate();
    }

    pub(crate) fn remove(&self, child: &impl IsA<gtk::Widget>) {
        let child = child.as_ref();
        if child.parent().as_ref() == Some(self.upcast_ref::<gtk::Widget>()) {
            child.unparent();
            self.queue_allocate();
        }
    }

    pub(crate) fn set_adjustment(&self, adjustment: &gtk::Adjustment) {
        self.imp().adjustment.replace(Some(adjustment.clone()));
        self.queue_allocate();
    }

    pub(crate) fn set_viewport_width(&self, width: i32) {
        set_cell_if_changed(&self.imp().viewport_width, width.max(0), self);
    }

    pub(crate) fn set_base_item_extent(&self, extent: i32) {
        set_cell_if_changed(&self.imp().base_item_extent, extent.max(0), self);
    }

    pub(crate) fn set_spacing(&self, spacing: i32) {
        set_cell_if_changed(&self.imp().spacing, spacing.max(0), self);
    }

    pub(crate) fn set_variant(&self, variant: MaterialCarouselVariant) {
        if self.imp().variant.replace(variant) != variant {
            self.queue_allocate();
        }
    }
}

fn set_cell_if_changed(cell: &Cell<i32>, value: i32, widget: &impl IsA<gtk::Widget>) {
    if cell.replace(value) != value {
        widget.queue_allocate();
    }
}
