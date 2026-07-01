//! Application controller data structures.

mod actions;
mod appearance;
mod background;
mod callbacks;
mod construction;
mod favorites;
mod feedback;
mod local_library;
mod lyrics;
mod navigation;
mod offline;
mod persistence;
mod playback;
mod queue;
mod queue_presentation;
mod settings;
mod youtube;
mod youtube_playlist_metadata;

pub(crate) use construction::build_application;

use crate::{
    app::state::{AppState, PlaybackSource, YouTubePlaybackState},
    background::BackgroundChannel,
    browser::LibraryBrowser,
    config,
    listening_history::{self, ListeningHistory},
    lyrics::LyricsPresenter,
    offline_store::OfflineStore,
    playback::{
        queue::{PlaybackQueue, QueueEntryId, QueueSnapshot, QueueSourceKind, ShuffleNavigator},
        session::PlaybackSession,
        transition::TransitionClock,
        PlaybackEngine,
    },
    reveal_bounce::RevealBounce,
    theme,
    ui::{
        player::PlayerViewHandle,
        settings::SettingsPage,
        widgets::{AnimatedPageSwitcher, CoverView, ExpressiveTransport, WaveProgress},
    },
    visual_theme,
    visualizer::SpectrumVisualizer,
    youtube::{
        LikeMutationRegistry, YouTubeBridge, YouTubeHomePage, YouTubeItem, YouTubeLibraryCache,
        YouTubePage,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

pub(crate) const YOUTUBE_PLAYLIST_REVALIDATION_BACKOFF_SECS: [u64; 4] = [5, 15, 30, 60];

#[derive(Clone, Debug)]
pub(crate) enum PlaylistRevalidationState {
    Loading { attempt: u8 },
    Succeeded,
    RetryAt { when: Instant, attempt: u8 },
}

pub(crate) fn youtube_playlist_revalidation_delay(attempt: u8) -> Duration {
    let index = attempt.saturating_sub(1) as usize;
    let seconds = YOUTUBE_PLAYLIST_REVALIDATION_BACKOFF_SECS
        .get(index)
        .copied()
        .unwrap_or(
            *YOUTUBE_PLAYLIST_REVALIDATION_BACKOFF_SECS
                .last()
                .unwrap_or(&60),
        );
    Duration::from_secs(seconds)
}

pub(crate) fn youtube_playlist_revalidation_can_start(
    state: Option<&PlaylistRevalidationState>,
    now: Instant,
) -> bool {
    match state {
        None => true,
        Some(PlaylistRevalidationState::RetryAt { when, .. }) => now >= *when,
        Some(PlaylistRevalidationState::Loading { .. } | PlaylistRevalidationState::Succeeded) => {
            false
        }
    }
}

pub(crate) struct ControllerRuntime {
    pub(crate) state: RefCell<AppState>,
    pub(crate) playback_queue_v2: RefCell<PlaybackQueue>,
    pub(crate) active_queue_source: Cell<QueueSourceKind>,
    pub(crate) queue_last_saved_snapshot: RefCell<QueueSnapshot>,
    pub(crate) queue_dragged_entry: Cell<Option<QueueEntryId>>,
    pub(crate) queue_v2_pending_entry: Cell<Option<QueueEntryId>>,
    pub(crate) config: RefCell<config::AppConfig>,
    pub(crate) listening_history: RefCell<ListeningHistory>,
    pub(crate) listening_session_id: RefCell<Option<String>>,
    pub(crate) listening_session_last_saved_seconds: Cell<u64>,
    pub(crate) listening_history_context: RefCell<listening_history::PlaybackHistoryContext>,
    pub(crate) pending_resume_position_us: Cell<Option<i64>>,
    pub(crate) restored_playback_session: RefCell<Option<PlaybackSession>>,
    pub(crate) startup_restore_autoplay: Cell<Option<bool>>,
    pub(crate) playback_session_last_position_seconds: Cell<u64>,
    pub(crate) playback_session_last_shuffle: Cell<bool>,
    pub(crate) playback_session_last_repeat: Cell<bool>,
    pub(crate) playback_session_restore_attempts: Cell<u8>,
    pub(crate) updating_progress: Cell<bool>,
    pub(crate) scanning: Cell<bool>,
    pub(crate) shuffle_enabled: Cell<bool>,
    pub(crate) shuffle_navigation: RefCell<ShuffleNavigator>,
    pub(crate) rng_state: Cell<u64>,
    pub(crate) search_query: RefCell<String>,
    pub(crate) lyrics_pending: RefCell<HashSet<PathBuf>>,
    pub(crate) background: BackgroundChannel,
    pub(crate) mpris: crate::playback::mpris::MprisBridge,
    pub(crate) last_mpris_position: Cell<i64>,
    pub(crate) playback_source: Cell<PlaybackSource>,
    pub(crate) youtube_state: RefCell<Option<YouTubePlaybackState>>,
    pub(crate) youtube_request_id: Cell<u64>,
    pub(crate) youtube_search_request_id: Cell<u64>,
    pub(crate) youtube_home_request_id: Cell<u64>,
    pub(crate) youtube_home_loading: Cell<bool>,
    pub(crate) youtube_home_previous_params: RefCell<String>,
    pub(crate) youtube_recovery_in_progress: Cell<bool>,
    pub(crate) youtube_recovery_attempted: Cell<bool>,
    pub(crate) youtube_recovery_retry_count: Cell<u8>,
    pub(crate) youtube_recovery_generation: Cell<u64>,
    pub(crate) youtube_recovery_resume_us: Cell<i64>,
    pub(crate) youtube_recovery_was_playing: Cell<bool>,
    pub(crate) youtube_playlist_request_id: Cell<u64>,
    pub(crate) youtube_collection_play_request_id: Cell<u64>,
    pub(crate) youtube_collection_queue_request_id: Cell<u64>,
    pub(crate) youtube_collection_prefetching: Cell<bool>,
    pub(crate) youtube_playlist_loading: Cell<bool>,
    pub(crate) youtube_playlist_prefetching: Cell<bool>,
    pub(crate) youtube_pending_playlist: RefCell<Option<YouTubeItem>>,
    pub(crate) youtube_playlist_revalidation: RefCell<HashMap<String, PlaylistRevalidationState>>,
    pub(crate) youtube_cache_first_cleanup: RefCell<Option<Rc<dyn Fn()>>>,
    pub(crate) youtube_bridge: Option<Arc<YouTubeBridge>>,
    pub(crate) youtube_home_page: RefCell<YouTubeHomePage>,
    pub(crate) youtube_library: RefCell<YouTubeLibraryCache>,
    pub(crate) offline_store: RefCell<OfflineStore>,
    pub(crate) offline_download_pending: RefCell<HashSet<String>>,
    pub(crate) youtube_like_request_id: Cell<u64>,
    pub(crate) youtube_like_pending: RefCell<HashMap<String, u64>>,
    pub(crate) youtube_like_mutations: RefCell<LikeMutationRegistry>,
    pub(crate) youtube_playlist_create_pending: Cell<bool>,
}

pub(crate) struct AppController {
    pub(crate) window: adw::ApplicationWindow,
    pub(crate) toast_overlay: adw::ToastOverlay,
    pub(crate) player: PlaybackEngine,
    runtime: ControllerRuntime,
    pub(crate) sidebar: gtk::Revealer,
    pub(crate) sidebar_motion: gtk::Fixed,
    pub(crate) sidebar_content: gtk::Box,
    pub(crate) sidebar_bounce: Rc<RevealBounce>,
    pub(crate) sidebar_button: gtk::ToggleButton,
    pub(crate) sidebar_all: gtk::Button,
    pub(crate) sidebar_all_label: gtk::Label,
    pub(crate) sidebar_albums: gtk::Button,
    pub(crate) sidebar_albums_label: gtk::Label,
    pub(crate) sidebar_artists: gtk::Button,
    pub(crate) sidebar_artists_label: gtk::Label,
    pub(crate) sidebar_playlists: gtk::Button,
    pub(crate) sidebar_playlists_label: gtk::Label,
    pub(crate) sidebar_liked: gtk::Button,
    pub(crate) sidebar_liked_label: gtk::Label,
    pub(crate) sidebar_section_label: gtk::Label,
    pub(crate) search_button: gtk::ToggleButton,
    pub(crate) folder_button: gtk::Button,
    pub(crate) search_entry: gtk::SearchEntry,
    pub(crate) settings_button: gtk::ToggleButton,
    pub(crate) content_stack: gtk::Stack,
    pub(crate) settings_page: Rc<SettingsPage>,
    pub(crate) views: adw::ViewStack,
    pub(crate) music_page: adw::ViewStackPage,
    pub(crate) lyrics_page: adw::ViewStackPage,
    pub(crate) queue_page_list: gtk::Box,
    pub(crate) queue_page_summary: gtk::Label,
    pub(crate) queue_page_source: gtk::Label,
    pub(crate) queue_page_upcoming_badge: gtk::Label,
    pub(crate) queue_page_total_badge: gtk::Label,
    pub(crate) queue_page_clear_upcoming: gtk::Button,
    pub(crate) queue_page_clear_all: gtk::Button,
    pub(crate) queue_page_popover_proxy: gtk::Popover,
    pub(crate) queue_page_last_snapshot: RefCell<Option<QueueSnapshot>>,
    pub(crate) queue_page_last_source: Cell<Option<QueueSourceKind>>,
    pub(crate) page_switcher: Rc<AnimatedPageSwitcher>,
    pub(crate) browser: LibraryBrowser,
    pub(crate) lyrics: LyricsPresenter,
    pub(crate) youtube_page: Rc<YouTubePage>,
    pub(crate) player_view: PlayerViewHandle,
    pub(crate) player_revealer: gtk::Revealer,
    pub(crate) player_motion: gtk::Fixed,
    pub(crate) player_viewport: gtk::ScrolledWindow,
    pub(crate) player_bounce: Rc<RevealBounce>,
    pub(crate) player_toggle_button: gtk::Button,
    pub(crate) player_toggle_icon: gtk::Image,
    pub(crate) player_artist: gtk::Label,
    pub(crate) album: gtk::Label,
    pub(crate) now_heading: gtk::Label,
    pub(crate) favorite_button: gtk::Button,
    pub(crate) previous_button: gtk::Button,
    pub(crate) hero_play_button: gtk::Button,
    pub(crate) main_transport_motion: Rc<ExpressiveTransport>,
    pub(crate) next_button: gtk::Button,
    pub(crate) mini_title: gtk::Label,
    pub(crate) mini_artist: gtk::Label,
    pub(crate) footer_source: gtk::Label,
    pub(crate) footer_now_playing: gtk::Button,
    pub(crate) footer_center: gtk::Box,
    pub(crate) footer_right_controls: gtk::Box,
    pub(crate) volume_revealer: gtk::Revealer,
    pub(crate) music_stack: gtk::Stack,
    pub(crate) empty_title: gtk::Label,
    pub(crate) empty_text: gtk::Label,
    pub(crate) empty_add: gtk::Button,
    pub(crate) hero_cover: CoverView,
    pub(crate) mini_cover: CoverView,
    pub(crate) player_bar: gtk::CenterBox,
    pub(crate) play_icon: gtk::Image,
    pub(crate) hero_play_icon: gtk::Image,
    pub(crate) favorite_icon: gtk::Image,
    pub(crate) footer_favorite_icon: gtk::Image,
    pub(crate) footer_favorite_button: gtk::Button,
    pub(crate) progress: gtk::Scale,
    pub(crate) home_progress_stack: gtk::Stack,
    pub(crate) home_wave_progress: WaveProgress,
    pub(crate) elapsed: gtk::Label,
    pub(crate) duration: gtk::Label,
    pub(crate) footer_progress_stack: gtk::Stack,
    pub(crate) footer_traditional_progress: gtk::Scale,
    pub(crate) footer_progress: WaveProgress,
    pub(crate) footer_elapsed: gtk::Label,
    pub(crate) footer_duration: gtk::Label,
    pub(crate) volume: gtk::Adjustment,
    pub(crate) mute_icon: gtk::Image,
    pub(crate) mute_button: gtk::Button,
    pub(crate) volume_before_mute: Cell<f64>,
    pub(crate) compact_volume_expanded: Cell<bool>,
    pub(crate) compact_volume_spring_generation: Rc<Cell<u64>>,
    pub(crate) footer_metadata_transition: TransitionClock,
    pub(crate) lyrics_button: gtk::ToggleButton,
    pub(crate) footer_previous: gtk::Button,
    pub(crate) footer_play_button: gtk::Button,
    pub(crate) footer_transport_motion: Rc<ExpressiveTransport>,
    pub(crate) footer_next: gtk::Button,
    pub(crate) footer_repeat_button: gtk::ToggleButton,
    pub(crate) footer_shuffle_button: gtk::ToggleButton,
    pub(crate) repeat_button: gtk::ToggleButton,
    pub(crate) shuffle_button: gtk::ToggleButton,
    pub(crate) visualizer: SpectrumVisualizer,
    pub(crate) visual_theme_manager: Rc<visual_theme::VisualThemeManager>,
    pub(crate) _theme: Rc<theme::ThemeBridge>,
}

// Transitional compatibility layer: controller modules can keep using
// `self.state`, `self.config`, and related field access while runtime state is
// progressively split into explicit domain contexts.
impl std::ops::Deref for AppController {
    type Target = ControllerRuntime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}
