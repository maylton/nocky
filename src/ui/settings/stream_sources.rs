use crate::{
    config::{AppLanguage, YouTubeStreamSources, YOUTUBE_STREAM_SOURCE_KEYS},
    dialogs::SettingsEvent,
};
use adw::prelude::*;
use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

#[derive(Clone, Copy)]
struct SourceCopy {
    key: &'static str,
    label: &'static str,
    description_pt: &'static str,
    description_en: &'static str,
    description_es: &'static str,
}

const SOURCES: [SourceCopy; 6] = [
    SourceCopy {
        key: "web_music",
        label: "WEB_REMIX",
        description_pt: "Cliente principal do YouTube Music; prefere a sessão conectada.",
        description_en: "Primary YouTube Music client; prefers the connected session.",
        description_es: "Cliente principal de YouTube Music; prefiere la sesión conectada.",
    },
    SourceCopy {
        key: "web_creator",
        label: "WEB_CREATOR",
        description_pt: "Fallback autenticado para conteúdo Premium; exige uma conta conectada.",
        description_en: "Authenticated Premium fallback; requires a connected account.",
        description_es: "Alternativa autenticada para contenido Premium; requiere una cuenta conectada.",
    },
    SourceCopy {
        key: "tv",
        label: "TVHTML5",
        description_pt: "Cliente de TV compatível com cookies e útil como fallback estável.",
        description_en: "TV client with cookie support and stable fallback behavior.",
        description_es: "Cliente de TV compatible con cookies y útil como alternativa estable.",
    },
    SourceCopy {
        key: "android_vr",
        label: "Android VR",
        description_pt: "Fallback nativo que não depende da sessão do navegador.",
        description_en: "Native fallback that does not rely on the browser session.",
        description_es: "Alternativa nativa que no depende de la sesión del navegador.",
    },
    SourceCopy {
        key: "web",
        label: "WEB",
        description_pt: "Cliente web geral mantido como fallback de compatibilidade.",
        description_en: "General web client retained as a compatibility fallback.",
        description_es: "Cliente web general conservado como alternativa de compatibilidad.",
    },
    SourceCopy {
        key: "ios",
        label: "iOS / iPadOS",
        description_pt: "Fonte opcional; desativada por padrão por ter comportamento menos previsível.",
        description_en: "Optional source; disabled by default because behavior is less predictable.",
        description_es: "Fuente opcional; desactivada por defecto por tener un comportamiento menos predecible.",
    },
];

fn text(language: AppLanguage, pt: &'static str, en: &'static str, es: &'static str) -> &'static str {
    match language {
        AppLanguage::Portuguese => pt,
        AppLanguage::English => en,
        AppLanguage::Spanish => es,
    }
}

fn source_copy(key: &str) -> SourceCopy {
    SOURCES
        .iter()
        .copied()
        .find(|source| source.key == key)
        .unwrap_or(SOURCES[0])
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn emit_policy(sender: &Sender<SettingsEvent>, policy: &YouTubeStreamSources) {
    let _ = sender.send(SettingsEvent::YouTubeStreamSources(policy.clone()));
}

fn populate_rows(
    rows: &gtk::Box,
    summary: &gtk::Label,
    policy: Rc<RefCell<YouTubeStreamSources>>,
    sender: Sender<SettingsEvent>,
    language: AppLanguage,
) {
    clear_box(rows);

    let current = policy.borrow().clone();
    let effective = current
        .effective_order()
        .iter()
        .map(|key| source_copy(key).label)
        .collect::<Vec<_>>()
        .join(" → ");
    summary.set_text(&effective);

    for (index, key) in current.order.iter().enumerate() {
        if !YOUTUBE_STREAM_SOURCE_KEYS.contains(&key.as_str()) {
            continue;
        }
        let source = source_copy(key);

        let title = gtk::Label::new(Some(source.label));
        title.set_xalign(0.0);
        title.add_css_class("heading");

        let description = gtk::Label::new(Some(match language {
            AppLanguage::Portuguese => source.description_pt,
            AppLanguage::English => source.description_en,
            AppLanguage::Spanish => source.description_es,
        }));
        description.set_xalign(0.0);
        description.set_wrap(true);
        description.add_css_class("dim-label");

        let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
        copy.set_hexpand(true);
        copy.append(&title);
        copy.append(&description);

        let priority = gtk::Label::new(Some(&(index + 1).to_string()));
        priority.set_width_chars(2);
        priority.add_css_class("dim-label");

        let move_up = gtk::Button::from_icon_name("go-up-symbolic");
        move_up.set_tooltip_text(Some(text(
            language,
            "Mover para cima",
            "Move up",
            "Mover hacia arriba",
        )));
        move_up.set_sensitive(index > 0);
        move_up.add_css_class("flat");

        let move_down = gtk::Button::from_icon_name("go-down-symbolic");
        move_down.set_tooltip_text(Some(text(
            language,
            "Mover para baixo",
            "Move down",
            "Mover hacia abajo",
        )));
        move_down.set_sensitive(index + 1 < current.order.len());
        move_down.add_css_class("flat");

        let enabled = gtk::Switch::new();
        enabled.set_active(current.is_enabled(key));
        enabled.set_valign(gtk::Align::Center);
        enabled.set_tooltip_text(Some(text(
            language,
            "Ativar ou desativar esta fonte",
            "Enable or disable this source",
            "Activar o desactivar esta fuente",
        )));
        if current.effective_order().len() <= 1 && current.is_enabled(key) {
            enabled.set_sensitive(false);
        }

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        controls.set_valign(gtk::Align::Center);
        controls.append(&priority);
        controls.append(&move_up);
        controls.append(&move_down);
        controls.append(&enabled);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(10);
        row.set_margin_bottom(10);
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.append(&copy);
        row.append(&controls);
        row.add_css_class("settings-row");
        rows.append(&row);

        {
            let rows = rows.clone();
            let summary = summary.clone();
            let policy = policy.clone();
            let sender = sender.clone();
            move_up.connect_clicked(move |_| {
                if policy.borrow_mut().move_source(source.key, -1) {
                    emit_policy(&sender, &policy.borrow());
                    populate_rows(
                        &rows,
                        &summary,
                        policy.clone(),
                        sender.clone(),
                        language,
                    );
                }
            });
        }

        {
            let rows = rows.clone();
            let summary = summary.clone();
            let policy = policy.clone();
            let sender = sender.clone();
            move_down.connect_clicked(move |_| {
                if policy.borrow_mut().move_source(source.key, 1) {
                    emit_policy(&sender, &policy.borrow());
                    populate_rows(
                        &rows,
                        &summary,
                        policy.clone(),
                        sender.clone(),
                        language,
                    );
                }
            });
        }

        {
            let rows = rows.clone();
            let summary = summary.clone();
            let policy = policy.clone();
            let sender = sender.clone();
            enabled.connect_active_notify(move |switch| {
                let requested = switch.is_active();
                if policy.borrow_mut().set_enabled(source.key, requested) {
                    emit_policy(&sender, &policy.borrow());
                    populate_rows(
                        &rows,
                        &summary,
                        policy.clone(),
                        sender.clone(),
                        language,
                    );
                } else if policy.borrow().is_enabled(source.key) != requested {
                    switch.set_active(policy.borrow().is_enabled(source.key));
                }
            });
        }
    }
}

pub(crate) fn entry_row(
    policy: &YouTubeStreamSources,
    language: AppLanguage,
) -> (gtk::Box, gtk::Button, gtk::Label) {
    let title = gtk::Label::new(Some(text(
        language,
        "YouTube Music · Fontes de stream",
        "YouTube Music · Stream sources",
        "YouTube Music · Fuentes de transmisión",
    )));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let subtitle = gtk::Label::new(Some(text(
        language,
        "Defina a prioridade dos clientes usados para resolver e recuperar a reprodução.",
        "Set the client priority used to resolve and recover playback.",
        "Define la prioridad de los clientes usados para resolver y recuperar la reproducción.",
    )));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");

    let summary = gtk::Label::new(Some(
        &policy
            .effective_order()
            .iter()
            .map(|key| source_copy(key).label)
            .collect::<Vec<_>>()
            .join(" → "),
    ));
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.add_css_class("dim-label");

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
    copy.set_hexpand(true);
    copy.append(&title);
    copy.append(&subtitle);
    copy.append(&summary);

    let button = gtk::Button::with_label(text(
        language,
        "Configurar",
        "Configure",
        "Configurar",
    ));
    button.add_css_class("suggested-action");
    button.set_valign(gtk::Align::Center);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    row.set_margin_start(24);
    row.set_margin_end(24);
    row.append(&copy);
    row.append(&button);
    row.add_css_class("settings-hero");

    (row, button, summary)
}

pub(crate) fn present_dialog(
    parent: &adw::ApplicationWindow,
    initial: YouTubeStreamSources,
    language: AppLanguage,
    sender: Sender<SettingsEvent>,
) {
    let dialog = adw::Dialog::builder()
        .title(text(
            language,
            "Fontes de stream",
            "Stream sources",
            "Fuentes de transmisión",
        ))
        .content_width(680)
        .content_height(620)
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let explanation = gtk::Label::new(Some(text(
        language,
        "A primeira fonte compatível é tentada primeiro. Fontes que exigem autenticação são ignoradas automaticamente quando a conta não está conectada.",
        "The first compatible source is tried first. Sources that require authentication are skipped automatically when the account is disconnected.",
        "La primera fuente compatible se prueba primero. Las fuentes que requieren autenticación se omiten automáticamente cuando la cuenta no está conectada.",
    )));
    explanation.set_xalign(0.0);
    explanation.set_wrap(true);
    explanation.add_css_class("dim-label");

    let summary_title = gtk::Label::new(Some(text(
        language,
        "Ordem efetiva",
        "Effective order",
        "Orden efectivo",
    )));
    summary_title.set_xalign(0.0);
    summary_title.add_css_class("heading");

    let summary = gtk::Label::new(None);
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.add_css_class("dim-label");

    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    rows.add_css_class("settings-group-rows");

    let policy = Rc::new(RefCell::new(initial));
    populate_rows(
        &rows,
        &summary,
        policy.clone(),
        sender.clone(),
        language,
    );

    let reset = gtk::Button::with_label(text(
        language,
        "Restaurar padrões",
        "Restore defaults",
        "Restaurar valores predeterminados",
    ));
    reset.set_halign(gtk::Align::End);
    reset.add_css_class("flat");
    {
        let rows = rows.clone();
        let summary = summary.clone();
        let policy = policy.clone();
        let sender = sender.clone();
        reset.connect_clicked(move |_| {
            policy.borrow_mut().reset();
            emit_policy(&sender, &policy.borrow());
            populate_rows(
                &rows,
                &summary,
                policy.clone(),
                sender.clone(),
                language,
            );
        });
    }

    content.append(&explanation);
    content.append(&summary_title);
    content.append(&summary);
    content.append(&rows);
    content.append(&reset);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&content));
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    dialog.present(Some(parent));
}
