use crate::{
    config::{AppConfig, AppLanguage, BlurMode, FooterMode, StartupSource, VisualTheme},
    dialogs::SettingsEvent,
    i18n::{self, Message},
    offline_store::OfflineStore,
    ui::widgets::{
        material_button::{
            apply_material_button, set_material_button_loading, set_material_button_selected,
            MaterialButtonSemantic, MaterialButtonSize, MaterialButtonSpec, MaterialButtonVariant,
        },
        material_card::{apply_material_card, MaterialCardSpec, MaterialCardVariant},
        AnimatedPageSpec, AnimatedPageSwitcher,
    },
    youtube::diagnostics::{self as youtube_diagnostics, DiagnosticCheck, DiagnosticState},
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};
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
    apply_material_card(&hero, MaterialCardSpec::new(MaterialCardVariant::Elevated));
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

    let settings_stack = gtk::Stack::new();
    settings_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    settings_stack.set_transition_duration(180);
    settings_stack.set_vexpand(true);
    settings_stack.set_hexpand(true);
    settings_stack.add_css_class("settings-tabs-stack");

    let general_page = settings_tab_page();
    general_page.append(&general_group);
    let appearance_page = settings_tab_page();
    appearance_page.append(&appearance_group);
    let playback_page = settings_tab_page();
    playback_page.append(&playback_group);
    playback_page.append(&lyrics_group);
    let youtube_page = settings_tab_page();
    youtube_page.append(&youtube_group);
    let about_page = settings_tab_page();
    about_page.append(&about_group);

    settings_stack.add_titled(
        &general_page,
        Some("general"),
        group_text("Geral", "General", "General"),
    );
    settings_stack.add_titled(
        &appearance_page,
        Some("appearance"),
        group_text("Aparência", "Appearance", "Apariencia"),
    );
    settings_stack.add_titled(
        &playback_page,
        Some("playback"),
        group_text("Reprodução", "Playback", "Reproducción"),
    );
    settings_stack.add_titled(&youtube_page, Some("youtube"), "YouTube Music");
    settings_stack.add_titled(
        &about_page,
        Some("about"),
        group_text("Sobre", "About", "Acerca de"),
    );
    settings_stack.set_visible_child_name("general");

    let settings_switcher = AnimatedPageSwitcher::from_specs(&[
        AnimatedPageSpec {
            icon_name: "preferences-system-symbolic",
            label: group_text("Geral", "General", "General"),
        },
        AnimatedPageSpec {
            icon_name: "applications-graphics-symbolic",
            label: group_text("Aparência", "Appearance", "Apariencia"),
        },
        AnimatedPageSpec {
            icon_name: "media-playback-start-symbolic",
            label: group_text("Reprodução", "Playback", "Reproducción"),
        },
        AnimatedPageSpec {
            icon_name: "folder-remote-symbolic",
            label: "YouTube Music",
        },
        AnimatedPageSpec {
            icon_name: "help-about-symbolic",
            label: group_text("Sobre", "About", "Acerca de"),
        },
    ]);

    {
        let stack = settings_stack.clone();
        let switcher = settings_switcher.clone();
        settings_switcher.connect_selected(move |index| {
            const PAGE_NAMES: [&str; 5] = ["general", "appearance", "playback", "youtube", "about"];
            if let Some(name) = PAGE_NAMES.get(index) {
                stack.set_visible_child_name(name);
                switcher.set_active_index(index, true);
            }
        });
    }

    {
        let switcher = settings_switcher.clone();
        settings_stack.connect_visible_child_name_notify(move |stack| {
            let index = match stack.visible_child_name().as_deref() {
                Some("appearance") => 1,
                Some("playback") => 2,
                Some("youtube") => 3,
                Some("about") => 4,
                _ => 0,
            };
            switcher.set_active_index(index, true);
        });
    }

    let tab_scroll = gtk::ScrolledWindow::new();
    tab_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    tab_scroll.set_propagate_natural_width(true);
    tab_scroll.set_hexpand(true);
    tab_scroll.set_halign(gtk::Align::Fill);
    settings_switcher.root().set_halign(gtk::Align::Center);
    tab_scroll.set_child(Some(settings_switcher.root()));
    tab_scroll.add_css_class("settings-tabs-scroll");

    page.append(&tab_scroll);
    page.append(&settings_stack);

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

    let visual_theme =
        gtk::DropDown::from_strings(&["Noctalia", "Material 3 Expressive", "Frosted Glass"]);
    visual_theme.set_selected(match initial.visual_theme {
        VisualTheme::Noctalia => 0,
        VisualTheme::MaterialExpressive => 1,
        VisualTheme::FrostedGlass => 2,
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

    // Both expressive themes share motion and interaction controls.
    let expressive_theme = initial.visual_theme.is_expressive();

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
    expressive_transport_row.set_visible(expressive_theme);
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
    expressive_home_cards_row.set_visible(expressive_theme);
    appearance_rows.append(&expressive_home_cards_row);

    let resume_playback = settings_switch(initial.resume_playback_on_startup);
    playback_rows.append(&switch_row(
        group_text(
            "Retomar reprodução automaticamente",
            "Resume playback automatically",
            "Reanudar reproducción automáticamente",
        ),
        group_text(
            "Ao abrir o Nocky, restaura a faixa e a posição anterior e continua tocando. Desativado por padrão; quando desligado, a sessão é restaurada pausada.",
            "When Nocky opens, restores the previous track and position and continues playing. Disabled by default; when off, the session is restored paused.",
            "Al abrir Nocky, restaura la pista y la posición anteriores y continúa reproduciendo. Desactivado por defecto; al apagarlo, la sesión se restaura en pausa.",
        ),
        &resume_playback,
    ));

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

    let collect_history = settings_switch(initial.collect_listening_history);
    playback_rows.append(&switch_row(
        group_text(
            "Aprender com minha atividade",
            "Learn from my listening activity",
            "Aprender de mi actividad",
        ),
        group_text(
            "Registra reproduções para personalizar a Home. Ao desativar, o Nocky para de adicionar novos eventos, mas mantém o histórico existente.",
            "Records plays to personalize Home. When disabled, Nocky stops adding new events but keeps existing history.",
            "Registra reproducciones para personalizar el inicio. Al desactivarlo, Nocky deja de añadir eventos nuevos, pero conserva el historial existente.",
        ),
        &collect_history,
    ));

    let clear_history = gtk::Button::with_label(group_text(
        "Limpar histórico",
        "Clear history",
        "Borrar historial",
    ));
    apply_material_button(
        &clear_history,
        MaterialButtonSpec::new(MaterialButtonVariant::Outlined, MaterialButtonSize::Compact)
            .with_semantic(MaterialButtonSemantic::Destructive),
    );
    playback_rows.append(&button_row(
        group_text(
            "Apagar atividade salva",
            "Delete saved activity",
            "Eliminar actividad guardada",
        ),
        group_text(
            "Remove permanentemente reproduções, progresso retomável e rankings usados pela Home personalizada.",
            "Permanently removes plays, resumable progress and rankings used by personalized Home.",
            "Elimina permanentemente reproducciones, progreso reanudable y rankings usados por el inicio personalizado.",
        ),
        &clear_history,
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

    let offline_collection_auto_sync = settings_switch(initial.offline_collection_auto_sync);
    youtube_rows.append(&switch_row(
        group_text(
            "Manter coleções offline atualizadas",
            "Keep offline collections updated",
            "Mantener colecciones sin conexión actualizadas",
        ),
        group_text(
            "Baixa automaticamente novas faixas das playlists, álbuns e mixes marcados para uso offline.",
            "Automatically download new tracks from playlists, albums, and mixes marked for offline use.",
            "Descarga automáticamente nuevas canciones de playlists, álbumes y mixes marcados para uso sin conexión.",
        ),
        &offline_collection_auto_sync,
    ));

    let youtube_button = gtk::Button::with_label(tr(Message::YoutubeManageAction));
    apply_material_button(
        &youtube_button,
        MaterialButtonSpec::new(MaterialButtonVariant::Filled, MaterialButtonSize::Compact),
    );
    youtube_rows.append(&button_row(
        tr(Message::YoutubeManage),
        tr(Message::YoutubeManageDescription),
        &youtube_button,
    ));

    let offline_store = OfflineStore::load_default();
    let offline_count = offline_store.track_count();
    let offline_bytes = offline_store.total_size_bytes();
    let (partial_count, partial_bytes) = offline_store.partial_stats();
    let offline_path = offline_store.root_dir();

    let storage_summary = gtk::Label::new(Some(&format!(
        "{} · {}",
        format_offline_track_count(initial.language, offline_count),
        format_storage_size(offline_bytes)
    )));
    storage_summary.set_valign(gtk::Align::Center);
    storage_summary.add_css_class("settings-version-badge");
    youtube_rows.append(&row_with_control(
        group_text(
            "Armazenamento offline",
            "Offline storage",
            "Almacenamiento sin conexión",
        ),
        &format!(
            "{} · {}",
            offline_path.display(),
            match initial.language {
                AppLanguage::Portuguese => format!(
                    "{partial_count} arquivos incompletos ({})",
                    format_storage_size(partial_bytes)
                ),
                AppLanguage::English => format!(
                    "{partial_count} incomplete files ({})",
                    format_storage_size(partial_bytes)
                ),
                AppLanguage::Spanish => format!(
                    "{partial_count} archivos incompletos ({})",
                    format_storage_size(partial_bytes)
                ),
            }
        ),
        &storage_summary,
    ));

    let open_offline_folder =
        gtk::Button::with_label(group_text("Abrir pasta", "Open folder", "Abrir carpeta"));
    apply_material_button(
        &open_offline_folder,
        MaterialButtonSpec::new(MaterialButtonVariant::Outlined, MaterialButtonSize::Compact),
    );
    youtube_rows.append(&button_row(
        group_text(
            "Local dos arquivos",
            "File location",
            "Ubicación de archivos",
        ),
        group_text(
            "Abra a pasta em que o Nocky mantém áudios e downloads incompletos.",
            "Open the folder where Nocky stores audio and incomplete downloads.",
            "Abre la carpeta donde Nocky guarda audio y descargas incompletas.",
        ),
        &open_offline_folder,
    ));

    let clean_partials = gtk::Button::with_label(group_text(
        "Limpar incompletos",
        "Clean incomplete",
        "Limpiar incompletos",
    ));
    apply_material_button(
        &clean_partials,
        MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Compact,
        ),
    );
    clean_partials.set_sensitive(partial_count > 0);
    youtube_rows.append(&button_row(
        group_text(
            "Downloads incompletos",
            "Incomplete downloads",
            "Descargas incompletas",
        ),
        group_text(
            "Remove somente arquivos temporários .part. As músicas concluídas são preservadas.",
            "Remove only temporary .part files. Completed music is preserved.",
            "Elimina solo archivos temporales .part. La música completada se conserva.",
        ),
        &clean_partials,
    ));

    let clear_offline = gtk::Button::with_label(group_text(
        "Remover downloads",
        "Remove downloads",
        "Eliminar descargas",
    ));
    apply_material_button(
        &clear_offline,
        MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Compact,
        )
        .with_semantic(MaterialButtonSemantic::Destructive),
    );
    clear_offline.set_sensitive(offline_count > 0 || partial_count > 0);
    youtube_rows.append(&button_row(
        group_text(
            "Apagar arquivos offline",
            "Delete offline files",
            "Eliminar archivos sin conexión",
        ),
        group_text(
            "Remove todos os áudios baixados deste dispositivo. A biblioteca do YouTube Music não é alterada.",
            "Remove all downloaded audio from this device. Your YouTube Music library is not changed.",
            "Elimina todo el audio descargado de este dispositivo. Tu biblioteca de YouTube Music no cambia.",
        ),
        &clear_offline,
    ));

    let diagnostics_summary = gtk::Label::new(None);
    diagnostics_summary.set_xalign(0.0);
    diagnostics_summary.set_wrap(true);
    diagnostics_summary.add_css_class("settings-row-subtitle");

    let diagnostics_icon = gtk::Image::from_icon_name("emblem-system-symbolic");
    diagnostics_icon.set_pixel_size(18);

    let diagnostics_status = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    diagnostics_status.set_valign(gtk::Align::Center);
    diagnostics_status.append(&diagnostics_icon);
    diagnostics_status.append(&diagnostics_summary);

    let diagnostics_toggle = gtk::Button::with_label(group_text(
        "Ver diagnóstico",
        "View diagnostics",
        "Ver diagnóstico",
    ));
    apply_material_button(
        &diagnostics_toggle,
        MaterialButtonSpec::new(MaterialButtonVariant::Text, MaterialButtonSize::Compact),
    );
    set_material_button_selected(&diagnostics_toggle, false);

    youtube_rows.append(&row_with_control(
        group_text(
            "Diagnóstico e solução de problemas",
            "Diagnostics and troubleshooting",
            "Diagnóstico y solución de problemas",
        ),
        group_text(
            "Verificações silenciosas do runtime, da conta e do cache. Nada é exibido no player.",
            "Quiet runtime, account and cache checks. Nothing is shown in the player.",
            "Comprobaciones silenciosas del entorno, la cuenta y la caché. No se muestra nada en el reproductor.",
        ),
        &diagnostics_toggle,
    ));

    let diagnostics_details = gtk::Box::new(gtk::Orientation::Vertical, 8);
    diagnostics_details.add_css_class("settings-group-rows");

    let diagnostics_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    diagnostics_actions.set_halign(gtk::Align::End);

    let diagnostics_refresh = gtk::Button::with_label(group_text(
        "Executar novamente",
        "Run again",
        "Ejecutar de nuevo",
    ));
    apply_material_button(
        &diagnostics_refresh,
        MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Compact,
        ),
    );

    let diagnostics_copy = gtk::Button::with_label(group_text(
        "Copiar relatório",
        "Copy report",
        "Copiar informe",
    ));
    apply_material_button(
        &diagnostics_copy,
        MaterialButtonSpec::new(MaterialButtonVariant::Filled, MaterialButtonSize::Compact),
    );

    diagnostics_actions.append(&diagnostics_refresh);
    diagnostics_actions.append(&diagnostics_copy);

    let diagnostics_panel = gtk::Box::new(gtk::Orientation::Vertical, 10);
    diagnostics_panel.append(&diagnostics_status);
    diagnostics_panel.append(&diagnostics_details);
    diagnostics_panel.append(&diagnostics_actions);

    let diagnostics_revealer = gtk::Revealer::new();
    diagnostics_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    diagnostics_revealer.set_transition_duration(180);
    diagnostics_revealer.set_child(Some(&diagnostics_panel));
    diagnostics_revealer.set_reveal_child(false);
    youtube_rows.append(&diagnostics_revealer);

    update_diagnostics_view(
        &diagnostics_icon,
        &diagnostics_summary,
        &diagnostics_details,
        initial.language,
    );

    let about_button = gtk::Button::with_label(group_text(
        "Ver informações",
        "View details",
        "Ver información",
    ));
    about_button.set_action_name(Some("app.about"));
    apply_material_button(
        &about_button,
        MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Compact,
        ),
    );
    about_button.add_css_class("settings-about-action");

    let about_subtitle = format!("v{} · GPL-3.0", env!("CARGO_PKG_VERSION"));
    about_rows.append(&button_row("Nocky", &about_subtitle, &about_button));

    let shortcuts_button =
        gtk::Button::with_label(group_text("Ver atalhos", "View shortcuts", "Ver atajos"));
    shortcuts_button.set_action_name(Some("app.shortcuts"));
    apply_material_button(
        &shortcuts_button,
        MaterialButtonSpec::new(MaterialButtonVariant::Outlined, MaterialButtonSize::Compact),
    );
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
            let theme = match dropdown.selected() {
                1 => VisualTheme::MaterialExpressive,
                2 => VisualTheme::FrostedGlass,
                _ => VisualTheme::Noctalia,
            };
            let expressive = theme.is_expressive();
            expressive_transport_row.set_visible(expressive);
            expressive_home_cards_row.set_visible(expressive);
            emit(SettingsEvent::VisualTheme(theme));
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
        clear_history.connect_clicked(move |_| emit(SettingsEvent::ClearListeningHistory));
    }

    {
        let emit = emit.clone();
        youtube_button.connect_clicked(move |_| emit(SettingsEvent::ManageYouTube));
    }

    {
        let emit = emit.clone();
        open_offline_folder.connect_clicked(move |_| emit(SettingsEvent::OpenOfflineFolder));
    }

    {
        let emit = emit.clone();
        clean_partials.connect_clicked(move |_| emit(SettingsEvent::CleanOfflinePartials));
    }

    {
        let emit = emit.clone();
        let awaiting_confirmation = Rc::new(Cell::new(false));
        let language = initial.language;
        clear_offline.connect_clicked(move |button| {
            if awaiting_confirmation.replace(true) {
                awaiting_confirmation.set(false);
                emit(SettingsEvent::ClearOfflineDownloads);
                return;
            }

            button.set_label(match language {
                AppLanguage::Portuguese => "Clique novamente para apagar",
                AppLanguage::English => "Click again to delete",
                AppLanguage::Spanish => "Haz clic de nuevo para eliminar",
            });

            let weak_button = button.downgrade();
            let awaiting_confirmation = awaiting_confirmation.clone();
            glib::timeout_add_local_once(Duration::from_secs(4), move || {
                awaiting_confirmation.set(false);
                if let Some(button) = weak_button.upgrade() {
                    button.set_label(match language {
                        AppLanguage::Portuguese => "Remover downloads",
                        AppLanguage::English => "Remove downloads",
                        AppLanguage::Spanish => "Eliminar descargas",
                    });
                }
            });
        });
    }

    {
        let revealer = diagnostics_revealer.clone();
        let language = initial.language;
        diagnostics_toggle.connect_clicked(move |button| {
            let reveal = !revealer.reveals_child();
            revealer.set_reveal_child(reveal);
            set_material_button_selected(button, reveal);
            button.set_label(match (language, reveal) {
                (AppLanguage::Portuguese, true) => "Ocultar diagnóstico",
                (AppLanguage::English, true) => "Hide diagnostics",
                (AppLanguage::Spanish, true) => "Ocultar diagnóstico",
                (AppLanguage::Portuguese, false) => "Ver diagnóstico",
                (AppLanguage::English, false) => "View diagnostics",
                (AppLanguage::Spanish, false) => "Ver diagnóstico",
            });
        });
    }

    {
        let icon = diagnostics_icon.clone();
        let summary = diagnostics_summary.clone();
        let details = diagnostics_details.clone();
        let button = diagnostics_refresh.clone();
        let language = initial.language;
        diagnostics_refresh.connect_clicked(move |_| {
            button.set_sensitive(false);
            set_material_button_loading(&button, true);
            youtube_diagnostics::refresh_now();

            let icon = icon.clone();
            let summary = summary.clone();
            let details = details.clone();
            let button = button.clone();
            glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                update_diagnostics_view(&icon, &summary, &details, language);
                button.set_sensitive(true);
                set_material_button_loading(&button, false);
            });
        });
    }

    diagnostics_copy.connect_clicked(move |_| {
        if let Some(display) = gtk::gdk::Display::default() {
            display
                .clipboard()
                .set_text(&youtube_diagnostics::sanitized_report());
        }
    });

    for (switch, setting) in [
        (&visualizer, ToggleSetting::Visualizer),
        (&lyrics, ToggleSetting::Lyrics),
        (
            &personalized_history,
            ToggleSetting::PersonalizedHomeHistory,
        ),
        (&collect_history, ToggleSetting::CollectListeningHistory),
        (&auto_lyrics, ToggleSetting::AutoLyrics),
        (&resume_playback, ToggleSetting::ResumePlaybackOnStartup),
        (&youtube_sync, ToggleSetting::YouTubeSync),
        (
            &offline_collection_auto_sync,
            ToggleSetting::OfflineCollectionAutoSync,
        ),
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
                ToggleSetting::CollectListeningHistory => {
                    SettingsEvent::CollectListeningHistory(active)
                }
                ToggleSetting::AutoLyrics => SettingsEvent::AutoDownloadLyrics(active),
                ToggleSetting::ResumePlaybackOnStartup => {
                    SettingsEvent::ResumePlaybackOnStartup(active)
                }
                ToggleSetting::YouTubeSync => SettingsEvent::YouTubeAutoSync(active),
                ToggleSetting::OfflineCollectionAutoSync => {
                    SettingsEvent::OfflineCollectionAutoSync(active)
                }
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
    CollectListeningHistory,
    AutoLyrics,
    ResumePlaybackOnStartup,
    YouTubeSync,
    OfflineCollectionAutoSync,
    ExpressiveTransport,
    ExpressiveHomeCards,
    Noctalia,
}

fn format_storage_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;

    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn format_offline_track_count(language: AppLanguage, count: usize) -> String {
    match language {
        AppLanguage::Portuguese => {
            format!("{count} {}", if count == 1 { "faixa" } else { "faixas" })
        }
        AppLanguage::English => {
            format!("{count} {}", if count == 1 { "track" } else { "tracks" })
        }
        AppLanguage::Spanish => {
            format!(
                "{count} {}",
                if count == 1 { "canción" } else { "canciones" }
            )
        }
    }
}

fn settings_tab_page() -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    page.set_hexpand(true);
    page.set_vexpand(true);
    page.add_css_class("settings-tab-page");
    page
}

fn update_diagnostics_view(
    icon: &gtk::Image,
    summary: &gtk::Label,
    details: &gtk::Box,
    language: AppLanguage,
) {
    let snapshot = youtube_diagnostics::snapshot();
    let overall = snapshot.overall_state();

    icon.set_icon_name(Some(match overall {
        DiagnosticState::Ok => "emblem-ok-symbolic",
        DiagnosticState::Warning => "dialog-warning-symbolic",
        DiagnosticState::Error => "dialog-error-symbolic",
        DiagnosticState::Unknown => "emblem-system-symbolic",
    }));

    summary.set_text(match (language, overall) {
        (AppLanguage::Portuguese, DiagnosticState::Ok) => {
            "O ambiente do YouTube Music está funcionando normalmente."
        }
        (AppLanguage::English, DiagnosticState::Ok) => {
            "The YouTube Music environment is working normally."
        }
        (AppLanguage::Spanish, DiagnosticState::Ok) => {
            "El entorno de YouTube Music funciona normalmente."
        }
        (AppLanguage::Portuguese, DiagnosticState::Warning) => {
            "Há um aviso não crítico. Abra os detalhes para verificar."
        }
        (AppLanguage::English, DiagnosticState::Warning) => {
            "There is a non-critical warning. Open the details to review it."
        }
        (AppLanguage::Spanish, DiagnosticState::Warning) => {
            "Hay una advertencia no crítica. Abre los detalles para revisarla."
        }
        (AppLanguage::Portuguese, DiagnosticState::Error) => {
            "Foi detectado um problema no ambiente do YouTube Music."
        }
        (AppLanguage::English, DiagnosticState::Error) => {
            "A problem was detected in the YouTube Music environment."
        }
        (AppLanguage::Spanish, DiagnosticState::Error) => {
            "Se detectó un problema en el entorno de YouTube Music."
        }
        (AppLanguage::Portuguese, DiagnosticState::Unknown) => {
            "As verificações ainda não foram concluídas."
        }
        (AppLanguage::English, DiagnosticState::Unknown) => "The checks have not finished yet.",
        (AppLanguage::Spanish, DiagnosticState::Unknown) => {
            "Las comprobaciones aún no han terminado."
        }
    });

    while let Some(child) = details.first_child() {
        details.remove(&child);
    }

    for (title, check) in [
        ("Helper", &snapshot.helper),
        ("Python", &snapshot.python_runtime),
        ("ytmusicapi", &snapshot.ytmusicapi),
        ("yt-dlp", &snapshot.yt_dlp),
        ("Deno", &snapshot.deno),
        (
            match language {
                AppLanguage::Portuguese => "Conta",
                AppLanguage::English => "Account",
                AppLanguage::Spanish => "Cuenta",
            },
            &snapshot.account,
        ),
        (
            match language {
                AppLanguage::Portuguese => "Cache",
                AppLanguage::English => "Cache",
                AppLanguage::Spanish => "Caché",
            },
            &snapshot.cache,
        ),
    ] {
        details.append(&diagnostic_check_row(title, check));
    }
}

fn diagnostic_check_row(title: &str, check: &DiagnosticCheck) -> gtk::Box {
    let icon = gtk::Image::from_icon_name(match check.state {
        DiagnosticState::Ok => "emblem-ok-symbolic",
        DiagnosticState::Warning => "dialog-warning-symbolic",
        DiagnosticState::Error => "dialog-error-symbolic",
        DiagnosticState::Unknown => "emblem-system-symbolic",
    });
    icon.set_pixel_size(16);

    let status = if check.detail.trim().is_empty() {
        check.summary.clone()
    } else {
        format!("{} · {}", check.summary, check.detail)
    };

    let row = row_with_control(title, &status, &icon);
    row.add_css_class(match check.state {
        DiagnosticState::Ok => "diagnostic-state-ok",
        DiagnosticState::Warning => "diagnostic-state-warning",
        DiagnosticState::Error => "diagnostic-state-error",
        DiagnosticState::Unknown => "diagnostic-state-unknown",
    });
    row
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
    apply_material_card(&group, MaterialCardSpec::new(MaterialCardVariant::Filled));
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
    if !button.has_css_class("material-button") {
        button.add_css_class("settings-row-action");
    }
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
    apply_material_card(&row, MaterialCardSpec::new(MaterialCardVariant::Outlined));
    row.append(&text);
    row.append(control);
    row
}
