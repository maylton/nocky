// local_artist_index_foundation_v3
// stable_artist_directory_refresh_v1
// stable_collection_identity_and_deferred_cache_v2
// clickable_player_artist_album_navigation_v1
// artist_profile_revalidation_v5
// youtube_like_reconciliation_and_request_guard_v1
// youtube_like_button_and_track_menu_v2
// youtube_real_like_sync_v5
// source_aware_liked_songs_page_v1
// clickable_lyrics_seek_v3
// fix_resume_seek_oscillation_v1
// fix_shutdown_save_order_v1
// youtube_resume_seek_convergence_v1
// lyrics_2_v2
// playback_resume_preferences_fix_v1
// playback_persistence_resume_2_v1
// queue_collection_cover_fallback_v1
// preserve_home_carousel_scroll_v1
// collection_card_inline_loading_fix_v2
// youtube_collection_background_playback_v1
// collection_card_loading_spinner_v3\n// youtube_collection_queue_background_load_v1
// collection_card_overflow_and_play_state_v2
// youtube_playlist_background_autoplay_v1
// contextual_collection_controls_v5
// recent_activity_exact_fix_v1
// personalized_home_resume_v2
#[path = "ui/widgets/animated_page_switcher.rs"]
mod animated_page_switcher;
mod app;
mod artist_index;
mod background;
mod background_handler;
mod browser;
#[path = "ui/widgets/compact_volume_motion.rs"]
mod compact_volume_motion;
mod config;
#[path = "ui/widgets/cover.rs"]
mod cover_view;
mod dialogs;
#[path = "ui/widgets/expressive_transport.rs"]
mod expressive_transport;
#[path = "ui/footer/layout.rs"]
mod footer_layout;
#[path = "ui/footer/now_playing.rs"]
mod footer_now_playing;
#[path = "ui/footer/progress.rs"]
mod footer_progress;
#[path = "ui/footer/transport.rs"]
mod footer_transport;
#[path = "ui/footer/utilities.rs"]
mod footer_utilities;
#[path = "ui/footer/view.rs"]
mod footer_view;
mod i18n;
mod integrations;
mod library;
mod listening_history;
mod local_mix_cover;
mod search_text;
// material_dynamic_palette_v1
mod lyrics;
#[path = "lyrics/provider.rs"]
mod lyrics_provider;
#[path = "lyrics/view.rs"]
mod lyrics_view;
mod material_palette;
mod md3_volume;
mod mode_toggle;
mod model;
mod offline_store;
mod onboarding;
pub mod playback;
#[path = "ui/player/view.rs"]
mod player_view;
mod reveal_bounce;
#[path = "ui/settings/page.rs"]
mod settings_page;
mod theme;
mod theme_css;
mod visual_theme;
mod visualizer;
#[path = "ui/widgets/wave_progress.rs"]
mod wave_progress;
mod youtube;
#[path = "youtube/diagnostics.rs"]
mod youtube_diagnostics;
#[path = "youtube/error.rs"]
mod youtube_error;
#[path = "youtube/playback.rs"]
mod youtube_playback;

use gtk::glib;

const APP_ID: &str = "io.github.maylton.Nocky";
const HOME_PLAYER_WIDTH: i32 = 454;

fn main() -> glib::ExitCode {
    app::run()
}
