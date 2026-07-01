use super::super::{youtube_playlist_revalidation_can_start, AppController};
use crate::{browser::BrowserRoute, youtube::YouTubeItem};
use gtk::glib;
use std::{rc::Rc, time::Duration};

const REVALIDATION_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(super) fn install(controller: &Rc<AppController>) {
    let weak = Rc::downgrade(controller);

    glib::timeout_add_local(REVALIDATION_POLL_INTERVAL, move || {
        let Some(controller) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let route = controller.browser.route();
        let BrowserRoute::YouTubePlaylist { title, browse_id } = route else {
            return glib::ControlFlow::Continue;
        };

        if browse_id.trim().is_empty() || !playlist_has_cache(&controller, &browse_id) {
            return glib::ControlFlow::Continue;
        }

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
                playlist_kind: "library".to_string(),
                ..YouTubeItem::default()
            });

        controller.revalidate_youtube_playlist_for_browser(playlist);
        glib::ControlFlow::Continue
    });
}

fn playlist_has_cache(controller: &AppController, browse_id: &str) -> bool {
    controller
        .youtube_library
        .borrow()
        .playlist_tracks
        .get(browse_id)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
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
