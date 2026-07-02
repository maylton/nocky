use gtk::{glib, graphene, gsk, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};

const MIN_VISIBLE_WIDTH: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MaterialCarouselItemGeometry {
    pub visible_width: f64,
    pub unmasked_width: f64,
    pub content_offset: f64,
    pub corner_radius: f64,
}

impl Default for MaterialCarouselItemGeometry {
    fn default() -> Self {
        Self {
            visible_width: MIN_VISIBLE_WIDTH,
            unmasked_width: MIN_VISIBLE_WIDTH,
            content_offset: 0.0,
            corner_radius: 0.0,
        }
    }
}

impl MaterialCarouselItemGeometry {
    fn normalized(self) -> Self {
        let unmasked_width = finite_positive(self.unmasked_width);
        let visible_width = finite_positive(self.visible_width)
            .max(MIN_VISIBLE_WIDTH)
            .min(unmasked_width);
        let max_offset = (unmasked_width - visible_width).max(0.0);
        let content_offset = finite_non_negative(self.content_offset).min(max_offset);
        let corner_radius = finite_non_negative(self.corner_radius);

        Self {
            visible_width,
            unmasked_width,
            content_offset,
            corner_radius,
        }
    }
}

glib::wrapper! {
    pub(crate) struct MaterialCarouselItem(ObjectSubclass<imp::MaterialCarouselItem>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl MaterialCarouselItem {
    pub(crate) fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let item: Self = glib::Object::new();
        item.set_child(Some(child));
        item
    }

    pub(crate) fn child(&self) -> Option<gtk::Widget> {
        self.imp().child.borrow().clone()
    }

    pub(crate) fn set_child(&self, child: Option<&impl IsA<gtk::Widget>>) {
        let imp = self.imp();

        if let Some(current) = imp.child.take() {
            current.unparent();
        }

        if let Some(child) = child {
            let child = child.as_ref();
            child.set_parent(self);
            imp.child.replace(Some(child.clone()));
        }

        self.queue_resize();
    }

    pub(crate) fn remove_child(&self) {
        self.set_child(None::<&gtk::Widget>);
    }

    pub(crate) fn set_geometry(
        &self,
        visible_width: f64,
        unmasked_width: f64,
        content_offset: f64,
        corner_radius: f64,
    ) {
        let geometry = MaterialCarouselItemGeometry {
            visible_width,
            unmasked_width,
            content_offset,
            corner_radius,
        }
        .normalized();

        let imp = self.imp();
        let changed = set_cell_if_changed(&imp.visible_width, geometry.visible_width)
            | set_cell_if_changed(&imp.unmasked_width, geometry.unmasked_width)
            | set_cell_if_changed(&imp.content_offset, geometry.content_offset)
            | set_cell_if_changed(&imp.corner_radius, geometry.corner_radius);

        if changed {
            self.queue_resize();
        }
    }

    pub(crate) fn geometry(&self) -> MaterialCarouselItemGeometry {
        self.imp().geometry()
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub(crate) struct MaterialCarouselItem {
        pub(super) child: RefCell<Option<gtk::Widget>>,
        pub(super) visible_width: Cell<f64>,
        pub(super) unmasked_width: Cell<f64>,
        pub(super) content_offset: Cell<f64>,
        pub(super) corner_radius: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MaterialCarouselItem {
        const NAME: &'static str = "NockyMaterialCarouselItem";
        type Type = super::MaterialCarouselItem;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for MaterialCarouselItem {
        fn constructed(&self) {
            self.parent_constructed();
            let geometry = super::MaterialCarouselItemGeometry::default();
            self.visible_width.set(geometry.visible_width);
            self.unmasked_width.set(geometry.unmasked_width);
            self.content_offset.set(geometry.content_offset);
            self.corner_radius.set(geometry.corner_radius);
        }

        fn dispose(&self) {
            if let Some(child) = self.child.take() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for MaterialCarouselItem {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => {
                    let width = ceil_to_i32(self.geometry().visible_width);
                    (width, width, -1, -1)
                }
                gtk::Orientation::Vertical => {
                    if let Some(child) = self.child.borrow().as_ref() {
                        child.measure(orientation, for_size)
                    } else {
                        (0, 0, -1, -1)
                    }
                }
                _ => (0, 0, -1, -1),
            }
        }

        fn size_allocate(&self, _width: i32, height: i32, baseline: i32) {
            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return;
            };
            let geometry = self.geometry();
            let child_width = ceil_to_i32(geometry.unmasked_width);
            let transform = gsk::Transform::new()
                .translate(&graphene::Point::new(-geometry.content_offset as f32, 0.0));

            child.allocate(child_width, height.max(0), baseline, Some(transform));
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(child) = self.child.borrow().as_ref().cloned() else {
                return;
            };
            let width = self.geometry().visible_width as f32;
            let height = self.obj().height().max(0) as f32;
            let bounds = graphene::Rect::new(0.0, 0.0, width, height);

            snapshot.push_clip(&bounds);
            self.obj().snapshot_child(&child, snapshot);
            snapshot.pop();
        }
    }

    impl MaterialCarouselItem {
        pub(super) fn geometry(&self) -> super::MaterialCarouselItemGeometry {
            super::MaterialCarouselItemGeometry {
                visible_width: self.visible_width.get(),
                unmasked_width: self.unmasked_width.get(),
                content_offset: self.content_offset.get(),
                corner_radius: self.corner_radius.get(),
            }
            .normalized()
        }
    }
}

fn set_cell_if_changed(cell: &Cell<f64>, value: f64) -> bool {
    if (cell.get() - value).abs() > f64::EPSILON {
        cell.set(value);
        true
    } else {
        false
    }
}

fn finite_positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        MIN_VISIBLE_WIDTH
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        any::Any,
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{mpsc, OnceLock},
        thread,
    };

    type GtkTestResult = Result<(), String>;
    type GtkTestFn = Box<dyn FnOnce() -> GtkTestResult + Send + 'static>;

    struct GtkTestJob {
        run: GtkTestFn,
        done: mpsc::Sender<GtkTestResult>,
    }

    static GTK_TEST_SENDER: OnceLock<Option<mpsc::Sender<GtkTestJob>>> = OnceLock::new();

    fn gtk_test_sender() -> Option<&'static mpsc::Sender<GtkTestJob>> {
        GTK_TEST_SENDER
            .get_or_init(|| {
                let (job_tx, job_rx) = mpsc::channel::<GtkTestJob>();
                let (ready_tx, ready_rx) = mpsc::channel();

                thread::spawn(move || {
                    let ready = gtk::init().is_ok();
                    let _ = ready_tx.send(ready);
                    if !ready {
                        return;
                    }

                    for job in job_rx {
                        let result = catch_unwind(AssertUnwindSafe(|| (job.run)()))
                            .map_err(panic_message)
                            .and_then(|result| result);
                        let _ = job.done.send(result);
                    }
                });

                if ready_rx.recv().unwrap_or(false) {
                    Some(job_tx)
                } else {
                    None
                }
            })
            .as_ref()
    }

    fn run_gtk_test<F>(test: F)
    where
        F: FnOnce() -> GtkTestResult + Send + 'static,
    {
        let Some(sender) = gtk_test_sender() else {
            return;
        };
        let (done_tx, done_rx) = mpsc::channel();
        sender
            .send(GtkTestJob {
                run: Box::new(test),
                done: done_tx,
            })
            .expect("GTK test worker should receive jobs");

        if let Err(message) = done_rx
            .recv()
            .expect("GTK test worker should return a result")
        {
            panic!("{message}");
        }
    }

    fn panic_message(payload: Box<dyn Any + Send>) -> String {
        if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else {
            "GTK test panicked".to_owned()
        }
    }

    fn label(text: &str) -> gtk::Label {
        gtk::Label::new(Some(text))
    }

    #[test]
    fn item_keeps_a_single_child() {
        run_gtk_test(|| {
            let first = label("first");
            let second = label("second");
            let item = MaterialCarouselItem::new(&first);

            assert_eq!(item.child(), Some(first.clone().upcast::<gtk::Widget>()));
            assert_eq!(first.parent(), Some(item.clone().upcast::<gtk::Widget>()));

            item.set_child(Some(&second));

            assert_eq!(first.parent(), None);
            assert_eq!(item.child(), Some(second.clone().upcast::<gtk::Widget>()));
            assert_eq!(second.parent(), Some(item.clone().upcast::<gtk::Widget>()));
            Ok(())
        });
    }

    #[test]
    fn child_can_be_removed_without_leaving_parent_links() {
        run_gtk_test(|| {
            let child = label("child");
            let item = MaterialCarouselItem::new(&child);

            item.remove_child();

            assert_eq!(item.child(), None);
            assert_eq!(child.parent(), None);
            Ok(())
        });
    }

    #[test]
    fn geometry_values_are_normalized() {
        let geometry = MaterialCarouselItemGeometry {
            visible_width: f64::NAN,
            unmasked_width: -42.0,
            content_offset: f64::INFINITY,
            corner_radius: -8.0,
        }
        .normalized();

        assert_eq!(geometry.visible_width, MIN_VISIBLE_WIDTH);
        assert_eq!(geometry.unmasked_width, MIN_VISIBLE_WIDTH);
        assert_eq!(geometry.content_offset, 0.0);
        assert_eq!(geometry.corner_radius, 0.0);
    }

    #[test]
    fn visible_width_is_limited_to_unmasked_width() {
        let geometry = MaterialCarouselItemGeometry {
            visible_width: 300.0,
            unmasked_width: 120.0,
            content_offset: 0.0,
            corner_radius: 12.0,
        }
        .normalized();

        assert_eq!(geometry.visible_width, 120.0);
        assert_eq!(geometry.unmasked_width, 120.0);
    }

    #[test]
    fn content_offset_is_limited_to_hidden_content_range() {
        let geometry = MaterialCarouselItemGeometry {
            visible_width: 80.0,
            unmasked_width: 120.0,
            content_offset: 64.0,
            corner_radius: 12.0,
        }
        .normalized();

        assert_eq!(geometry.content_offset, 40.0);
    }

    #[test]
    fn horizontal_measure_uses_visible_width() {
        run_gtk_test(|| {
            let item = MaterialCarouselItem::new(&label("child"));
            item.set_geometry(80.2, 120.0, 20.0, 12.0);

            let (minimum, natural, _, _) = item.measure(gtk::Orientation::Horizontal, -1);

            assert_eq!(minimum, 81);
            assert_eq!(natural, 81);
            Ok(())
        });
    }

    #[test]
    fn public_geometry_is_normalized_after_update() {
        run_gtk_test(|| {
            let item = MaterialCarouselItem::new(&label("child"));
            item.set_geometry(0.0, 80.0, 400.0, 8.0);

            assert_eq!(
                item.geometry(),
                MaterialCarouselItemGeometry {
                    visible_width: 1.0,
                    unmasked_width: 80.0,
                    content_offset: 79.0,
                    corner_radius: 8.0,
                }
            );
            Ok(())
        });
    }
}
