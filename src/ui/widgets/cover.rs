//! Album artwork widget used by player and queue surfaces.

use crate::playback::transition::TransitionClock;
use gtk::gdk;
use gtk::prelude::*;
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Clone)]
pub(crate) struct CoverView {
    pub(crate) stack: gtk::Stack,
    picture: gtk::Picture,
    placeholder: gtk::Box,
    icon: gtk::Image,
    display_size: Rc<Cell<i32>>,
    current_path: Rc<RefCell<Option<PathBuf>>>,
    transition: TransitionClock,
}

impl CoverView {
    pub(crate) fn set_display_size(&self, size: i32) {
        let size = size.max(1);
        let previous_size = self.display_size.replace(size);

        if self.stack.width_request() != size || self.stack.height_request() != size {
            self.stack.set_size_request(size, size);
            self.picture.set_size_request(size, size);
            self.placeholder.set_size_request(size, size);
            self.icon.set_pixel_size((f64::from(size) * 0.30) as i32);
        }

        if previous_size != size {
            let current_path = self.current_path.borrow().clone();
            self.set_path_immediate(current_path.as_deref());
        }
    }

    pub(crate) fn set_path(&self, path: Option<&Path>) {
        let path = path.map(Path::to_path_buf);
        self.current_path.replace(path.clone());

        if !adw::is_animations_enabled(&self.stack) {
            self.set_path_immediate(path.as_deref());
            self.stack.set_opacity(1.0);
            return;
        }

        let token = self.transition.next();
        self.transition
            .fade(token, &self.stack, self.stack.opacity(), 0.0, 0, 105);

        let cover = self.clone();
        self.transition.after(token, 116, move || {
            cover.set_path_immediate(path.as_deref());
            cover.transition.fade(token, &cover.stack, 0.0, 1.0, 0, 205);
        });
    }

    pub(crate) fn set_path_immediate(&self, path: Option<&Path>) {
        let Some(path) = path.filter(|path| path.is_file()) else {
            self.picture.set_paintable(None::<&gdk::Texture>);
            self.stack.set_visible_child_name("placeholder");
            return;
        };

        match square_cover_pixbuf(path, self.display_size.get()) {
            Some(pixbuf) => {
                let texture = gdk::Texture::for_pixbuf(&pixbuf);
                self.picture.set_paintable(Some(&texture));
                self.stack.set_visible_child_name("picture");
            }
            None => {
                eprintln!("Could not load cover {}", path.display());
                self.picture.set_paintable(None::<&gdk::Texture>);
                self.stack.set_visible_child_name("placeholder");
            }
        }
    }
}

fn square_cover_pixbuf(path: &Path, size: i32) -> Option<gdk_pixbuf::Pixbuf> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    let width = pixbuf.width();
    let height = pixbuf.height();
    if width <= 0 || height <= 0 {
        return None;
    }

    let side = width.min(height);
    let x = (width - side) / 2;
    let y = (height - side) / 2;
    let cropped = pixbuf.new_subpixbuf(x, y, side, side);
    cropped.scale_simple(size, size, gdk_pixbuf::InterpType::Bilinear)
}

pub(crate) fn build_cover(size: i32) -> CoverView {
    let icon = gtk::Image::from_icon_name("audio-x-generic-symbolic");
    icon.set_pixel_size((size as f64 * 0.30) as i32);
    icon.add_css_class("cover-icon");
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    icon.set_hexpand(true);
    icon.set_vexpand(true);

    let placeholder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    placeholder.set_width_request(size);
    placeholder.set_height_request(size);
    placeholder.set_halign(gtk::Align::Center);
    placeholder.set_valign(gtk::Align::Center);
    placeholder.set_hexpand(false);
    placeholder.set_vexpand(false);
    placeholder.append(&icon);

    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_can_shrink(true);
    picture.set_width_request(size);
    picture.set_height_request(size);
    picture.set_halign(gtk::Align::Center);
    picture.set_valign(gtk::Align::Center);
    picture.add_css_class("cover-picture");

    let stack = gtk::Stack::new();
    stack.set_width_request(size);
    stack.set_height_request(size);
    stack.set_halign(gtk::Align::Center);
    stack.set_valign(gtk::Align::Center);
    stack.set_hexpand(false);
    stack.set_vexpand(false);
    stack.set_overflow(gtk::Overflow::Hidden);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(180);
    stack.add_named(&placeholder, Some("placeholder"));
    stack.add_named(&picture, Some("picture"));
    stack.set_visible_child_name("placeholder");
    stack.add_css_class("album-cover");
    if size <= 64 {
        stack.add_css_class("mini-cover");
    }

    CoverView {
        stack,
        picture,
        placeholder,
        icon,
        display_size: Rc::new(Cell::new(size)),
        current_path: Rc::new(RefCell::new(None)),
        transition: TransitionClock::new(),
    }
}
