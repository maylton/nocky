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
mod app;
mod artist_index;
mod background;
mod background_handler;
mod browser;
mod config;
mod dialogs;
mod i18n;
mod integrations;
mod library;
mod listening_history;
mod local_mix_cover;
mod search_text;
// material_dynamic_palette_v1
mod lyrics;
mod material_palette;
mod md3_volume;
mod mode_toggle;
mod model;
mod offline_store;
mod onboarding;
pub mod playback;
mod reveal_bounce;
mod theme;
mod theme_css;
mod ui;
mod visual_theme;
mod visualizer;
mod youtube;

use gtk::glib;

const APP_ID: &str = "io.github.maylton.Nocky";
const HOME_PLAYER_WIDTH: i32 = 454;

fn main() -> glib::ExitCode {
    app::run()
}
