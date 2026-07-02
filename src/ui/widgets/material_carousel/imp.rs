use super::{MaterialCarouselItem, MaterialCarouselLayout, MaterialCarouselVariant};
use gtk::{glib, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};

#[derive(Default)]
pub(crate) struct MaterialCarousel {
    pub(super) adjustment: RefCell<Option<gtk::Adjustment>>,
    pub(super) adjustment_handler: RefCell<Option<glib::SignalHandlerId>>,
    pub(super) items: RefCell<Vec<MaterialCarouselItem>>,
    pub(super) viewport_width: Cell<f64>,
    pub(super) base_item_width: Cell<f64>,
    pub(super) spacing: Cell<f64>,
    pub(super) scroll_offset: Cell<f64>,
    pub(super) item_count: Cell<usize>,
    pub(super) pending_layout: Cell<bool>,
    pub(super) variant: Cell<MaterialCarouselVariant>,
}

#[glib::object_subclass]
impl ObjectSubclass for MaterialCarousel {
    const NAME: &'static str = "NockyMaterialCarousel";
    type Type = super::MaterialCarousel;
    type ParentType = gtk::Widget;
}

impl ObjectImpl for MaterialCarousel {
    fn constructed(&self) {
        self.parent_constructed();
        self.base_item_width.set(1.0);
        self.spacing.set(0.0);
        self.obj()
            .set_layout_manager(Some(MaterialCarouselLayout::new()));
    }

    fn dispose(&self) {
        self.disconnect_adjustment();
        for item in self.items.take() {
            item.unparent();
        }
        self.item_count.set(0);
    }
}

impl WidgetImpl for MaterialCarousel {}

impl MaterialCarousel {
    pub(super) fn disconnect_adjustment(&self) {
        if let Some(adjustment) = self.adjustment.take() {
            if let Some(handler) = self.adjustment_handler.take() {
                adjustment.disconnect(handler);
            }
        }
    }
}
