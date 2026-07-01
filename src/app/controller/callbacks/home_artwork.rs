//! Incremental artwork updates for the mounted YouTube Home.
//!
//! Cover downloads complete after the first paint. Updating the existing
//! artwork stacks keeps the mounted card tree and scroll position intact.

use super::home_grid::find_home_stack;
use crate::youtube::YouTubeHomePage;
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

const ARTWORK_UPDATES_PER_IDLE: usize = 2;

type ArtworkUpdate = (glib::WeakRef<gtk::Stack>, PathBuf);

pub(super) fn install(root: &gtk::Stack) {
    let root = root.downgrade();
    YouTubeHomePage::install_cover_update_handler(move |page| {
        let Some(root) = root.upgrade() else {
            return false;
        };
        schedule_mounted_artwork_updates(&root.clone().upcast::<gtk::Widget>(), page)
    });
}

fn schedule_mounted_artwork_updates(root: &gtk::Widget, page: &YouTubeHomePage) -> bool {
    let Some(home_stack) = find_home_stack(root) else {
        return false;
    };
    let Some(content) = home_stack.visible_child() else {
        return false;
    };
    if !content.has_css_class("expressive-library-home") {
        return false;
    }

    let mut paths = cover_paths_by_title(page);
    if paths.is_empty() {
        return false;
    }

    let mut updates = VecDeque::new();
    collect_artwork_updates(&content, &mut paths, &mut updates);
    if updates.is_empty() {
        return false;
    }

    let updates = Rc::new(RefCell::new(updates));
    glib::timeout_add_local(Duration::from_millis(8), move || {
        for _ in 0..ARTWORK_UPDATES_PER_IDLE {
            let next = updates.borrow_mut().pop_front();
            let Some((artwork, path)) = next else {
                return glib::ControlFlow::Break;
            };
            if let Some(artwork) = artwork.upgrade() {
                apply_artwork_path(&artwork, &path);
            }
        }

        if updates.borrow().is_empty() {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
    true
}

fn cover_paths_by_title(page: &YouTubeHomePage) -> HashMap<String, VecDeque<PathBuf>> {
    let mut paths = HashMap::<String, VecDeque<PathBuf>>::new();
    for item in page.sections.iter().flat_map(|section| section.items.iter()) {
        let Some(path) = item.cached_cover() else {
            continue;
        };
        let key = normalize_title(&item.title);
        if !key.is_empty() {
            paths.entry(key).or_default().push_back(path.to_path_buf());
        }
    }
    paths
}

fn collect_artwork_updates(
    widget: &gtk::Widget,
    paths: &mut HashMap<String, VecDeque<PathBuf>>,
    updates: &mut VecDeque<ArtworkUpdate>,
) {
    if widget.has_css_class("home-card") {
        let title = find_label_text(widget, "collection-card-title");
        let artwork = find_artwork_stack(widget);
        if let (Some(title), Some(artwork)) = (title, artwork) {
            let key = normalize_title(&title);
            if let Some(candidates) = paths.get_mut(&key) {
                if let Some(path) = candidates.pop_front() {
                    updates.push_back((artwork.downgrade(), path));
                }
            }
        }
        return;
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        collect_artwork_updates(&current, paths, updates);
        child = current.next_sibling();
    }
}

fn find_label_text(widget: &gtk::Widget, css_class: &str) -> Option<String> {
    if widget.has_css_class(css_class) {
        if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
            return Some(label.text().to_string());
        }
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(text) = find_label_text(&current, css_class) {
            return Some(text);
        }
        child = current.next_sibling();
    }
    None
}

fn find_artwork_stack(widget: &gtk::Widget) -> Option<gtk::Stack> {
    if widget.has_css_class("collection-artwork") {
        if let Ok(stack) = widget.clone().downcast::<gtk::Stack>() {
            return Some(stack);
        }
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(stack) = find_artwork_stack(&current) {
            return Some(stack);
        }
        child = current.next_sibling();
    }
    None
}

fn apply_artwork_path(artwork: &gtk::Stack, path: &Path) {
    let Some(picture) = artwork
        .last_child()
        .and_then(|child| child.downcast::<gtk::Picture>().ok())
    else {
        return;
    };

    let file = gio::File::for_path(path);
    let Ok(texture) = gdk::Texture::from_file(&file) else {
        return;
    };

    picture.set_paintable(Some(&texture));
    artwork.set_visible_child_name("picture");
    artwork.remove_css_class("typed-collection-placeholder");
}

fn normalize_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_keys_ignore_spacing_and_case() {
        assert_eq!(normalize_title("  Daily   Mix 1 "), "daily mix 1");
        assert_eq!(normalize_title("ÁLBUNS PARA VOCÊ"), "álbuns para você");
    }
}
