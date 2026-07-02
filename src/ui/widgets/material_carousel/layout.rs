use gtk::glib;

glib::wrapper! {
    pub(crate) struct MaterialCarouselLayout(ObjectSubclass<imp::MaterialCarouselLayout>)
        @extends gtk::LayoutManager;
}

impl MaterialCarouselLayout {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }
}

mod imp {
    use gtk::{glib, subclass::prelude::*};

    #[derive(Default)]
    pub(crate) struct MaterialCarouselLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for MaterialCarouselLayout {
        const NAME: &'static str = "NockyMaterialCarouselLayout";
        type Type = super::MaterialCarouselLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for MaterialCarouselLayout {}
    impl LayoutManagerImpl for MaterialCarouselLayout {}
}
