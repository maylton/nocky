#[path = "playlist_cache_first/home_snapshot.rs"]
mod home_snapshot;
#[path = "playlist_cache_first/persistence.rs"]
mod persistence;

use self::{home_snapshot::DurableHomeSnapshot, persistence::DurablePlaylistCache};
use super::super::{youtube_playlist_revalidation_can_start, AppController};
use crate::{browser::BrowserRoute, youtube::YouTubeItem};
use gtk::{glib, prelude::*};
use std::{cell::RefCell, rc::Rc, time::Duration};

const HOME_SNAPSHOT_INTERVAL: Duration = Duration::from_secs(1);
const REVALIDATION_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(super) fn install(controller: &Rc<AppController>) {
    let home_snapshot = Rc::new(RefCell::new(DurableHomeSnapshot::load(controller)));
    let durable = Rc::new(RefCell::new(DurablePlaylistCache::load(controller)));

    {
        let home_snapshot = home_snapshot.clone();
        let durable = durable.clone();
        let cleanup: Rc<dyn Fn()> = Rc::new(move || {
            home_snapshot.borrow_mut().clear();
            durable.borrow_mut().clear();
        });
        controller
            .youtube_cache_first_cleanup
            .replace(Some(cleanup));
    }

    {
        let weak = Rc::downgrade(controller);
        let home_snapshot = home_snapshot.clone();
        controller.window.connect_close_request(move |_| {
            if let Some(controller) = weak.upgrade() {
                let current_home = controller.youtube_home_page.borrow().clone();
                home_snapshot.borrow_mut().persist_if_changed(&current_home);
            }
            glib::Propagation::Proceed
        });
    }

    {
        let weak = Rc::downgrade(controller);
        let home_snapshot = home_snapshot.clone();
        glib::timeout_add_local(HOME_SNAPSHOT_INTERVAL, move || {
            let Some(controller) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            let current_home = controller.youtube_home_page.borrow().clone();
            home_snapshot.borrow_mut().persist_if_changed(&current_home);
            glib::ControlFlow::Continue
        });
    }

    let weak = Rc::downgrade(controller);
    glib::timeout_add_local(REVALIDATION_POLL_INTERVAL, move || {
        let Some(controller) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        if !controller.youtube_library.borrow().connected {
            return glib::ControlFlow::Continue;
        }

        let route = controller.browser.route();
        let BrowserRoute::YouTubePlaylist { title, browse_id } = route else {
            return glib::ControlFlow::Continue;
        };

        if browse_id.trim().is_empty() {
            return glib::ControlFlow::Continue;
        }

        let (items, restored) = durable
            .borrow()
            .items_with_fallback(&controller, browse_id.as_str());
        if restored {
            controller.refresh_browser();
        }
        if items.is_empty() {
            return glib::ControlFlow::Continue;
        }

        durable.borrow_mut().persist_if_changed(&browse_id, &items);

        let now = std::time::Instant::now();
        let state = controller
            .youtube_playlist_revalidation
            .borrow()
            .get(&browse_id)
            .cloned();
        if !youtube_playlist_revalidation_can_start(state.as_ref(), now) {
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
                ..YouTubeItem::default()
            });

        controller.revalidate_youtube_playlist_for_browser(playlist);
        glib::ControlFlow::Continue
    });
}

impl AppController {
    pub(crate) fn clear_youtube_cache_first_data(&self) {
        let cleanup = self.youtube_cache_first_cleanup.borrow().clone();
        if let Some(cleanup) = cleanup {
            cleanup();
        }

        self.youtube_home_page.replace(Default::default());
        self.youtube_home_previous_params.borrow_mut().clear();
        self.youtube_playlist_revalidation.borrow_mut().clear();
        self.youtube_pending_playlist.replace(None);

        self.youtube_home_loading.set(false);
        self.youtube_playlist_loading.set(false);
        self.youtube_playlist_prefetching.set(false);

        // Invalidate responses started before the account was disconnected.
        self.youtube_home_request_id
            .set(self.youtube_home_request_id.get().wrapping_add(1));
        self.youtube_playlist_request_id
            .set(self.youtube_playlist_request_id.get().wrapping_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::youtube_playlist_revalidation_can_start;
    use crate::app::controller::{youtube_playlist_revalidation_delay, PlaylistRevalidationState};
    use std::time::{Duration, Instant};

    #[test]
    fn retry_is_allowed_after_failure_window() {
        let now = Instant::now();
        let state = PlaylistRevalidationState::RetryAt {
            when: now - Duration::from_secs(1),
            attempt: 1,
        };

        assert!(youtube_playlist_revalidation_can_start(Some(&state), now));
    }

    #[test]
    fn retry_is_blocked_while_loading_or_succeeded() {
        let now = Instant::now();

        assert!(!youtube_playlist_revalidation_can_start(
            Some(&PlaylistRevalidationState::Loading { attempt: 0 }),
            now
        ));
        assert!(!youtube_playlist_revalidation_can_start(
            Some(&PlaylistRevalidationState::Succeeded),
            now
        ));
    }

    #[test]
    fn retry_waits_until_retry_at() {
        let now = Instant::now();
        let state = PlaylistRevalidationState::RetryAt {
            when: now + Duration::from_secs(1),
            attempt: 1,
        };

        assert!(!youtube_playlist_revalidation_can_start(Some(&state), now));
    }

    #[test]
    fn backoff_increases_and_is_capped() {
        assert_eq!(
            youtube_playlist_revalidation_delay(1),
            Duration::from_secs(5)
        );
        assert_eq!(
            youtube_playlist_revalidation_delay(2),
            Duration::from_secs(15)
        );
        assert_eq!(
            youtube_playlist_revalidation_delay(3),
            Duration::from_secs(30)
        );
        assert_eq!(
            youtube_playlist_revalidation_delay(4),
            Duration::from_secs(60)
        );
        assert_eq!(
            youtube_playlist_revalidation_delay(9),
            Duration::from_secs(60)
        );
    }
}
