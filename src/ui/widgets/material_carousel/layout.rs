use super::{keyline::layout_items, MaterialCarousel, MaterialCarouselStrategy};
use gtk::{glib, graphene, gsk, prelude::*, subclass::prelude::*};

const LEADING_PADDING: f64 = 0.0;
const TRAILING_PADDING: f64 = 0.0;

glib::wrapper! {
    pub(crate) struct MaterialCarouselLayout(ObjectSubclass<imp::MaterialCarouselLayout>)
        @extends gtk::LayoutManager;
}

impl MaterialCarouselLayout {
    pub(crate) fn new() -> Self {
        glib::Object::new()
    }
}

pub(super) fn logical_extent(item_count: usize, base_item_width: f64, spacing: f64) -> f64 {
    let base_item_width = finite_positive(base_item_width);
    let spacing = finite_non_negative(spacing);
    let content =
        item_count as f64 * base_item_width + item_count.saturating_sub(1) as f64 * spacing;
    LEADING_PADDING + content + TRAILING_PADDING
}

fn strategy_from_variant(variant: super::MaterialCarouselVariant) -> MaterialCarouselStrategy {
    match variant {
        super::MaterialCarouselVariant::Hero => MaterialCarouselStrategy::Hero,
        super::MaterialCarouselVariant::MultiBrowse => MaterialCarouselStrategy::MultiBrowse,
        super::MaterialCarouselVariant::Uncontained => MaterialCarouselStrategy::Uncontained,
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

fn ceil_to_i32(value: f64) -> i32 {
    value.ceil().clamp(0.0, i32::MAX as f64) as i32
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub(crate) struct MaterialCarouselLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for MaterialCarouselLayout {
        const NAME: &'static str = "NockyMaterialCarouselLayout";
        type Type = super::MaterialCarouselLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for MaterialCarouselLayout {}

    impl LayoutManagerImpl for MaterialCarouselLayout {
        fn measure(
            &self,
            widget: &gtk::Widget,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let Ok(carousel) = widget.clone().downcast::<MaterialCarousel>() else {
                return (0, 0, -1, -1);
            };

            let imp = carousel.imp();
            match orientation {
                gtk::Orientation::Horizontal => {
                    let extent = ceil_to_i32(logical_extent(
                        imp.item_count.get(),
                        imp.base_item_width.get(),
                        imp.spacing.get(),
                    ));
                    (extent, extent, -1, -1)
                }
                gtk::Orientation::Vertical => {
                    let mut minimum = 0;
                    let mut natural = 0;
                    for item in imp.items.borrow().iter() {
                        let (child_min, child_nat, _, _) = item.measure(orientation, for_size);
                        minimum = minimum.max(child_min);
                        natural = natural.max(child_nat);
                    }
                    (minimum, natural, -1, -1)
                }
                _ => (0, 0, -1, -1),
            }
        }

        fn allocate(&self, widget: &gtk::Widget, _width: i32, height: i32, baseline: i32) {
            let Ok(carousel) = widget.clone().downcast::<MaterialCarousel>() else {
                return;
            };
            let imp = carousel.imp();
            let items = imp.items.borrow();
            if items.is_empty() {
                return;
            }

            let geometry = layout_items(super::super::keyline::CarouselGeometryInput {
                item_count: items.len(),
                viewport_width: finite_non_negative(imp.viewport_width.get()),
                scroll_offset: finite_non_negative(imp.scroll_offset.get()),
                base_item_width: finite_positive(imp.base_item_width.get()),
                spacing: finite_non_negative(imp.spacing.get()),
                leading_padding: LEADING_PADDING,
                variant: strategy_from_variant(imp.variant.get()),
            });

            for (item, geometry) in items.iter().zip(geometry.iter()) {
                item.set_geometry(
                    geometry.visible_width,
                    finite_positive(imp.base_item_width.get()),
                    geometry.content_offset,
                    geometry.corner_radius,
                );
                let transform = gsk::Transform::new()
                    .translate(&graphene::Point::new(geometry.content_x as f32, 0.0));
                item.allocate(
                    ceil_to_i32(geometry.visible_width),
                    height.max(0),
                    baseline,
                    Some(transform),
                );
            }
        }
    }
}
