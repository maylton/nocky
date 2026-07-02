use super::MaterialCarouselVariant;
use gtk::{glib, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};

#[derive(Default)]
pub(crate) struct MaterialCarousel {
    pub(super) adjustment: RefCell<Option<gtk::Adjustment>>,
    pub(super) viewport_width: Cell<i32>,
    pub(super) base_item_extent: Cell<i32>,
    pub(super) spacing: Cell<i32>,
    pub(super) variant: Cell<MaterialCarouselVariant>,
}

#[glib::object_subclass]
impl ObjectSubclass for MaterialCarousel {
    const NAME: &'static str = "NockyMaterialCarousel";
    type Type = super::MaterialCarousel;
    type ParentType = gtk::Widget;
}

impl ObjectImpl for MaterialCarousel {
    fn dispose(&self) {
        while let Some(child) = self.obj().first_child() {
            child.unparent();
        }
        self.adjustment.take();
    }
}

impl WidgetImpl for MaterialCarousel {}
