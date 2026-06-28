use crate::config::{AppConfig, AppLanguage, YouTubeStreamSources};
use adw::prelude::*;
use gtk::glib;
use std::{cell::RefCell, fs, rc::Rc};

const KEYS: [&str; 6] = ["web_music", "web_creator", "tv", "android_vr", "web", "ios"];

fn text(
    language: AppLanguage,
    pt: &'static str,
    en: &'static str,
    es: &'static str,
) -> &'static str {
    match language {
        AppLanguage::Portuguese => pt,
        AppLanguage::English => en,
        AppLanguage::Spanish => es,
    }
}

fn label(key: &str) -> &'static str {
    match key {
        "web_music" => "WEB_REMIX",
        "web_creator" => "WEB_CREATOR",
        "tv" => "TVHTML5",
        "android_vr" => "Android VR",
        "web" => "WEB",
        "ios" => "iOS / iPadOS",
        _ => "YouTube",
    }
}

fn description(key: &str, language: AppLanguage) -> &'static str {
    match key {
        "web_music" => text(
            language,
            "Cliente principal do YouTube Music; prefere a sessão conectada.",
            "Primary YouTube Music client; prefers the connected session.",
            "Cliente principal de YouTube Music; prefiere la sesión conectada.",
        ),
        "web_creator" => text(
            language,
            "Fallback autenticado para conteúdo Premium; exige uma conta conectada.",
            "Authenticated Premium fallback; requires a connected account.",
            "Alternativa autenticada para contenido Premium; requiere una cuenta conectada.",
        ),
        "tv" => text(
            language,
            "Cliente de TV compatível com cookies e útil como fallback estável.",
            "TV client with cookie support and stable fallback behavior.",
            "Cliente de TV compatible con cookies y útil como alternativa estable.",
        ),
        "android_vr" => text(
            language,
            "Fallback nativo que não depende da sessão do navegador.",
            "Native fallback that does not rely on the browser session.",
            "Alternativa nativa que no depende de la sesión del navegador.",
        ),
        "web" => text(
            language,
            "Cliente web geral mantido como fallback de compatibilidade.",
            "General web client retained as a compatibility fallback.",
            "Cliente web general conservado como alternativa de compatibilidad.",
        ),
        _ => text(
            language,
            "Fonte opcional, desativada por padrão.",
            "Optional source, disabled by default.",
            "Fuente opcional, desactivada por defecto.",
        ),
    }
}

fn effective_label(policy: &YouTubeStreamSources) -> String {
    policy
        .effective_order()
        .iter()
        .map(|key| label(key))
        .collect::<Vec<_>>()
        .join(" → ")
}

fn last_stream_diagnostic(language: AppLanguage) -> String {
    let path = glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("stream-cache.json");
    let unavailable = || {
        text(
            language,
            "Nenhum stream foi resolvido recentemente.",
            "No stream has been resolved recently.",
            "No se ha resuelto ningún stream recientemente.",
        )
        .to_string()
    };

    let Ok(contents) = fs::read_to_string(path) else {
        return unavailable();
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return unavailable();
    };
    let Some(streams) = payload
        .get("streams")
        .and_then(serde_json::Value::as_object)
    else {
        return unavailable();
    };
    let Some((_expires_at, stream)) = streams
        .values()
        .filter_map(|stream| {
            stream
                .get("expires_at")
                .and_then(serde_json::Value::as_f64)
                .map(|expires_at| (expires_at, stream))
        })
        .max_by(|left, right| left.0.total_cmp(&right.0))
    else {
        return unavailable();
    };

    let value = |key: &str| {
        stream
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
    };
    let client = {
        let client_label = value("stream_client_label");
        if client_label.is_empty() {
            value("stream_client")
        } else {
            client_label
        }
    };
    if client.is_empty() {
        return unavailable();
    }

    let technical = [
        value("format_id"),
        value("protocol"),
        value("container"),
        value("audio_codec"),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let fallback_used = stream
        .get("fallback_used")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let fallback = match (language, fallback_used) {
        (AppLanguage::Portuguese, true) => "fallback usado",
        (AppLanguage::Portuguese, false) => "sem fallback",
        (AppLanguage::English, true) => "fallback used",
        (AppLanguage::English, false) => "no fallback",
        (AppLanguage::Spanish, true) => "alternativa utilizada",
        (AppLanguage::Spanish, false) => "sin alternativa",
    };
    let prefix = text(language, "Último stream", "Last stream", "Último stream");

    if technical.is_empty() {
        format!("{prefix}: {client} · {fallback}")
    } else {
        format!("{prefix}: {client} · {technical} · {fallback}")
    }
}

fn save_policy(policy: &YouTubeStreamSources) {
    let mut config = AppConfig::load();
    config.youtube_stream_sources = policy.clone();
    if let Err(error) = config.save() {
        eprintln!("Could not save YouTube stream-source preferences: {error}");
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

    let summary = gtk::Label::new(Some(&effective_label(policy)));
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.add_css_class("dim-label");

    let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
    copy.set_hexpand(true);
    copy.append(&title);
    copy.append(&summary);

    let button = gtk::Button::with_label(text(language, "Configurar", "Configure", "Configurar"));
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

struct DialogState {
    language: AppLanguage,
    policy: RefCell<YouTubeStreamSources>,
    rows: gtk::Box,
    summary: gtk::Label,
    entry_summary: gtk::Label,
}

impl DialogState {
    fn persist(&self) {
        save_policy(&self.policy.borrow());
        let summary = effective_label(&self.policy.borrow());
        self.summary.set_text(&summary);
        self.entry_summary.set_text(&summary);
    }

    fn rebuild(self: &Rc<Self>) {
        while let Some(child) = self.rows.first_child() {
            self.rows.remove(&child);
        }

        let current = self.policy.borrow().clone();
        let enabled_count = current.effective_order().len();
        for (index, key) in current.order.iter().enumerate() {
            if !KEYS.contains(&key.as_str()) {
                continue;
            }

            let title = gtk::Label::new(Some(label(key)));
            title.set_xalign(0.0);
            title.add_css_class("heading");

            let detail = gtk::Label::new(Some(description(key, self.language)));
            detail.set_xalign(0.0);
            detail.set_wrap(true);
            detail.add_css_class("dim-label");

            let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
            copy.set_hexpand(true);
            copy.append(&title);
            copy.append(&detail);

            let up = gtk::Button::from_icon_name("go-up-symbolic");
            up.add_css_class("flat");
            up.set_sensitive(index > 0);

            let down = gtk::Button::from_icon_name("go-down-symbolic");
            down.add_css_class("flat");
            down.set_sensitive(index + 1 < current.order.len());

            let enabled = gtk::Switch::new();
            enabled.set_active(current.is_enabled(key));
            enabled.set_valign(gtk::Align::Center);
            enabled.set_sensitive(enabled_count > 1 || !current.is_enabled(key));

            let controls = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            controls.set_valign(gtk::Align::Center);
            controls.append(&up);
            controls.append(&down);
            controls.append(&enabled);

            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.set_margin_top(10);
            row.set_margin_bottom(10);
            row.set_margin_start(12);
            row.set_margin_end(12);
            row.append(&copy);
            row.append(&controls);
            row.add_css_class("settings-row");
            self.rows.append(&row);

            let key_up = key.clone();
            let weak = Rc::downgrade(self);
            up.connect_clicked(move |_| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if state.policy.borrow_mut().move_source(&key_up, -1) {
                    state.persist();
                    state.rebuild();
                }
            });

            let key_down = key.clone();
            let weak = Rc::downgrade(self);
            down.connect_clicked(move |_| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if state.policy.borrow_mut().move_source(&key_down, 1) {
                    state.persist();
                    state.rebuild();
                }
            });

            let key_enabled = key.clone();
            let weak = Rc::downgrade(self);
            enabled.connect_active_notify(move |switch| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if state
                    .policy
                    .borrow_mut()
                    .set_enabled(&key_enabled, switch.is_active())
                {
                    state.persist();
                    state.rebuild();
                }
            });
        }
    }
}

pub(crate) fn present_dialog(
    parent: &adw::ApplicationWindow,
    initial: YouTubeStreamSources,
    language: AppLanguage,
    entry_summary: gtk::Label,
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

    let summary = gtk::Label::new(Some(&effective_label(&initial)));
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.add_css_class("dim-label");

    let diagnostic = gtk::Label::new(Some(&last_stream_diagnostic(language)));
    diagnostic.set_xalign(0.0);
    diagnostic.set_wrap(true);
    diagnostic.add_css_class("dim-label");
    diagnostic.set_tooltip_text(Some(text(
        language,
        "Mostra apenas cliente e formato. URLs, cookies e cabeçalhos nunca são exibidos.",
        "Shows only client and format. URLs, cookies, and headers are never displayed.",
        "Muestra solo cliente y formato. Nunca se muestran URLs, cookies ni cabeceras.",
    )));

    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let state = Rc::new(DialogState {
        language,
        policy: RefCell::new(initial),
        rows: rows.clone(),
        summary: summary.clone(),
        entry_summary,
    });
    state.rebuild();

    let reset = gtk::Button::with_label(text(
        language,
        "Restaurar padrões",
        "Restore defaults",
        "Restaurar valores predeterminados",
    ));
    reset.set_halign(gtk::Align::End);
    reset.add_css_class("flat");
    {
        let weak = Rc::downgrade(&state);
        reset.connect_clicked(move |_| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            state.policy.borrow_mut().reset();
            state.persist();
            state.rebuild();
        });
    }

    content.append(&summary);
    content.append(&diagnostic);
    content.append(&rows);
    content.append(&reset);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&content));
    toolbar.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar));
    dialog.present(Some(parent));
}
