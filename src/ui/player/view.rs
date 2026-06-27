// clickable_player_artist_album_navigation_v1
use crate::{
    config::AppLanguage,
    cover_view::{build_cover, CoverView},
    expressive_transport::{ExpressiveTransport, TransportVariant},
    i18n::{self, Message},
    lyrics_view::LyricsPresenter,
    playback::transition::TransitionClock,
    visualizer::SpectrumVisualizer,
    wave_progress::WaveProgress,
};
use gtk::prelude::*;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct PlayerViewHandle {
    title: gtk::Label,
    artist: gtk::Label,
    album: gtk::Label,
    favorite_icon: gtk::Image,
    hero_play_icon: gtk::Image,
    lyrics: LyricsPresenter,
    lyrics_slot: gtk::CenterBox,
    lyrics_toggle_button: gtk::ToggleButton,
    visualizer: SpectrumVisualizer,
    metadata_transition: TransitionClock,
}

impl PlayerViewHandle {
    pub(crate) fn set_metadata(&self, title: &str, artist: &str, album: &str) {
        if !adw::is_animations_enabled(&self.title) {
            self.title.set_text(title);
            self.artist.set_text(artist);
            self.album.set_text(album);
            self.title.set_opacity(1.0);
            self.artist.set_opacity(1.0);
            self.album.set_opacity(1.0);
            return;
        }

        if self.title.text().as_str() == title
            && self.artist.text().as_str() == artist
            && self.album.text().as_str() == album
        {
            return;
        }

        let token = self.metadata_transition.next();
        self.metadata_transition
            .fade(token, &self.title, self.title.opacity(), 0.0, 0, 90);
        self.metadata_transition
            .fade(token, &self.artist, self.artist.opacity(), 0.0, 12, 90);
        self.metadata_transition
            .fade(token, &self.album, self.album.opacity(), 0.0, 24, 90);

        let handle = self.clone();
        let title = title.to_owned();
        let artist = artist.to_owned();
        let album = album.to_owned();
        self.metadata_transition.after(token, 112, move || {
            handle.title.set_text(&title);
            handle.artist.set_text(&artist);
            handle.album.set_text(&album);

            handle
                .metadata_transition
                .fade(token, &handle.title, 0.0, 1.0, 0, 190);
            handle
                .metadata_transition
                .fade(token, &handle.artist, 0.0, 1.0, 42, 190);
            handle
                .metadata_transition
                .fade(token, &handle.album, 0.0, 1.0, 78, 190);
        });
    }

    pub(crate) fn set_favorite(&self, active: bool) {
        self.favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.favorite_icon
            .set_opacity(if active { 1.0 } else { 0.28 });
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        self.hero_play_icon.set_icon_name(Some(if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        }));
    }

    pub(crate) fn set_lyrics_visible(&self, visible: bool) {
        self.lyrics_slot.set_visible(visible);
        self.lyrics.inline_widget().set_visible(visible);
        if self.lyrics_toggle_button.is_active() != visible {
            self.lyrics_toggle_button.set_active(visible);
        }
    }
    pub(crate) fn set_visualizer_active(&self, active: bool) {
        self.visualizer.set_active(active);
    }
}

pub(crate) struct PlayerView {
    pub(crate) handle: PlayerViewHandle,
    pub(crate) root: gtk::Box,
    pub(crate) artist: gtk::Label,
    pub(crate) album: gtk::Label,
    pub(crate) now_heading: gtk::Label,
    pub(crate) favorite_button: gtk::Button,
    pub(crate) previous_button: gtk::Button,
    pub(crate) hero_play_button: gtk::Button,
    pub(crate) next_button: gtk::Button,
    pub(crate) transport_motion: Rc<ExpressiveTransport>,
    pub(crate) inline_lyrics_button: gtk::ToggleButton,
    pub(crate) refresh_lyrics_button: gtk::Button,
    pub(crate) hero_cover: CoverView,
    pub(crate) hero_play_icon: gtk::Image,
    pub(crate) favorite_icon: gtk::Image,
    pub(crate) progress: gtk::Scale,
    pub(crate) home_progress_stack: gtk::Stack,
    pub(crate) home_wave_progress: WaveProgress,
    pub(crate) elapsed: gtk::Label,
    pub(crate) duration: gtk::Label,
    pub(crate) repeat_button: gtk::ToggleButton,
    pub(crate) shuffle_button: gtk::ToggleButton,
    pub(crate) visualizer: SpectrumVisualizer,
    pub(crate) lyrics: LyricsPresenter,
}

// material_settings_and_local_player_dimensions_v1
const PLAYER_INNER_WIDTH: i32 = 384;
const PLAYER_SURFACE_WIDTH: i32 = 414;
const PLAYER_CARD_WIDTH: i32 = 454;

// material_expressive_player_v1
impl PlayerView {
    pub(crate) fn new(language: AppLanguage, expressive_transport_effects: bool) -> Self {
        let tr = |message: Message| i18n::text(language, message);

        let title = gtk::Label::new(Some(tr(Message::IntegratedMusic)));
        title.set_xalign(0.0);
        title.set_wrap(false);
        title.set_single_line_mode(true);
        title.set_hexpand(true);
        title.set_width_request(330);
        title.set_width_chars(-1);
        title.set_max_width_chars(32);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_overflow(gtk::Overflow::Hidden);
        title.add_css_class("hero-title");
        title.add_css_class("player-track-title");

        let artist = gtk::Label::new(Some(tr(Message::NoTrackSelected)));
        artist.set_xalign(0.0);
        artist.set_single_line_mode(true);
        artist.set_width_request(PLAYER_INNER_WIDTH);
        artist.set_width_chars(-1);
        artist.set_max_width_chars(40);
        artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist.set_overflow(gtk::Overflow::Hidden);
        artist.add_css_class("hero-artist");
        artist.add_css_class("player-artist");
        artist.add_css_class("player-metadata-link");
        artist.set_cursor_from_name(Some("pointer"));

        let album = gtk::Label::new(Some(tr(Message::ChooseFolderToStart)));
        album.set_xalign(0.0);
        album.set_single_line_mode(true);
        album.set_width_request(PLAYER_INNER_WIDTH);
        album.set_width_chars(-1);
        album.set_max_width_chars(40);
        album.set_ellipsize(gtk::pango::EllipsizeMode::End);
        album.set_overflow(gtk::Overflow::Hidden);
        album.add_css_class("dim-label");
        album.add_css_class("player-album");
        album.add_css_class("player-metadata-link");
        album.set_cursor_from_name(Some("pointer"));

        let favorite_icon = gtk::Image::from_icon_name("emblem-favorite-symbolic");
        favorite_icon.set_opacity(0.28);
        let favorite = gtk::Button::new();
        favorite.set_child(Some(&favorite_icon));
        favorite.add_css_class("flat");
        favorite.add_css_class("card-icon-button");
        favorite.set_tooltip_text(Some(tr(Message::FavoriteTooltip)));
        favorite.add_css_class("like-button");
        favorite.add_css_class("player-favorite-action");
        favorite.set_size_request(34, 34);
        favorite.set_hexpand(false);
        favorite.set_vexpand(false);

        let now_heading = gtk::Label::new(Some(tr(Message::NowPlaying)));
        now_heading.set_xalign(0.0);
        now_heading.set_hexpand(true);
        now_heading.add_css_class("now-heading");
        now_heading.add_css_class("player-eyebrow");
        let headphones = gtk::Image::from_icon_name("audio-headphones-symbolic");
        headphones.set_pixel_size(16);
        headphones.add_css_class("now-heading-icon");

        let headphones_slot = gtk::CenterBox::new();
        headphones_slot.set_size_request(34, 34);
        headphones_slot.set_hexpand(false);
        headphones_slot.set_vexpand(false);
        headphones_slot.set_halign(gtk::Align::Center);
        headphones_slot.set_valign(gtk::Align::Center);
        headphones_slot.set_center_widget(Some(&headphones));
        headphones_slot.add_css_class("player-header-icon");
        let inline_lyrics_button = gtk::ToggleButton::builder()
            .icon_name("audio-input-microphone-symbolic")
            .active(true)
            .tooltip_text(tr(Message::HomeLyricsDescription))
            .build();
        inline_lyrics_button.add_css_class("flat");
        inline_lyrics_button.add_css_class("card-icon-button");
        inline_lyrics_button.add_css_class("player-secondary-action");
        inline_lyrics_button.set_size_request(34, 34);

        let refresh_lyrics_button = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh_lyrics_button.set_tooltip_text(Some(tr(Message::MenuDownloadLyrics)));
        refresh_lyrics_button.add_css_class("flat");
        refresh_lyrics_button.add_css_class("card-icon-button");
        refresh_lyrics_button.add_css_class("player-secondary-action");
        refresh_lyrics_button.set_size_request(34, 34);

        let player_header_actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        player_header_actions.set_hexpand(false);
        player_header_actions.set_halign(gtk::Align::End);
        player_header_actions.set_valign(gtk::Align::Center);
        player_header_actions.add_css_class("player-header-actions");
        player_header_actions.append(&headphones_slot);
        player_header_actions.append(&inline_lyrics_button);
        player_header_actions.append(&refresh_lyrics_button);

        let now_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        now_header.set_hexpand(true);
        now_header.add_css_class("player-now-header");
        now_header.append(&now_heading);
        now_header.append(&player_header_actions);

        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        title_row.set_size_request(PLAYER_INNER_WIDTH, 34);
        title_row.set_hexpand(false);
        title_row.set_vexpand(false);
        title_row.set_overflow(gtk::Overflow::Hidden);
        title_row.add_css_class("player-title-row");
        title_row.append(&title);
        title_row.append(&favorite);

        let hero_cover = build_cover(280);
        hero_cover.stack.set_halign(gtk::Align::Center);
        hero_cover.stack.set_overflow(gtk::Overflow::Hidden);
        hero_cover.stack.add_css_class("player-artwork");

        // Keep placeholder and real artwork in the same balanced vertical slot.
        let hero_cover_slot = gtk::CenterBox::new();
        hero_cover_slot.set_orientation(gtk::Orientation::Vertical);
        hero_cover_slot.set_vexpand(false);
        hero_cover_slot.set_hexpand(true);
        hero_cover_slot.set_margin_top(0);
        hero_cover_slot.set_margin_bottom(0);
        hero_cover_slot.set_height_request(328);
        hero_cover_slot.set_width_request(PLAYER_SURFACE_WIDTH);
        hero_cover_slot.set_valign(gtk::Align::Start);
        hero_cover_slot.set_center_widget(Some(&hero_cover.stack));
        hero_cover_slot.add_css_class("hero-cover-slot");
        hero_cover_slot.add_css_class("stable-player-cover-slot");
        hero_cover_slot.add_css_class("player-artwork-slot");

        let elapsed = gtk::Label::new(Some("0:00"));
        elapsed.add_css_class("time-label");
        elapsed.add_css_class("player-elapsed");
        let duration = gtk::Label::new(Some("0:00"));
        duration.add_css_class("time-label");
        duration.add_css_class("player-duration");
        let progress = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
        progress.set_draw_value(false);
        progress.add_css_class("player-progress-track");
        progress.set_hexpand(true);
        progress.add_css_class("progress-scale");

        let home_wave_progress = WaveProgress::new();

        // material_footer_player_progress_refinement_v1

        home_wave_progress
            .widget()
            .add_css_class("player-progress-wave");
        home_wave_progress
            .widget()
            .add_css_class("home-wave-progress");

        let home_progress_stack = gtk::Stack::new();
        home_progress_stack.set_hexpand(true);
        home_progress_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        home_progress_stack.set_transition_duration(160);
        home_progress_stack.add_named(&progress, Some("classic"));
        home_progress_stack.add_named(home_wave_progress.widget(), Some("m3"));

        let time_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        elapsed.set_hexpand(true);
        elapsed.set_xalign(0.0);
        duration.set_xalign(1.0);
        time_row.append(&elapsed);
        time_row.append(&duration);
        time_row.add_css_class("player-time-row");

        let repeat = crate::mode_toggle::new_mode_toggle(
            "media-playlist-repeat-symbolic",
            tr(Message::RepeatTrack),
            crate::mode_toggle::ModeToggleKind::RepeatOne,
        );
        repeat.add_css_class("media-control");
        repeat.add_css_class("player-mode-control");
        let previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        previous.set_tooltip_text(Some(tr(Message::PreviousTrack)));
        previous.add_css_class("media-control");
        previous.add_css_class("player-skip-control");

        let hero_play_icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
        hero_play_icon.set_pixel_size(24);
        let hero_play_button = gtk::Button::new();
        hero_play_button.set_child(Some(&hero_play_icon));
        hero_play_button.add_css_class("shell-play-button");
        hero_play_button.add_css_class("player-primary-control");
        hero_play_button.set_tooltip_text(Some(tr(Message::PlayPause)));

        let next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next.set_tooltip_text(Some(tr(Message::NextTrack)));
        next.add_css_class("media-control");
        next.add_css_class("player-skip-control");
        let shuffle = crate::mode_toggle::new_mode_toggle(
            "media-playlist-shuffle-symbolic",
            tr(Message::Shuffle),
            crate::mode_toggle::ModeToggleKind::Shuffle,
        );
        shuffle.add_css_class("media-control");
        shuffle.add_css_class("player-mode-control");

        let transport_motion = ExpressiveTransport::new(
            TransportVariant::Main,
            &previous,
            &hero_play_button,
            &next,
            &hero_play_icon,
            expressive_transport_effects,
        );

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        controls.set_halign(gtk::Align::Center);
        controls.add_css_class("player-transport-controls");
        controls.append(&shuffle);
        controls.append(transport_motion.root());
        controls.append(&repeat);

        let visualizer = SpectrumVisualizer::new();
        let lyrics = LyricsPresenter::new(language);

        // stable_home_player_layout_v1
        // stable_standby_slots_v1
        let visualizer_widget = visualizer.widget().clone();
        visualizer_widget.set_size_request(PLAYER_INNER_WIDTH, 74);
        visualizer_widget.set_hexpand(false);
        visualizer_widget.set_halign(gtk::Align::Center);
        visualizer_widget.set_vexpand(false);
        visualizer_widget.set_valign(gtk::Align::Center);

        let visualizer_slot = gtk::CenterBox::new();
        visualizer_slot.set_orientation(gtk::Orientation::Vertical);
        visualizer_slot.set_size_request(PLAYER_SURFACE_WIDTH, 74);
        visualizer_slot.set_hexpand(false);
        visualizer_slot.set_halign(gtk::Align::Center);
        visualizer_slot.set_vexpand(false);
        visualizer_slot.set_valign(gtk::Align::Start);
        visualizer_slot.set_center_widget(Some(&visualizer_widget));
        visualizer_slot.set_overflow(gtk::Overflow::Hidden);
        visualizer_slot.add_css_class("stable-visualizer-slot");
        visualizer_slot.add_css_class("player-visualizer-surface");

        let inline_lyrics_widget = lyrics.inline_widget().clone();
        inline_lyrics_widget.set_size_request(PLAYER_INNER_WIDTH, 158);
        inline_lyrics_widget.set_hexpand(false);
        inline_lyrics_widget.set_halign(gtk::Align::Center);
        inline_lyrics_widget.set_vexpand(false);
        inline_lyrics_widget.set_valign(gtk::Align::Center);

        let lyrics_slot = gtk::CenterBox::new();
        lyrics_slot.set_orientation(gtk::Orientation::Vertical);
        lyrics_slot.set_size_request(PLAYER_SURFACE_WIDTH, 158);
        lyrics_slot.set_hexpand(false);
        lyrics_slot.set_halign(gtk::Align::Center);
        lyrics_slot.set_vexpand(false);
        lyrics_slot.set_valign(gtk::Align::Start);
        lyrics_slot.set_center_widget(Some(&inline_lyrics_widget));
        lyrics_slot.set_overflow(gtk::Overflow::Hidden);
        lyrics_slot.add_css_class("stable-lyrics-slot");
        lyrics_slot.add_css_class("player-lyrics-surface");

        title_row.set_height_request(34);
        title_row.set_vexpand(false);
        title_row.set_valign(gtk::Align::Center);

        artist.set_height_request(22);
        artist.set_vexpand(false);
        album.set_height_request(22);
        album.set_vexpand(false);

        home_progress_stack.set_height_request(22);
        home_progress_stack.set_vexpand(false);

        time_row.set_height_request(18);
        time_row.set_vexpand(false);

        controls.set_height_request(52);
        controls.set_vexpand(false);
        controls.set_valign(gtk::Align::Center);

        let metadata_block = gtk::Box::new(gtk::Orientation::Vertical, 6);
        metadata_block.set_size_request(PLAYER_SURFACE_WIDTH, 92);
        metadata_block.set_hexpand(false);
        metadata_block.set_vexpand(false);
        metadata_block.set_valign(gtk::Align::Start);
        metadata_block.set_overflow(gtk::Overflow::Hidden);
        metadata_block.add_css_class("stable-player-metadata");
        metadata_block.add_css_class("player-metadata-surface");
        metadata_block.append(&title_row);
        metadata_block.append(&artist);
        metadata_block.append(&album);

        let transport_block = gtk::Box::new(gtk::Orientation::Vertical, 6);
        transport_block.set_size_request(PLAYER_SURFACE_WIDTH, 116);
        transport_block.set_hexpand(false);
        transport_block.set_vexpand(false);
        transport_block.set_valign(gtk::Align::Start);
        transport_block.set_overflow(gtk::Overflow::Hidden);
        transport_block.add_css_class("stable-player-transport");
        transport_block.add_css_class("player-transport-surface");
        transport_block.append(&home_progress_stack);
        transport_block.append(&time_row);
        transport_block.append(&controls);

        let now_content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        now_content.set_width_request(PLAYER_SURFACE_WIDTH);
        now_content.set_hexpand(false);
        now_content.set_halign(gtk::Align::Center);
        now_content.set_vexpand(false);
        now_content.set_valign(gtk::Align::Start);
        now_content.set_overflow(gtk::Overflow::Hidden);
        now_content.add_css_class("stable-player-content");
        now_content.add_css_class("expressive-player-content");
        now_content.append(&now_header);
        now_content.append(&hero_cover_slot);
        now_content.append(&metadata_block);
        now_content.append(&transport_block);
        now_content.append(&visualizer_slot);
        now_content.append(&lyrics_slot);

        let now_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        now_card.set_size_request(PLAYER_CARD_WIDTH, -1);
        now_card.set_hexpand(false);
        now_card.set_vexpand(true);
        now_card.set_valign(gtk::Align::Fill);
        now_card.set_overflow(gtk::Overflow::Hidden);
        now_card.add_css_class("now-playing-card");
        now_card.add_css_class("stable-home-player");
        now_card.add_css_class("expressive-player-card");
        now_card.append(&now_content);

        let handle = PlayerViewHandle {
            title: title.clone(),
            artist: artist.clone(),
            album: album.clone(),
            favorite_icon: favorite_icon.clone(),
            hero_play_icon: hero_play_icon.clone(),
            lyrics: lyrics.clone(),
            lyrics_slot: lyrics_slot.clone(),
            lyrics_toggle_button: inline_lyrics_button.clone(),
            visualizer: visualizer.clone(),
            metadata_transition: TransitionClock::new(),
        };

        Self {
            handle,
            root: now_card,
            artist,
            album,
            now_heading,
            favorite_button: favorite,
            previous_button: previous,
            hero_play_button,
            next_button: next,
            transport_motion,
            inline_lyrics_button,
            refresh_lyrics_button,
            hero_cover,
            hero_play_icon,
            favorite_icon,
            progress,
            home_progress_stack,
            home_wave_progress,
            elapsed,
            duration,
            repeat_button: repeat,
            shuffle_button: shuffle,
            visualizer,
            lyrics,
        }
    }
}
