// playback_resume_preferences_fix_v1
use crate::{
    config::{AppConfig, AppLanguage, BlurMode, FooterMode, StartupSource, VisualTheme},
    i18n::{self, Message},
};
use adw::prelude::*;
use gtk::glib;
use std::{cell::RefCell, rc::Rc, time::Duration};

// material_expressive_remaining_interface_v1
fn inherit_visual_theme(parent: &adw::ApplicationWindow, widget: &impl IsA<gtk::Widget>) {
    widget.remove_css_class("theme-noctalia");
    widget.remove_css_class("theme-material-expressive");
    widget.add_css_class(if parent.has_css_class("theme-material-expressive") {
        "theme-material-expressive"
    } else {
        "theme-noctalia"
    });
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SettingsEvent {
    Language(AppLanguage),
    StartupSource(StartupSource),
    BlurMode(BlurMode),
    BlurOpacityPreview(f64),
    BlurOpacityCommit(f64),
    ShowHomeVisualizer(bool),
    ShowHomeLyrics(bool),
    ShowPersonalizedHomeHistory(bool),
    CollectListeningHistory(bool),
    ClearListeningHistory,
    VisualTheme(VisualTheme),
    FooterMode(FooterMode),
    ExpressiveTransportEffects(bool),
    ExpressiveHomeCardEffects(bool),
    AutoDownloadLyrics(bool),
    ResumePlaybackOnStartup(bool),
    YouTubeAutoSync(bool),
    NoctaliaThemeSync(bool),
    ManageYouTube,
}

#[derive(Clone, Copy)]
enum ToggleSetting {
    Visualizer,
    Lyrics,
    AutoLyrics,
    YouTubeSync,
    Noctalia,
}

#[allow(dead_code)]
pub(crate) fn present_settings<F>(
    parent: &adw::ApplicationWindow,
    initial: &AppConfig,
    noctalia_available: bool,
    on_event: F,
) where
    F: Fn(SettingsEvent) + 'static,
{
    let tr = |message| i18n::text(initial.language, message);
    let emit: Rc<dyn Fn(SettingsEvent)> = Rc::new(on_event);

    let dialog = adw::Dialog::builder()
        .title(tr(Message::SettingsTitle))
        .content_width(640)
        .content_height(720)
        .build();
    dialog.add_css_class("settings-dialog");
    inherit_visual_theme(parent, &dialog);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_css_class("material-dialog-toolbar");
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.add_css_class("settings-content");
    scrolled.set_child(Some(&content));
    toolbar.set_content(Some(&scrolled));

    // m3_settings_explicit_shell_fix_v2
    // Style a real child widget instead of relying on AdwDialog's
    // internal presentation nodes, which vary across libadwaita modes.
    toolbar.add_css_class("settings-dialog-surface");
    inherit_visual_theme(parent, &toolbar);
    toolbar.set_hexpand(true);
    toolbar.set_vexpand(true);

    let dialog_shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dialog_shell.add_css_class("settings-dialog-shell");
    inherit_visual_theme(parent, &dialog_shell);
    dialog_shell.set_hexpand(true);
    dialog_shell.set_vexpand(true);
    dialog_shell.set_overflow(gtk::Overflow::Hidden);
    dialog_shell.append(&toolbar);
    dialog.set_child(Some(&dialog_shell));
    content.set_margin_top(22);
    content.set_margin_bottom(22);
    content.set_margin_start(22);
    content.set_margin_end(22);

    let title = gtk::Label::new(Some(tr(Message::SettingsTitle)));
    title.set_xalign(0.0);
    title.add_css_class("title-2");
    title.add_css_class("settings-title");

    let description = gtk::Label::new(Some(tr(Message::SettingsDescription)));
    description.set_xalign(0.0);
    description.set_wrap(true);
    description.add_css_class("dim-label");
    description.add_css_class("settings-description");

    let hero_icon = gtk::Image::from_icon_name("applications-multimedia-symbolic");
    hero_icon.set_pixel_size(28);
    hero_icon.add_css_class("settings-hero-icon");

    let hero_icon_container = gtk::CenterBox::new();
    hero_icon_container.set_center_widget(Some(&hero_icon));
    hero_icon_container.add_css_class("settings-hero-icon-container");

    let hero_copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
    hero_copy.set_hexpand(true);
    hero_copy.add_css_class("settings-hero-copy");
    hero_copy.append(&title);
    hero_copy.append(&description);

    let version_badge = gtk::Label::new(Some(&format!("Nocky {}", env!("CARGO_PKG_VERSION"))));
    version_badge.set_valign(gtk::Align::Center);
    version_badge.add_css_class("settings-version-badge");

    let hero = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    hero.add_css_class("settings-hero");
    hero.append(&hero_icon_container);
    hero.append(&hero_copy);
    hero.append(&version_badge);
    content.append(&hero);

    let group_text = |pt: &'static str, en: &'static str, es: &'static str| match initial.language {
        AppLanguage::Portuguese => pt,
        AppLanguage::English => en,
        AppLanguage::Spanish => es,
    };

    let (general_group, general_rows) = settings_group(
        "preferences-system-symbolic",
        group_text("Geral", "General", "General"),
        group_text(
            "Idioma e comportamento inicial do aplicativo",
            "Language and initial application behavior",
            "Idioma y comportamiento inicial de la aplicación",
        ),
    );
    let (appearance_group, appearance_rows) = settings_group(
        "applications-graphics-symbolic",
        group_text("Aparência", "Appearance", "Apariencia"),
        group_text(
            "Tema visual, desfoque e integração com o sistema",
            "Visual theme, blur and system integration",
            "Tema visual, desenfoque e integración con el sistema",
        ),
    );
    let (playback_group, playback_rows) = settings_group(
        "media-playback-start-symbolic",
        group_text(
            "Reprodução e Home",
            "Playback and Home",
            "Reproducción e inicio",
        ),
        group_text(
            "Player, footer e conteúdo exibido na tela inicial",
            "Player, footer and content shown on the home screen",
            "Reproductor, pie y contenido mostrado en el inicio",
        ),
    );
    let (lyrics_group, lyrics_rows) = settings_group(
        "audio-input-microphone-symbolic",
        group_text("Letras", "Lyrics", "Letras"),
        group_text(
            "Download e comportamento das letras sincronizadas",
            "Download and behavior of synchronized lyrics",
            "Descarga y comportamiento de las letras sincronizadas",
        ),
    );
    let (youtube_group, youtube_rows) = settings_group(
        "folder-remote-symbolic",
        "YouTube Music",
        group_text(
            "Sincronização, conta e biblioteca online",
            "Synchronization, account and online library",
            "Sincronización, cuenta y biblioteca en línea",
        ),
    );

    content.append(&general_group);
    content.append(&appearance_group);
    content.append(&playback_group);
    content.append(&lyrics_group);
    content.append(&youtube_group);

    let language = gtk::DropDown::from_strings(&[
        AppLanguage::Portuguese.label(),
        AppLanguage::English.label(),
        AppLanguage::Spanish.label(),
    ]);
    language.set_selected(match initial.language {
        AppLanguage::Portuguese => 0,
        AppLanguage::English => 1,
        AppLanguage::Spanish => 2,
    });
    general_rows.append(&dropdown_row(
        tr(Message::Language),
        tr(Message::LanguageDescription),
        &language,
    ));

    let source = gtk::DropDown::from_strings(&[tr(Message::LocalLibrary), "YouTube Music"]);
    source.set_selected(
        match initial.startup_source.unwrap_or(StartupSource::YouTube) {
            StartupSource::Local => 0,
            StartupSource::YouTube => 1,
        },
    );
    general_rows.append(&dropdown_row(
        tr(Message::HomeSource),
        tr(Message::HomeSourceDescription),
        &source,
    ));

    let blur_mode = if noctalia_available {
        gtk::DropDown::from_strings(&[
            tr(Message::BlurCustom),
            tr(Message::BlurNoctalia),
            tr(Message::BlurOff),
        ])
    } else {
        gtk::DropDown::from_strings(&[tr(Message::BlurCustom), tr(Message::BlurOff)])
    };
    blur_mode.set_selected(if noctalia_available {
        match initial.blur_mode {
            BlurMode::Custom => 0,
            BlurMode::Noctalia => 1,
            BlurMode::Off => 2,
        }
    } else {
        match initial.blur_mode {
            BlurMode::Off => 1,
            _ => 0,
        }
    });
    appearance_rows.append(&dropdown_row(
        tr(Message::WindowBlur),
        tr(Message::WindowBlurDescription),
        &blur_mode,
    ));

    let blur_opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 45.0, 95.0, 1.0);
    blur_opacity.set_draw_value(true);
    blur_opacity.set_value(initial.blur_opacity.clamp(0.45, 0.95) * 100.0);
    blur_opacity.set_value_pos(gtk::PositionType::Right);

    let blur_opacity_row = scale_row(
        tr(Message::BlurOpacity),
        tr(Message::BlurOpacityDescription),
        &blur_opacity,
    );
    blur_opacity_row.set_visible(initial.blur_mode == BlurMode::Custom);
    appearance_rows.append(&blur_opacity_row);

    let visualizer = settings_switch(initial.show_home_visualizer);
    playback_rows.append(&switch_row(
        tr(Message::HomeVisualizer),
        tr(Message::HomeVisualizerDescription),
        &visualizer,
    ));

    let lyrics = settings_switch(initial.show_home_lyrics);
    playback_rows.append(&switch_row(
        tr(Message::HomeLyrics),
        tr(Message::HomeLyricsDescription),
        &lyrics,
    ));

    let visual_theme = gtk::DropDown::from_strings(&["Noctalia", "Material 3 Expressive"]);
    visual_theme.set_selected(match initial.visual_theme {
        VisualTheme::Noctalia => 0,
        VisualTheme::MaterialExpressive => 1,
    });
    appearance_rows.append(&dropdown_row(
        tr(Message::M3Progress),
        tr(Message::M3ProgressDescription),
        &visual_theme,
    ));

    let footer_mode = gtk::DropDown::from_strings(&[
        tr(Message::FooterAutomatic),
        tr(Message::FooterFull),
        tr(Message::FooterCompact),
        tr(Message::FooterHidden),
    ]);
    footer_mode.set_selected(match initial.footer_mode {
        FooterMode::Automatic => 0,
        FooterMode::Full => 1,
        FooterMode::Compact => 2,
        FooterMode::Hidden => 3,
    });
    playback_rows.append(&dropdown_row(
        tr(Message::FooterMode),
        tr(Message::FooterModeDescription),
        &footer_mode,
    ));

    let auto_lyrics = settings_switch(initial.auto_download_lyrics);
    lyrics_rows.append(&switch_row(
        tr(Message::AutoLyrics),
        tr(Message::AutoLyricsDescription),
        &auto_lyrics,
    ));

    let youtube_sync = settings_switch(initial.youtube_auto_sync);
    youtube_rows.append(&switch_row(
        tr(Message::YoutubeSync),
        tr(Message::YoutubeSyncDescription),
        &youtube_sync,
    ));

    let youtube_button = gtk::Button::with_label(tr(Message::YoutubeManageAction));
    youtube_button.add_css_class("suggested-action");
    youtube_button.add_css_class("settings-primary-action");
    youtube_rows.append(&button_row(
        tr(Message::YoutubeManage),
        tr(Message::YoutubeManageDescription),
        &youtube_button,
    ));

    let noctalia = settings_switch(initial.noctalia_theme_sync && noctalia_available);
    noctalia.set_sensitive(noctalia_available);
    let noctalia_row = switch_row(
        tr(Message::NoctaliaSync),
        tr(Message::NoctaliaSyncDescription),
        &noctalia,
    );
    noctalia_row.set_sensitive(noctalia_available);
    appearance_rows.append(&noctalia_row);

    {
        let emit = emit.clone();
        let dialog = dialog.clone();
        language.connect_selected_notify(move |dropdown| {
            let language = match dropdown.selected() {
                1 => AppLanguage::English,
                2 => AppLanguage::Spanish,
                _ => AppLanguage::Portuguese,
            };
            emit(SettingsEvent::Language(language));
            dialog.close();
        });
    }

    {
        let emit = emit.clone();
        source.connect_selected_notify(move |dropdown| {
            emit(SettingsEvent::StartupSource(if dropdown.selected() == 0 {
                StartupSource::Local
            } else {
                StartupSource::YouTube
            }));
        });
    }

    {
        let emit = emit.clone();
        let opacity_row = blur_opacity_row.clone();
        blur_mode.connect_selected_notify(move |dropdown| {
            let mode = if noctalia_available {
                match dropdown.selected() {
                    0 => BlurMode::Custom,
                    2 => BlurMode::Off,
                    _ => BlurMode::Noctalia,
                }
            } else if dropdown.selected() == 0 {
                BlurMode::Custom
            } else {
                BlurMode::Off
            };
            opacity_row.set_visible(mode == BlurMode::Custom);
            emit(SettingsEvent::BlurMode(mode));
        });
    }

    {
        let emit = emit.clone();
        let pending_save = Rc::new(RefCell::new(None::<glib::SourceId>));
        blur_opacity.connect_value_changed(move |scale| {
            let value = (scale.value() / 100.0).clamp(0.45, 0.95);
            emit(SettingsEvent::BlurOpacityPreview(value));

            if let Some(source) = pending_save.borrow_mut().take() {
                source.remove();
            }

            let emit = emit.clone();
            let pending = pending_save.clone();
            let source = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                pending.borrow_mut().take();
                emit(SettingsEvent::BlurOpacityCommit(value));
            });
            pending_save.borrow_mut().replace(source);
        });
    }

    {
        let emit = emit.clone();
        youtube_button.connect_clicked(move |_| emit(SettingsEvent::ManageYouTube));
    }

    {
        let emit = emit.clone();
        visual_theme.connect_selected_notify(move |dropdown| {
            emit(SettingsEvent::VisualTheme(if dropdown.selected() == 1 {
                VisualTheme::MaterialExpressive
            } else {
                VisualTheme::Noctalia
            }));
        });
    }

    {
        let emit = emit.clone();
        footer_mode.connect_selected_notify(move |dropdown| {
            let mode = match dropdown.selected() {
                1 => FooterMode::Full,
                2 => FooterMode::Compact,
                3 => FooterMode::Hidden,
                _ => FooterMode::Automatic,
            };
            emit(SettingsEvent::FooterMode(mode));
        });
    }

    for (switch, setting) in [
        (&visualizer, ToggleSetting::Visualizer),
        (&lyrics, ToggleSetting::Lyrics),
        (&auto_lyrics, ToggleSetting::AutoLyrics),
        (&youtube_sync, ToggleSetting::YouTubeSync),
        (&noctalia, ToggleSetting::Noctalia),
    ] {
        let emit = emit.clone();
        switch.connect_active_notify(move |switch| {
            let active = switch.is_active();
            let event = match setting {
                ToggleSetting::Visualizer => SettingsEvent::ShowHomeVisualizer(active),
                ToggleSetting::Lyrics => SettingsEvent::ShowHomeLyrics(active),
                ToggleSetting::AutoLyrics => SettingsEvent::AutoDownloadLyrics(active),
                ToggleSetting::YouTubeSync => SettingsEvent::YouTubeAutoSync(active),
                ToggleSetting::Noctalia => SettingsEvent::NoctaliaThemeSync(active),
            };
            emit(event);
        });
    }

    dialog.present(Some(parent));
}

pub(crate) fn present_youtube_settings<W>(parent: &adw::ApplicationWindow, root: &W)
where
    W: IsA<gtk::Widget> + Clone + 'static,
{
    let dialog = adw::Dialog::builder()
        .title("YouTube Music")
        .content_width(760)
        .content_height(620)
        .build();
    dialog.add_css_class("youtube-settings-dialog");
    inherit_visual_theme(parent, &dialog);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_css_class("material-dialog-toolbar");
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.add_css_class("youtube-settings-host");
    host.append(root);
    toolbar.set_content(Some(&host));
    dialog.set_child(Some(&toolbar));

    let youtube_root = root.clone();
    dialog.connect_closed(move |_| {
        host.remove(&youtube_root);
    });
    dialog.present(Some(parent));
}

pub(crate) fn present_startup_source<F>(
    parent: &adw::ApplicationWindow,
    language: AppLanguage,
    first_run: bool,
    on_select: F,
) where
    F: Fn(StartupSource) + 'static,
{
    let tr = |message| i18n::text(language, message);
    let on_select: Rc<dyn Fn(StartupSource)> = Rc::new(on_select);

    let dialog = adw::Dialog::builder()
        .title(if first_run {
            tr(Message::StartupWelcome)
        } else {
            tr(Message::StartupSourceTitle)
        })
        .content_width(480)
        .build();
    dialog.add_css_class("startup-dialog");
    inherit_visual_theme(parent, &dialog);
    dialog.set_can_close(!first_run);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.add_css_class("startup-dialog-content");
    dialog.set_child(Some(&content));
    content.set_margin_top(22);
    content.set_margin_bottom(22);
    content.set_margin_start(22);
    content.set_margin_end(22);

    let title = gtk::Label::new(Some(if first_run {
        tr(Message::StartupQuestion)
    } else {
        tr(Message::StartupChoose)
    }));
    title.set_wrap(true);
    title.set_xalign(0.0);
    title.add_css_class("title-2");
    title.add_css_class("startup-dialog-title");

    let description = gtk::Label::new(Some(tr(Message::StartupDescription)));
    description.set_wrap(true);
    description.set_xalign(0.0);
    description.add_css_class("dim-label");
    description.add_css_class("startup-dialog-description");

    let local_button = gtk::Button::with_label(tr(Message::UseLocalLibrary));
    local_button.set_tooltip_text(Some(tr(Message::UseLocalLibraryTooltip)));
    local_button.add_css_class("source-choice-button");

    let youtube_button = gtk::Button::with_label(tr(Message::UseYoutubeMusic));
    youtube_button.set_tooltip_text(Some(tr(Message::UseYoutubeMusicTooltip)));
    youtube_button.add_css_class("source-choice-button");
    youtube_button.add_css_class("suggested-action");

    let choices = gtk::Box::new(gtk::Orientation::Vertical, 10);
    choices.add_css_class("startup-choice-group");
    choices.append(&local_button);
    choices.append(&youtube_button);

    content.append(&title);
    content.append(&description);
    content.append(&choices);

    if !first_run {
        let cancel_button = gtk::Button::with_label(tr(Message::Cancel));
        cancel_button.set_halign(gtk::Align::End);
        cancel_button.add_css_class("startup-cancel-action");
        content.append(&cancel_button);

        let dialog = dialog.clone();
        cancel_button.connect_clicked(move |_| {
            dialog.close();
        });
    }

    {
        let on_select = on_select.clone();
        let dialog = dialog.clone();
        local_button.connect_clicked(move |_| {
            on_select(StartupSource::Local);
            dialog.close();
        });
    }

    {
        let on_select = on_select.clone();
        let dialog = dialog.clone();
        youtube_button.connect_clicked(move |_| {
            on_select(StartupSource::YouTube);
            dialog.close();
        });
    }

    dialog.present(Some(parent));
}

// organized_settings_milestone_v1
fn settings_group(icon_name: &str, title: &str, description: &str) -> (gtk::Box, gtk::Box) {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(20);
    icon.add_css_class("settings-group-icon");

    let icon_container = gtk::CenterBox::new();
    icon_container.set_center_widget(Some(&icon));
    icon_container.add_css_class("settings-group-icon-container");

    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("settings-group-title");

    let description_label = gtk::Label::new(Some(description));
    description_label.set_xalign(0.0);
    description_label.set_wrap(true);
    description_label.add_css_class("settings-group-description");

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 2);
    copy.set_hexpand(true);
    copy.add_css_class("settings-group-copy");
    copy.append(&title_label);
    copy.append(&description_label);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.add_css_class("settings-group-header");
    header.append(&icon_container);
    header.append(&copy);

    let rows = gtk::Box::new(gtk::Orientation::Vertical, 2);
    rows.add_css_class("settings-group-rows");

    let group = gtk::Box::new(gtk::Orientation::Vertical, 6);
    group.add_css_class("settings-group");
    group.append(&header);
    group.append(&rows);

    (group, rows)
}
fn settings_switch(active: bool) -> gtk::Switch {
    let switch = gtk::Switch::builder()
        .active(active)
        .valign(gtk::Align::Center)
        .build();
    switch.add_css_class("settings-switch");
    switch
}

fn switch_row(title: &str, subtitle: &str, switch: &gtk::Switch) -> gtk::Box {
    row_with_control(title, subtitle, switch)
}

fn dropdown_row(title: &str, subtitle: &str, dropdown: &gtk::DropDown) -> gtk::Box {
    dropdown.set_valign(gtk::Align::Center);
    dropdown.set_width_request(170);
    dropdown.add_css_class("settings-dropdown");
    row_with_control(title, subtitle, dropdown)
}

fn scale_row(title: &str, subtitle: &str, scale: &gtk::Scale) -> gtk::Box {
    scale.set_valign(gtk::Align::Center);
    scale.set_width_request(190);
    scale.add_css_class("settings-scale");
    row_with_control(title, subtitle, scale)
}

fn button_row(title: &str, subtitle: &str, button: &gtk::Button) -> gtk::Box {
    button.set_valign(gtk::Align::Center);
    button.add_css_class("settings-row-action");
    row_with_control(title, subtitle, button)
}

fn row_with_control(title: &str, subtitle: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("track-title");
    title_label.add_css_class("settings-row-title");

    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    subtitle_label.add_css_class("dim-label");
    subtitle_label.add_css_class("settings-row-subtitle");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.add_css_class("settings-row-text");
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-row");
    row.add_css_class("settings-surface-row");
    row.append(&text);
    row.append(control);
    row
}
