use super::super::AppController;
use crate::{
    browser::BrowserRoute,
    youtube::{queue_library_cache_save, YouTubeItem},
};
use gtk::glib;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

const REVALIDATION_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(super) fn install(controller: &Rc<AppController>) {
    let weak = Rc::downgrade(controller);
    let active_route = Rc::new(RefCell::new(None::<String>));
    let revalidated = Rc::new(RefCell::new(HashSet::<String>::new()));
    let fallbacks = Rc::new(RefCell::new(HashMap::<String, Vec<YouTubeItem>>::new()));

    glib::timeout_add_local(REVALIDATION_POLL_INTERVAL, move || {
        let Some(controller) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let route = controller.browser.route();
        let BrowserRoute::YouTubePlaylist { title, browse_id } = route else {
            active_route.borrow_mut().take();
            return glib::ControlFlow::Continue;
        };

        if browse_id.trim().is_empty() {
            return glib::ControlFlow::Continue;
        }

        restore_fallback_after_revalidation(&controller, &browse_id, &fallbacks);

        let route_changed = active_route.borrow().as_deref() != Some(browse_id.as_str());
        if !route_changed {
            return glib::ControlFlow::Continue;
        }
        active_route.borrow_mut().replace(browse_id.clone());

        let loading = controller.youtube_playlist_loading.get()
            || controller
                .youtube_library
                .borrow()
                .playlist_loading
                .contains(&browse_id);
        let cached_items = controller
            .youtube_library
            .borrow()
            .playlist_tracks
            .get(&browse_id)
            .cloned()
            .unwrap_or_default();
        let already_revalidated = revalidated.borrow().contains(&browse_id);

        if !should_revalidate(!cached_items.is_empty(), loading, already_revalidated) {
            return glib::ControlFlow::Continue;
        }

        let playlist = controller
            .youtube_library
            .borrow()
            .playlists
            .iter()
            .find(|item| item.browse_id == browse_id)
            .cloned()
            .unwrap_or_else(|| YouTubeItem {
                result_type: "playlist".to_string(),
                title,
                browse_id: browse_id.clone(),
                playlist_kind: "library".to_string(),
                ..YouTubeItem::default()
            });

        revalidated.borrow_mut().insert(browse_id.clone());
        fallbacks
            .borrow_mut()
            .insert(browse_id.clone(), cached_items.clone());

        controller
            .youtube_library
            .borrow_mut()
            .playlist_tracks
            .remove(&browse_id);
        controller.load_youtube_playlist_for_browser(playlist);

        {
            let mut library = controller.youtube_library.borrow_mut();
            library
                .playlist_tracks
                .insert(browse_id.clone(), cached_items);
            library.playlist_loading.remove(&browse_id);
        }
        controller.refresh_browser();

        glib::ControlFlow::Continue
    });
}

fn restore_fallback_after_revalidation(
    controller: &AppController,
    browse_id: &str,
    fallbacks: &Rc<RefCell<HashMap<String, Vec<YouTubeItem>>>>,
) {
    let finished = !controller.youtube_playlist_loading.get()
        && !controller
            .youtube_library
            .borrow()
            .playlist_loading
            .contains(browse_id);
    if !finished {
        return;
    }

    let Some(fallback) = fallbacks.borrow_mut().remove(browse_id) else {
        return;
    };

    let needs_restore = controller
        .youtube_library
        .borrow()
        .playlist_tracks
        .get(browse_id)
        .map(Vec::is_empty)
        .unwrap_or(true);
    if !needs_restore || fallback.is_empty() {
        return;
    }

    controller
        .youtube_library
        .borrow_mut()
        .playlist_tracks
        .insert(browse_id.to_string(), fallback);
    if let Err(error) = queue_library_cache_save(&controller.youtube_library.borrow()) {
        eprintln!("Could not restore the last valid YouTube playlist cache: {error}");
    }
    controller.refresh_browser();
}

fn should_revalidate(has_cache: bool, loading: bool, already_revalidated: bool) -> bool {
    has_cache && !loading && !already_revalidated
}

#[cfg(test)]
mod tests {
    use super::should_revalidate;

    #[test]
    fn cached_playlist_revalidates_once_without_blocking() {
        assert!(should_revalidate(true, false, false));
        assert!(!should_revalidate(false, false, false));
        assert!(!should_revalidate(true, true, false));
        assert!(!should_revalidate(true, false, true));
    }
}
