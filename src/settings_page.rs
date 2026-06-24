use crate::{
    config::{AppConfig, AppLanguage, BlurMode, FooterMode, StartupSource, VisualTheme},
    dialogs::SettingsEvent,
    i18n::{self, Message},
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

// navigable_settings_page_v1
pub(crate) struct SettingsPage {
    root: gtk::ScrolledWindow,
    sender: Sender<SettingsEvent>,
    receiver: Receiver<SettingsEvent>,
    rebuilding: RefCell<bool>,
}

impl SettingsPage {
    pub(crate) fn new(initial: &AppConfig, noctalia_available: bool) -> Rc<Self> {
        let (sender, receiver) = mpsc::channel();

        let root = gtk::ScrolledWindow::new();
        root.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        root.set_vexpand(true);
        root.set_hexpand(true);
        root.add_css_class("settings-page-scroll");

        let page = Rc::new(Self {
            root,
            sender,
            receiver,
            rebuilding: RefCell::new(false),
        });
        page.rebuild(initial, noctalia_available);
        page
    }

    pub(crate) fn root(&self) -> &gtk::ScrolledWindow {
        &self.root
    }

    pub(crate) fn try_recv(&self) -> Option<SettingsEvent> {
        self.receiver.try_recv().ok()
    }

    pub(crate) fn rebuild(&self, initial: &AppConfig, noctalia_available: bool) {
        if *self.rebuilding.borrow() {
            return;
        }
        *self.rebuilding.borrow_mut() = true;
        self.root.set_child(Some(&build_content(
            initial,
            noctalia_available,
            self.sender.clone(),
        )));
        *self.rebuilding.borrow_mut() = false;
    }
}

fn build_content(
    initial: &AppConfig,
    noctalia_available: bool,
    sender: Sender<SettingsEvent>,
) -> gtk::Box {
    let tr = |message| i18n::text(initial.language, message);
    let emit: Rc<dyn Fn(SettingsEvent)> = Rc::new(move |event| {
        let _ = sender.send(event);
    });

    let page = gtk::Box::new(gtk::Orientation::Vertical, 16);
    page.set_width_request(760);
    page.set_halign(gtk::Align::Center);
    page.set_margin_top(28);
    page.set_margin_bottom(34);
    page.set_margin_start(24);
    page.set_margin_end(24);
    page.add_css_class("settings-page");
    page.add_css_class("settings-content");

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
    page.append(&hero);

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

    // settings_about_and_remove_overflow_v1
    let (about_group, about_rows) = settings_group(
        "help-about-symbolic",
        group_text("Sobre", "About", "Acerca de"),
        group_text(
            "Informações do aplicativo, versão e licença",
            "Application information, version and license",
            "Información de la aplicación, versión y licencia",
        ),
    );
    about_group.add_css_class("settings-about-group");

    page.append(&general_group);
    page.append(&appearance_group);
    page.append(&playback_group);
    page.append(&lyrics_group);
    page.append(&youtube_group);
    page.append(&about_group);

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

    let noctalia = settings_switch(initial.noctalia_theme_sync && noctalia_available);
    noctalia.set_sensitive(noctalia_available);
    let noctalia_row = switch_row(
        tr(Message::NoctaliaSync),
        tr(Message::NoctaliaSyncDescription),
        &noctalia,
    );
    noctalia_row.set_sensitive(noctalia_available);
    appearance_rows.append(&noctalia_row);

    // nocky_expressive_settings_in_appearance_v1: Expressive controls belong to Appearance
    // nocky_theme_scoped_expressive_effects_v1: Expressive controls are Material-only
    let material_expressive = initial.visual_theme == VisualTheme::MaterialExpressive;

    let expressive_transport = settings_switch(initial.expressive_transport_effects);
    let expressive_transport_row = switch_row(
        group_text(
            "Animações expressivas de reprodução",
            "Expressive playback animations",
            "Animaciones expresivas de reproducción",
        ),
        group_text(
            "Expande e reorganiza os controles com efeito de mola no modo Material 3. Desative para usar o comportamento clássico.",
            "Expands and rearranges the controls with spring motion in Material 3 mode. Disable it to use the classic behavior.",
            "Expande y reorganiza los controles con movimiento de resorte en el modo Material 3. Desactívalo para usar el comportamiento clásico.",
        ),
        &expressive_transport,
    );
    expressive_transport_row.set_visible(material_expressive);
    appearance_rows.append(&expressive_transport_row);

    let expressive_home_cards = settings_switch(initial.expressive_home_card_effects);
    let expressive_home_cards_row = switch_row(
        group_text(
            "Resposta expressiva dos carrosséis",
            "Expressive carousel response",
            "Respuesta expresiva de los carruseles",
        ),
        group_text(
            "Ativa a resposta elástica nas extremidades dos carrosséis da Home no tema Material 3.",
            "Enables elastic edge feedback for Home carousels in the Material 3 theme.",
            "Activa la respuesta elástica en los extremos de los carruseles de inicio con el tema Material 3.",
        ),
        &expressive_home_cards,
    );
    expressive_home_cards_row.set_visible(material_expressive);
    appearance_rows.append(&expressive_home_cards_row);

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

    let personalized_history = settings_switch(initial.show_personalized_home_history);
    playback_rows.append(&switch_row(
        group_text(
            "Histórico personalizado na Home",
            "Personalized history on Home",
            "Historial personalizado en inicio",
        ),
        group_text(
            "Exibe Ouvidos recentemente com faixas, álbuns e playlists em ordem cronológica. O histórico salvo é preservado quando esta opção está desativada.",
            "Shows Recently listened with tracks, albums and playlists in chronological order. Saved history is preserved while this option is disabled.",
            "Muestra Escuchados recientemente con canciones, álbumes y playlists en orden cronológico. El historial guardado se conserva mientras esta opción está desactivada.",
        ),
        &personalized_history,
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

    let about_button = gtk::Button::with_label(group_text(
        "Ver informações",
        "View details",
        "Ver información",
    ));
    about_button.set_action_name(Some("app.about"));
    // noctalia_about_action_release_polish_v1
    about_button.add_css_class("settings-primary-action");
    about_button.add_css_class("settings-row-action");
    about_button.add_css_class("settings-about-action");

    let about_subtitle = format!("v{} · GPL-3.0", env!("CARGO_PKG_VERSION"));
    about_rows.append(&button_row("Nocky", &about_subtitle, &about_button));

    let shortcuts_button =
        gtk::Button::with_label(group_text("Ver atalhos", "View shortcuts", "Ver atajos"));
    shortcuts_button.set_action_name(Some("app.shortcuts"));
    shortcuts_button.add_css_class("settings-row-action");
    shortcuts_button.add_css_class("settings-shortcuts-action");

    about_rows.append(&button_row(
        group_text(
            "Atalhos de teclado",
            "Keyboard shortcuts",
            "Atajos de teclado",
        ),
        group_text(
            "Veja todos os comandos disponíveis em uma lista.",
            "View every available command in a list.",
            "Consulta todos los comandos disponibles en una lista.",
        ),
        &shortcuts_button,
    ));

    {
        let emit = emit.clone();
        language.connect_selected_notify(move |dropdown| {
            let language = match dropdown.selected() {
                1 => AppLanguage::English,
                2 => AppLanguage::Spanish,
                _ => AppLanguage::Portuguese,
            };
            emit(SettingsEvent::Language(language));
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
        let expressive_transport_row = expressive_transport_row.clone();
        let expressive_home_cards_row = expressive_home_cards_row.clone();

        visual_theme.connect_selected_notify(move |dropdown| {
            let material_expressive = dropdown.selected() == 1;
            expressive_transport_row.set_visible(material_expressive);
            expressive_home_cards_row.set_visible(material_expressive);

            emit(SettingsEvent::VisualTheme(if material_expressive {
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

    {
        let emit = emit.clone();
        youtube_button.connect_clicked(move |_| emit(SettingsEvent::ManageYouTube));
    }

    for (switch, setting) in [
        (&visualizer, ToggleSetting::Visualizer),
        (&lyrics, ToggleSetting::Lyrics),
        (
            &personalized_history,
            ToggleSetting::PersonalizedHomeHistory,
        ),
        (&auto_lyrics, ToggleSetting::AutoLyrics),
        (&youtube_sync, ToggleSetting::YouTubeSync),
        (&expressive_home_cards, ToggleSetting::ExpressiveHomeCards),
        (&expressive_transport, ToggleSetting::ExpressiveTransport),
        (&noctalia, ToggleSetting::Noctalia),
    ] {
        let emit = emit.clone();
        switch.connect_active_notify(move |switch| {
            let active = switch.is_active();
            let event = match setting {
                ToggleSetting::Visualizer => SettingsEvent::ShowHomeVisualizer(active),
                ToggleSetting::Lyrics => SettingsEvent::ShowHomeLyrics(active),
                ToggleSetting::PersonalizedHomeHistory => {
                    SettingsEvent::ShowPersonalizedHomeHistory(active)
                }
                ToggleSetting::AutoLyrics => SettingsEvent::AutoDownloadLyrics(active),
                ToggleSetting::YouTubeSync => SettingsEvent::YouTubeAutoSync(active),
                ToggleSetting::ExpressiveTransport => {
                    SettingsEvent::ExpressiveTransportEffects(active)
                }
                ToggleSetting::ExpressiveHomeCards => {
                    SettingsEvent::ExpressiveHomeCardEffects(active)
                }
                ToggleSetting::Noctalia => SettingsEvent::NoctaliaThemeSync(active),
            };
            emit(event);
        });
    }

    page
}

#[derive(Clone, Copy)]
enum ToggleSetting {
    Visualizer,
    Lyrics,
    PersonalizedHomeHistory,
    AutoLyrics,
    YouTubeSync,
    ExpressiveTransport,
    ExpressiveHomeCards,
    Noctalia,
}

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
