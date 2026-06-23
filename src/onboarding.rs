use crate::config::{AppConfig, AppLanguage, BlurMode, FooterMode, StartupSource, VisualTheme};
use adw::prelude::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug)]
pub struct OnboardingChoices {
    pub startup_source: StartupSource,
    pub blur_mode: BlurMode,
    pub blur_opacity: f64,
    pub footer_mode: FooterMode,
    pub visual_theme: VisualTheme,
    pub noctalia_theme_sync: bool,
}

#[derive(Clone, Copy)]
struct Copy {
    window_title: &'static str,
    welcome_title: &'static str,
    welcome_body: &'static str,
    welcome_note: &'static str,
    source_title: &'static str,
    source_body: &'static str,
    local_title: &'static str,
    local_body: &'static str,
    youtube_title: &'static str,
    youtube_body: &'static str,
    experimental_title: &'static str,
    experimental_body: &'static str,
    appearance_title: &'static str,
    appearance_body: &'static str,
    palette_title: &'static str,
    palette_available: &'static str,
    palette_unavailable: &'static str,
    blur_title: &'static str,
    blur_custom: &'static str,
    blur_noctalia: &'static str,
    blur_off: &'static str,
    opacity_title: &'static str,
    player_title: &'static str,
    player_body: &'static str,
    progress_title: &'static str,
    progress_body: &'static str,
    footer_title: &'static str,
    footer_body: &'static str,
    footer_automatic: &'static str,
    footer_full: &'static str,
    footer_compact: &'static str,
    footer_hidden: &'static str,
    summary_title: &'static str,
    summary_body: &'static str,
    summary_source: &'static str,
    summary_blur: &'static str,
    summary_palette: &'static str,
    summary_progress: &'static str,
    summary_footer: &'static str,
    yes: &'static str,
    no: &'static str,
    back: &'static str,
    next: &'static str,
    finish: &'static str,
    step: &'static str,
}

fn copy(language: AppLanguage) -> Copy {
    match language {
        AppLanguage::Portuguese => Copy {
            window_title: "Configuração inicial do Nocky",
            welcome_title: "Boas-vindas ao Nocky",
            welcome_body: "Vamos configurar sua experiência musical antes de começar.",
            welcome_note: "Todas estas escolhas poderão ser alteradas depois nas Configurações.",
            source_title: "Como você quer ouvir música?",
            source_body: "Escolha a fonte principal mostrada na Home.",
            local_title: "Arquivos locais",
            local_body: "Use músicas armazenadas no computador. O Nocky poderá pedir a pasta da biblioteca ao concluir.",
            youtube_title: "YouTube Music",
            youtube_body: "Use a busca pública e, opcionalmente, conecte sua conta para sincronizar biblioteca e playlists.",
            experimental_title: "Integração experimental",
            experimental_body: "O acesso ao YouTube Music usa interfaces não oficiais e pode precisar de atualizações quando o serviço mudar. A conexão da conta é opcional e os dados de sessão permanecem locais.",
            appearance_title: "Aparência",
            appearance_body: "Escolha o vidro da janela e a integração visual com o desktop.",
            palette_title: "Seguir a paleta do Noctalia Shell",
            palette_available: "Noctalia Shell detectado. A paleta gerada poderá ser aplicada automaticamente.",
            palette_unavailable: "Noctalia Shell não foi detectado em execução. Esta opção permanecerá desativada.",
            blur_title: "Desfoque da janela",
            blur_custom: "Desfoque personalizado",
            blur_noctalia: "Seguir desfoque do Noctalia",
            blur_off: "Sem desfoque",
            opacity_title: "Transparência do vidro",
            player_title: "Player",
            player_body: "Escolha o tema visual e o comportamento do footer.",
            progress_title: "Tema visual",
            progress_body: "Escolha entre a integração visual do Noctalia e o estilo Material 3 Expressive.",
            footer_title: "Comportamento do footer",
            footer_body: "O modo Automático evita controles duplicados enquanto o player da Home estiver visível.",
            footer_automatic: "Automático — recomendado",
            footer_full: "Completo",
            footer_compact: "Compacto",
            footer_hidden: "Oculto",
            summary_title: "Tudo pronto",
            summary_body: "Revise as escolhas antes de abrir o Nocky.",
            summary_source: "Fonte da Home",
            summary_blur: "Desfoque",
            summary_palette: "Paleta do Noctalia",
            summary_progress: "Tema visual",
            summary_footer: "Footer",
            yes: "Ativado",
            no: "Desativado",
            back: "Voltar",
            next: "Próximo",
            finish: "Começar a usar o Nocky",
            step: "Etapa",
        },
        AppLanguage::English => Copy {
            window_title: "Nocky first-run setup",
            welcome_title: "Welcome to Nocky",
            welcome_body: "Let’s configure your music experience before getting started.",
            welcome_note: "Every choice can be changed later in Settings.",
            source_title: "How do you want to listen?",
            source_body: "Choose the primary source displayed on Home.",
            local_title: "Local files",
            local_body: "Use music stored on this computer. Nocky can ask for the library folder after setup.",
            youtube_title: "YouTube Music",
            youtube_body: "Use public search and optionally connect an account to synchronize your library and playlists.",
            experimental_title: "Experimental integration",
            experimental_body: "YouTube Music access uses unofficial interfaces and may require updates when the service changes. Account connection is optional and session data remains local.",
            appearance_title: "Appearance",
            appearance_body: "Choose the window glass and desktop visual integration.",
            palette_title: "Follow the Noctalia Shell palette",
            palette_available: "Noctalia Shell was detected. Its generated palette can be applied automatically.",
            palette_unavailable: "Noctalia Shell was not detected as running. This option will remain disabled.",
            blur_title: "Window blur",
            blur_custom: "Custom blur",
            blur_noctalia: "Follow Noctalia blur",
            blur_off: "No blur",
            opacity_title: "Glass transparency",
            player_title: "Player",
            player_body: "Choose the visual theme and footer behavior.",
            progress_title: "Visual theme",
            progress_body: "Choose between Noctalia integration and the Material 3 Expressive style.",
            footer_title: "Footer behavior",
            footer_body: "Automatic mode avoids duplicate controls while the Home player is visible.",
            footer_automatic: "Automatic — recommended",
            footer_full: "Full",
            footer_compact: "Compact",
            footer_hidden: "Hidden",
            summary_title: "Ready to go",
            summary_body: "Review your choices before opening Nocky.",
            summary_source: "Home source",
            summary_blur: "Blur",
            summary_palette: "Noctalia palette",
            summary_progress: "Visual theme",
            summary_footer: "Footer",
            yes: "Enabled",
            no: "Disabled",
            back: "Back",
            next: "Next",
            finish: "Start using Nocky",
            step: "Step",
        },
        AppLanguage::Spanish => Copy {
            window_title: "Configuración inicial de Nocky",
            welcome_title: "Bienvenido a Nocky",
            welcome_body: "Configuremos tu experiencia musical antes de comenzar.",
            welcome_note: "Todas estas opciones podrán cambiarse después en Configuración.",
            source_title: "¿Cómo quieres escuchar música?",
            source_body: "Elige la fuente principal que se mostrará en Home.",
            local_title: "Archivos locales",
            local_body: "Usa música almacenada en el ordenador. Nocky podrá solicitar la carpeta al finalizar.",
            youtube_title: "YouTube Music",
            youtube_body: "Usa la búsqueda pública y conecta una cuenta opcionalmente para sincronizar biblioteca y playlists.",
            experimental_title: "Integración experimental",
            experimental_body: "El acceso a YouTube Music usa interfaces no oficiales y puede necesitar actualizaciones cuando cambie el servicio. Conectar la cuenta es opcional y los datos de sesión permanecen locales.",
            appearance_title: "Apariencia",
            appearance_body: "Elige el cristal de la ventana y la integración visual con el escritorio.",
            palette_title: "Seguir la paleta de Noctalia Shell",
            palette_available: "Noctalia Shell fue detectado. Su paleta generada puede aplicarse automáticamente.",
            palette_unavailable: "Noctalia Shell no fue detectado en ejecución. Esta opción permanecerá desactivada.",
            blur_title: "Desenfoque de la ventana",
            blur_custom: "Desenfoque personalizado",
            blur_noctalia: "Seguir desenfoque de Noctalia",
            blur_off: "Sin desenfoque",
            opacity_title: "Transparencia del cristal",
            player_title: "Reproductor",
            player_body: "Elige el tema visual y el comportamiento del footer.",
            progress_title: "Tema visual",
            progress_body: "Elige entre la integración visual de Noctalia y el estilo Material 3 Expressive.",
            footer_title: "Comportamiento del footer",
            footer_body: "El modo Automático evita controles duplicados mientras el reproductor de Home está visible.",
            footer_automatic: "Automático — recomendado",
            footer_full: "Completo",
            footer_compact: "Compacto",
            footer_hidden: "Oculto",
            summary_title: "Todo listo",
            summary_body: "Revisa tus opciones antes de abrir Nocky.",
            summary_source: "Fuente de Home",
            summary_blur: "Desenfoque",
            summary_palette: "Paleta de Noctalia",
            summary_progress: "Tema visual",
            summary_footer: "Footer",
            yes: "Activado",
            no: "Desactivado",
            back: "Atrás",
            next: "Siguiente",
            finish: "Comenzar a usar Nocky",
            step: "Paso",
        },
    }
}

fn page_shell(title: &str, description: &str) -> (gtk::Box, gtk::Box) {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
    page.set_margin_top(26);
    page.set_margin_bottom(20);
    page.set_margin_start(30);
    page.set_margin_end(30);
    page.set_vexpand(true);

    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    heading.add_css_class("title-1");

    let body = gtk::Label::new(Some(description));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.add_css_class("dim-label");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.set_vexpand(true);

    page.append(&heading);
    page.append(&body);
    page.append(&content);
    (page, content)
}

fn option_card(title: &str, description: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("onboarding-option-card");
    row.set_margin_top(2);
    row.set_margin_bottom(2);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_hexpand(true);

    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    heading.add_css_class("heading");

    let body = gtk::Label::new(Some(description));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.add_css_class("dim-label");

    text.append(&heading);
    text.append(&body);
    row.append(&text);
    row.append(control);
    row
}

fn summary_row(title: &str, value: &gtk::Label) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.add_css_class("onboarding-summary-row");

    let title = gtk::Label::new(Some(title));
    title.set_xalign(0.0);
    title.set_hexpand(true);

    value.set_xalign(1.0);
    value.add_css_class("heading");

    row.append(&title);
    row.append(value);
    row
}

pub fn present<F>(
    parent: &adw::ApplicationWindow,
    language: AppLanguage,
    initial: &AppConfig,
    noctalia_available: bool,
    on_finish: F,
) where
    F: Fn(OnboardingChoices) + 'static,
{
    let text = copy(language);

    let dialog = adw::Dialog::builder()
        .title(text.window_title)
        .content_width(720)
        .content_height(600)
        .build();
    dialog.set_can_close(false);
    dialog.add_css_class("onboarding-dialog");

    let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shell.set_vexpand(true);
    shell.add_css_class("onboarding-shell");
    dialog.set_child(Some(&shell));

    let progress_header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    progress_header.set_margin_top(16);
    progress_header.set_margin_start(30);
    progress_header.set_margin_end(30);

    let step_label = gtk::Label::new(None);
    step_label.set_xalign(0.0);
    step_label.set_hexpand(true);
    step_label.add_css_class("caption");

    let progress = gtk::ProgressBar::new();
    progress.set_size_request(220, -1);
    progress.add_css_class("onboarding-progress");

    progress_header.append(&step_label);
    progress_header.append(&progress);
    shell.append(&progress_header);

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    stack.set_transition_duration(220);
    shell.append(&stack);

    // Welcome
    let (welcome_page, welcome_content) = page_shell(text.welcome_title, text.welcome_body);
    let welcome_icon = gtk::Image::from_icon_name("io.github.maylton.Nocky");
    welcome_icon.set_pixel_size(128);
    welcome_icon.set_halign(gtk::Align::Center);
    welcome_icon.set_valign(gtk::Align::Center);
    welcome_icon.set_vexpand(true);
    welcome_icon.add_css_class("onboarding-app-icon");

    let welcome_note = gtk::Label::new(Some(text.welcome_note));
    welcome_note.set_wrap(true);
    welcome_note.set_justify(gtk::Justification::Center);
    welcome_note.set_halign(gtk::Align::Center);
    welcome_note.add_css_class("dim-label");

    welcome_content.append(&welcome_icon);
    welcome_content.append(&welcome_note);
    stack.add_named(&welcome_page, Some("welcome"));

    // Source
    let (source_page, source_content) = page_shell(text.source_title, text.source_body);

    let local_choice = gtk::CheckButton::with_label(text.local_title);
    local_choice.add_css_class("onboarding-radio");
    let youtube_choice = gtk::CheckButton::with_label(text.youtube_title);
    youtube_choice.set_group(Some(&local_choice));
    youtube_choice.add_css_class("onboarding-radio");

    if initial.startup_source == Some(StartupSource::YouTube) {
        youtube_choice.set_active(true);
    } else {
        local_choice.set_active(true);
    }

    source_content.append(&option_card(
        text.local_title,
        text.local_body,
        &local_choice,
    ));
    source_content.append(&option_card(
        text.youtube_title,
        text.youtube_body,
        &youtube_choice,
    ));

    let warning = gtk::Box::new(gtk::Orientation::Vertical, 5);
    warning.add_css_class("onboarding-warning");
    let warning_title = gtk::Label::new(Some(text.experimental_title));
    warning_title.set_xalign(0.0);
    warning_title.add_css_class("heading");
    let warning_body = gtk::Label::new(Some(text.experimental_body));
    warning_body.set_xalign(0.0);
    warning_body.set_wrap(true);
    warning_body.add_css_class("dim-label");
    warning.append(&warning_title);
    warning.append(&warning_body);
    warning.set_visible(youtube_choice.is_active());
    source_content.append(&warning);

    {
        let warning = warning.clone();
        youtube_choice.connect_active_notify(move |choice| {
            warning.set_visible(choice.is_active());
        });
    }

    stack.add_named(&source_page, Some("source"));

    // Appearance
    let (appearance_page, appearance_content) =
        page_shell(text.appearance_title, text.appearance_body);

    let palette_switch = gtk::Switch::new();
    palette_switch.set_valign(gtk::Align::Center);
    palette_switch.set_active(initial.noctalia_theme_sync && noctalia_available);
    palette_switch.set_sensitive(noctalia_available);

    appearance_content.append(&option_card(
        text.palette_title,
        if noctalia_available {
            text.palette_available
        } else {
            text.palette_unavailable
        },
        &palette_switch,
    ));

    let blur_items: Vec<&str> = if noctalia_available {
        vec![text.blur_custom, text.blur_noctalia, text.blur_off]
    } else {
        vec![text.blur_custom, text.blur_off]
    };
    let blur_mode = gtk::DropDown::from_strings(&blur_items);
    let initial_blur_index = if noctalia_available {
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
    };
    blur_mode.set_selected(initial_blur_index);
    appearance_content.append(&option_card(
        text.blur_title,
        if noctalia_available {
            text.palette_available
        } else {
            text.palette_unavailable
        },
        &blur_mode,
    ));

    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 45.0, 95.0, 1.0);
    opacity.set_draw_value(true);
    opacity.set_value(initial.blur_opacity.clamp(0.45, 0.95) * 100.0);
    opacity.set_value_pos(gtk::PositionType::Right);
    opacity.set_hexpand(true);

    let opacity_row = gtk::Box::new(gtk::Orientation::Vertical, 8);
    opacity_row.add_css_class("onboarding-option-card");
    let opacity_title = gtk::Label::new(Some(text.opacity_title));
    opacity_title.set_xalign(0.0);
    opacity_title.add_css_class("heading");
    opacity_row.append(&opacity_title);
    opacity_row.append(&opacity);
    opacity_row.set_visible(blur_mode.selected() == 0);
    appearance_content.append(&opacity_row);

    {
        let opacity_row = opacity_row.clone();
        blur_mode.connect_selected_notify(move |dropdown| {
            opacity_row.set_visible(dropdown.selected() == 0);
        });
    }

    stack.add_named(&appearance_page, Some("appearance"));

    // Player
    let (player_page, player_content) = page_shell(text.player_title, text.player_body);

    let visual_theme = gtk::DropDown::from_strings(&["Noctalia", "Material 3 Expressive"]);
    visual_theme.set_selected(match initial.visual_theme {
        VisualTheme::Noctalia => 0,
        VisualTheme::MaterialExpressive => 1,
    });
    player_content.append(&option_card(
        text.progress_title,
        text.progress_body,
        &visual_theme,
    ));

    let footer = gtk::DropDown::from_strings(&[
        text.footer_automatic,
        text.footer_full,
        text.footer_compact,
        text.footer_hidden,
    ]);
    footer.set_selected(match initial.footer_mode {
        FooterMode::Automatic => 0,
        FooterMode::Full => 1,
        FooterMode::Compact => 2,
        FooterMode::Hidden => 3,
    });
    player_content.append(&option_card(text.footer_title, text.footer_body, &footer));

    stack.add_named(&player_page, Some("player"));

    // Summary
    let (summary_page, summary_content) = page_shell(text.summary_title, text.summary_body);

    let summary_source = gtk::Label::new(None);
    let summary_blur = gtk::Label::new(None);
    let summary_palette = gtk::Label::new(None);
    let summary_progress = gtk::Label::new(None);
    let summary_footer = gtk::Label::new(None);

    summary_content.append(&summary_row(text.summary_source, &summary_source));
    summary_content.append(&summary_row(text.summary_blur, &summary_blur));
    summary_content.append(&summary_row(text.summary_palette, &summary_palette));
    summary_content.append(&summary_row(text.summary_progress, &summary_progress));
    summary_content.append(&summary_row(text.summary_footer, &summary_footer));

    stack.add_named(&summary_page, Some("summary"));

    let nav = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    nav.set_margin_start(30);
    nav.set_margin_end(30);
    nav.set_margin_bottom(22);

    let back = gtk::Button::with_label(text.back);
    let next = gtk::Button::with_label(text.next);
    let finish = gtk::Button::with_label(text.finish);
    next.add_css_class("suggested-action");
    finish.add_css_class("suggested-action");

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    nav.append(&back);
    nav.append(&spacer);
    nav.append(&next);
    nav.append(&finish);
    shell.append(&nav);

    let pages = Rc::new(["welcome", "source", "appearance", "player", "summary"]);
    let step = Rc::new(Cell::new(0_usize));

    let update_navigation: Rc<dyn Fn()> = {
        let pages = pages.clone();
        let step = step.clone();
        let stack = stack.clone();
        let step_label = step_label.clone();
        let progress = progress.clone();
        let back = back.clone();
        let next = next.clone();
        let finish = finish.clone();
        let local_choice = local_choice.clone();
        let blur_mode = blur_mode.clone();
        let palette_switch = palette_switch.clone();
        let visual_theme = visual_theme.clone();
        let footer = footer.clone();
        let summary_source = summary_source.clone();
        let summary_blur = summary_blur.clone();
        let summary_palette = summary_palette.clone();
        let summary_progress = summary_progress.clone();
        let summary_footer = summary_footer.clone();

        Rc::new(move || {
            let index = step.get().min(pages.len() - 1);
            stack.set_visible_child_name(pages[index]);
            step_label.set_text(&format!("{} {} / {}", text.step, index + 1, pages.len()));
            progress.set_fraction((index + 1) as f64 / pages.len() as f64);
            back.set_sensitive(index > 0);
            next.set_visible(index + 1 < pages.len());
            finish.set_visible(index + 1 == pages.len());

            if index + 1 == pages.len() {
                summary_source.set_text(if local_choice.is_active() {
                    text.local_title
                } else {
                    text.youtube_title
                });

                let blur_label = if noctalia_available {
                    match blur_mode.selected() {
                        0 => text.blur_custom,
                        1 => text.blur_noctalia,
                        _ => text.blur_off,
                    }
                } else if blur_mode.selected() == 0 {
                    text.blur_custom
                } else {
                    text.blur_off
                };
                summary_blur.set_text(blur_label);
                summary_palette.set_text(if palette_switch.is_active() {
                    text.yes
                } else {
                    text.no
                });
                summary_progress.set_text(if visual_theme.selected() == 1 {
                    "Material 3 Expressive"
                } else {
                    "Noctalia"
                });
                summary_footer.set_text(match footer.selected() {
                    1 => text.footer_full,
                    2 => text.footer_compact,
                    3 => text.footer_hidden,
                    _ => text.footer_automatic,
                });
            }
        })
    };

    {
        let update_navigation = update_navigation.clone();
        let step = step.clone();
        next.connect_clicked(move |_| {
            step.set((step.get() + 1).min(4));
            update_navigation();
        });
    }

    {
        let update_navigation = update_navigation.clone();
        let step = step.clone();
        back.connect_clicked(move |_| {
            step.set(step.get().saturating_sub(1));
            update_navigation();
        });
    }

    {
        let dialog = dialog.clone();
        finish.connect_clicked(move |_| {
            let blur = if noctalia_available {
                match blur_mode.selected() {
                    0 => BlurMode::Custom,
                    1 => BlurMode::Noctalia,
                    _ => BlurMode::Off,
                }
            } else if blur_mode.selected() == 0 {
                BlurMode::Custom
            } else {
                BlurMode::Off
            };

            let choices = OnboardingChoices {
                startup_source: if local_choice.is_active() {
                    StartupSource::Local
                } else {
                    StartupSource::YouTube
                },
                blur_mode: blur,
                blur_opacity: (opacity.value() / 100.0).clamp(0.45, 0.95),
                footer_mode: match footer.selected() {
                    1 => FooterMode::Full,
                    2 => FooterMode::Compact,
                    3 => FooterMode::Hidden,
                    _ => FooterMode::Automatic,
                },
                visual_theme: if visual_theme.selected() == 1 {
                    VisualTheme::MaterialExpressive
                } else {
                    VisualTheme::Noctalia
                },
                noctalia_theme_sync: noctalia_available && palette_switch.is_active(),
            };

            on_finish(choices);
            dialog.force_close();
        });
    }

    update_navigation();
    dialog.present(Some(parent));
}
