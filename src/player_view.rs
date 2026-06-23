use crate::{
    build_cover,
    config::AppLanguage,
    i18n::{self, Message},
    lyrics_view::LyricsPresenter,
    visualizer::SpectrumVisualizer,
    wave_progress::WaveProgress,
    CoverView,
};
use gtk::prelude::*;

pub(crate) struct PlayerView {
    pub(crate) root: gtk::Box,
    pub(crate) title: gtk::Label,
    pub(crate) artist: gtk::Label,
    pub(crate) album: gtk::Label,
    pub(crate) now_heading: gtk::Label,
    pub(crate) favorite_button: gtk::Button,
    pub(crate) previous_button: gtk::Button,
    pub(crate) hero_play_button: gtk::Button,
    pub(crate) next_button: gtk::Button,
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

impl PlayerView {
    pub(crate) fn new(language: AppLanguage) -> Self {
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

        let artist = gtk::Label::new(Some(tr(Message::NoTrackSelected)));
        artist.set_xalign(0.0);
        artist.set_single_line_mode(true);
        artist.set_width_request(384);
        artist.set_width_chars(-1);
        artist.set_max_width_chars(40);
        artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist.set_overflow(gtk::Overflow::Hidden);
        artist.add_css_class("hero-artist");

        let album = gtk::Label::new(Some(tr(Message::ChooseFolderToStart)));
        album.set_xalign(0.0);
        album.set_single_line_mode(true);
        album.set_width_request(384);
        album.set_width_chars(-1);
        album.set_max_width_chars(40);
        album.set_ellipsize(gtk::pango::EllipsizeMode::End);
        album.set_overflow(gtk::Overflow::Hidden);
        album.add_css_class("dim-label");

        let favorite_icon = gtk::Image::from_icon_name("emblem-favorite-symbolic");
        favorite_icon.set_opacity(0.28);
        let favorite = gtk::Button::new();
        favorite.set_child(Some(&favorite_icon));
        favorite.add_css_class("flat");
        favorite.add_css_class("card-icon-button");
        favorite.set_tooltip_text(Some(tr(Message::FavoriteTooltip)));
        favorite.add_css_class("like-button");
        favorite.set_size_request(34, 34);
        favorite.set_hexpand(false);
        favorite.set_vexpand(false);

        let now_heading = gtk::Label::new(Some(tr(Message::NowPlaying)));
        now_heading.set_xalign(0.0);
        now_heading.set_hexpand(true);
        now_heading.add_css_class("now-heading");
        let headphones = gtk::Image::from_icon_name("audio-headphones-symbolic");
        headphones.add_css_class("now-heading-icon");
        let now_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        now_header.append(&now_heading);
        now_header.append(&headphones);

        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        title_row.set_size_request(384, 34);
        title_row.set_hexpand(false);
        title_row.set_vexpand(false);
        title_row.set_overflow(gtk::Overflow::Hidden);
        title_row.append(&title);
        title_row.append(&favorite);

        let hero_cover = build_cover(280);
        hero_cover.stack.set_halign(gtk::Align::Center);

        // Keep placeholder and real artwork in the same balanced vertical slot.
        let hero_cover_slot = gtk::CenterBox::new();
        hero_cover_slot.set_orientation(gtk::Orientation::Vertical);
        hero_cover_slot.set_vexpand(false);
        hero_cover_slot.set_hexpand(true);
        hero_cover_slot.set_margin_top(0);
        hero_cover_slot.set_margin_bottom(0);
        hero_cover_slot.set_height_request(328);
        hero_cover_slot.set_width_request(384);
        hero_cover_slot.set_valign(gtk::Align::Start);
        hero_cover_slot.set_center_widget(Some(&hero_cover.stack));
        hero_cover_slot.add_css_class("hero-cover-slot");
        hero_cover_slot.add_css_class("stable-player-cover-slot");

        let elapsed = gtk::Label::new(Some("0:00"));
        elapsed.add_css_class("time-label");
        let duration = gtk::Label::new(Some("0:00"));
        duration.add_css_class("time-label");
        let progress = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
        progress.set_draw_value(false);
        progress.set_hexpand(true);
        progress.add_css_class("progress-scale");

        let home_wave_progress = WaveProgress::new();
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

        let repeat = gtk::ToggleButton::builder()
            .icon_name("media-playlist-repeat-symbolic")
            .tooltip_text(tr(Message::RepeatTrack))
            .build();
        repeat.add_css_class("media-control");
        let previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        previous.set_tooltip_text(Some(tr(Message::PreviousTrack)));
        previous.add_css_class("media-control");

        let hero_play_icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
        hero_play_icon.set_pixel_size(24);
        let hero_play_button = gtk::Button::new();
        hero_play_button.set_child(Some(&hero_play_icon));
        hero_play_button.add_css_class("shell-play-button");
        hero_play_button.set_tooltip_text(Some(tr(Message::PlayPause)));

        let next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next.set_tooltip_text(Some(tr(Message::NextTrack)));
        next.add_css_class("media-control");
        let shuffle = gtk::ToggleButton::builder()
            .icon_name("media-playlist-shuffle-symbolic")
            .tooltip_text(tr(Message::Shuffle))
            .build();
        shuffle.add_css_class("media-control");

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        controls.set_halign(gtk::Align::Center);
        controls.append(&repeat);
        controls.append(&previous);
        controls.append(&hero_play_button);
        controls.append(&next);
        controls.append(&shuffle);

        let visualizer = SpectrumVisualizer::new();
        let lyrics = LyricsPresenter::new();

        // stable_home_player_layout_v1
        // stable_standby_slots_v1
        let visualizer_widget = visualizer.widget().clone();
        visualizer_widget.set_size_request(384, 74);
        visualizer_widget.set_hexpand(false);
        visualizer_widget.set_halign(gtk::Align::Center);
        visualizer_widget.set_vexpand(false);
        visualizer_widget.set_valign(gtk::Align::Center);

        let visualizer_slot = gtk::CenterBox::new();
        visualizer_slot.set_orientation(gtk::Orientation::Vertical);
        visualizer_slot.set_size_request(384, 74);
        visualizer_slot.set_hexpand(false);
        visualizer_slot.set_halign(gtk::Align::Center);
        visualizer_slot.set_vexpand(false);
        visualizer_slot.set_valign(gtk::Align::Start);
        visualizer_slot.set_center_widget(Some(&visualizer_widget));
        visualizer_slot.set_overflow(gtk::Overflow::Hidden);
        visualizer_slot.add_css_class("stable-visualizer-slot");

        let inline_lyrics_widget = lyrics.inline_widget().clone();
        inline_lyrics_widget.set_size_request(384, 158);
        inline_lyrics_widget.set_hexpand(false);
        inline_lyrics_widget.set_halign(gtk::Align::Center);
        inline_lyrics_widget.set_vexpand(false);
        inline_lyrics_widget.set_valign(gtk::Align::Center);

        let lyrics_slot = gtk::CenterBox::new();
        lyrics_slot.set_orientation(gtk::Orientation::Vertical);
        lyrics_slot.set_size_request(384, 158);
        lyrics_slot.set_hexpand(false);
        lyrics_slot.set_halign(gtk::Align::Center);
        lyrics_slot.set_vexpand(false);
        lyrics_slot.set_valign(gtk::Align::Start);
        lyrics_slot.set_center_widget(Some(&inline_lyrics_widget));
        lyrics_slot.set_overflow(gtk::Overflow::Hidden);
        lyrics_slot.add_css_class("stable-lyrics-slot");

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
        metadata_block.set_size_request(384, 92);
        metadata_block.set_hexpand(false);
        metadata_block.set_vexpand(false);
        metadata_block.set_valign(gtk::Align::Start);
        metadata_block.set_overflow(gtk::Overflow::Hidden);
        metadata_block.add_css_class("stable-player-metadata");
        metadata_block.append(&title_row);
        metadata_block.append(&artist);
        metadata_block.append(&album);

        let transport_block = gtk::Box::new(gtk::Orientation::Vertical, 6);
        transport_block.set_size_request(384, 116);
        transport_block.set_hexpand(false);
        transport_block.set_vexpand(false);
        transport_block.set_valign(gtk::Align::Start);
        transport_block.set_overflow(gtk::Overflow::Hidden);
        transport_block.add_css_class("stable-player-transport");
        transport_block.append(&home_progress_stack);
        transport_block.append(&time_row);
        transport_block.append(&controls);

        let now_content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        now_content.set_width_request(384);
        now_content.set_hexpand(false);
        now_content.set_halign(gtk::Align::Center);
        now_content.set_vexpand(false);
        now_content.set_valign(gtk::Align::Start);
        now_content.set_overflow(gtk::Overflow::Hidden);
        now_content.add_css_class("stable-player-content");
        now_content.append(&now_header);
        now_content.append(&hero_cover_slot);
        now_content.append(&metadata_block);
        now_content.append(&transport_block);
        now_content.append(&visualizer_slot);
        now_content.append(&lyrics_slot);

        let now_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        now_card.set_size_request(420, -1);
        now_card.set_hexpand(false);
        now_card.set_vexpand(true);
        now_card.set_valign(gtk::Align::Fill);
        now_card.set_overflow(gtk::Overflow::Hidden);
        now_card.add_css_class("now-playing-card");
        now_card.add_css_class("stable-home-player");
        now_card.append(&now_content);

        Self {
            root: now_card,
            title,
            artist,
            album,
            now_heading,
            favorite_button: favorite,
            previous_button: previous,
            hero_play_button,
            next_button: next,
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
