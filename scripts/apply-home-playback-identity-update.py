#!/usr/bin/env python3
"""Keep Home playback controls accurate without rebuilding the page."""

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match in {path}, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/browser.rs",
    '''fn update_active_home_playback_controls(
    widget: &gtk::Widget,
    playing: bool,
    language: AppLanguage,
) -> usize {
    let mut updated = 0;
    if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
        if button.has_css_class("collection-card-context-action")
            && button.has_css_class("active")
            && !button.has_css_class("loading")
        {
            button.set_icon_name(if playing {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            });
            button.set_tooltip_text(Some(match (language, playing) {
                (AppLanguage::Portuguese, true) => "Pausar coleção",
                (AppLanguage::Portuguese, false) => "Continuar coleção",
                (AppLanguage::English, true) => "Pause collection",
                (AppLanguage::English, false) => "Resume collection",
                (AppLanguage::Spanish, true) => "Pausar colección",
                (AppLanguage::Spanish, false) => "Continuar colección",
            }));
            updated += 1;
        }
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        updated += update_active_home_playback_controls(&current, playing, language);
        child = current.next_sibling();
    }
    updated
}
''',
    '''fn home_playback_key(kind: &str, id: &str, title: &str) -> Option<String> {
    let kind = kind.trim().to_lowercase();
    if kind.is_empty() {
        return None;
    }
    let identity = if id.trim().is_empty() {
        title.trim().to_lowercase()
    } else {
        id.trim().to_lowercase()
    };
    (!identity.is_empty()).then(|| format!("{kind}:{identity}"))
}

fn update_home_playback_widgets(
    widget: &gtk::Widget,
    active_key: Option<&str>,
    playing: bool,
    language: AppLanguage,
) -> usize {
    let mut updated = 0;
    let widget_name = widget.widget_name();
    if let Some(key) = widget_name.strip_prefix("home-play-card:") {
        let active = active_key == Some(key);
        if active {
            widget.add_css_class("collection-card-playing");
        } else {
            widget.remove_css_class("collection-card-playing");
        }
        updated += 1;
    }

    if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
        let button_name = button.widget_name();
        if let Some(key) = button_name.strip_prefix("home-play-control:") {
            let active = active_key == Some(key);
            if active {
                button.add_css_class("active");
            } else {
                button.remove_css_class("active");
            }

            if !button.has_css_class("loading") {
                button.set_icon_name(if active && playing {
                    "media-playback-pause-symbolic"
                } else {
                    "media-playback-start-symbolic"
                });
                button.set_tooltip_text(Some(match (language, active, playing) {
                    (AppLanguage::Portuguese, true, true) => "Pausar coleção",
                    (AppLanguage::Portuguese, true, false) => "Continuar coleção",
                    (AppLanguage::Portuguese, false, _) => "Reproduzir coleção",
                    (AppLanguage::English, true, true) => "Pause collection",
                    (AppLanguage::English, true, false) => "Resume collection",
                    (AppLanguage::English, false, _) => "Play collection",
                    (AppLanguage::Spanish, true, true) => "Pausar colección",
                    (AppLanguage::Spanish, true, false) => "Continuar colección",
                    (AppLanguage::Spanish, false, _) => "Reproducir colección",
                }));
            }
            updated += 1;
        }
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        updated += update_home_playback_widgets(&current, active_key, playing, language);
        child = current.next_sibling();
    }
    updated
}
''',
    "Playback identity widget updater",
)

replace_once(
    "src/browser.rs",
    '''    #[test]
    fn dirty_or_unmounted_home_must_rebuild() {
''',
    '''    #[test]
    fn playback_key_prefers_stable_id_and_falls_back_to_title() {
        assert_eq!(
            home_playback_key("playlist", "RD123", "Ignored"),
            Some("playlist:rd123".to_string())
        );
        assert_eq!(
            home_playback_key("album", "", "My Album"),
            Some("album:my album".to_string())
        );
        assert_eq!(home_playback_key("", "id", "title"), None);
    }

    #[test]
    fn dirty_or_unmounted_home_must_rebuild() {
''',
    "Playback key tests",
)

replace_once(
    "src/browser.rs",
    '''    pub fn update_home_playback_state(&self, playing: bool, language: AppLanguage) -> usize {
        let Some(content) = self.home_stack.visible_child() else {
            return 0;
        };
        update_active_home_playback_controls(&content, playing, language)
    }
''',
    '''    pub fn update_home_playback_state(
        &self,
        playback: &BrowserPlaybackState,
        language: AppLanguage,
    ) -> usize {
        let Some(content) = self.home_stack.visible_child() else {
            return 0;
        };
        let active_key = home_playback_key(
            &playback.collection_kind,
            &playback.collection_id,
            &playback.collection_title,
        );
        update_home_playback_widgets(
            &content,
            active_key.as_deref(),
            playback.playing,
            language,
        )
    }
''',
    "Playback state method",
)

replace_once(
    "src/browser.rs",
    '''        self.route.replace(route);
        if reuse_home {
            self.root.set_visible_child_name("home");
            return;
        }
''',
    '''        self.route.replace(route);
        if reuse_home {
            self.update_home_playback_state(context.playback, config.language);
            self.root.set_visible_child_name("home");
            return;
        }
''',
    "Refresh reused Home playback state",
)

replace_once(
    "src/browser.rs",
    '''    let is_active = play_event.is_some()
        && playback.matches_collection(collection_kind, &collection_id, &collection_title);
''',
    '''    let playback_key = home_playback_key(collection_kind, &collection_id, &collection_title);
    let is_active = play_event.is_some()
        && playback.matches_collection(collection_kind, &collection_id, &collection_title);
''',
    "Compute Home playback key",
)

replace_once(
    "src/browser.rs",
    '''    card_widget.add_css_class("home-card");
    card_widget.add_css_class("expressive-collection-card");

    if let Some((offline_collection_id, _)) = &offline_collection {
''',
    '''    card_widget.add_css_class("home-card");
    card_widget.add_css_class("expressive-collection-card");
    if let Some(key) = playback_key.as_deref() {
        card_widget.set_widget_name(&format!("home-play-card:{key}"));
    }

    if let Some((offline_collection_id, _)) = &offline_collection {
''',
    "Tag Home playback card",
)

replace_once(
    "src/browser.rs",
    '''        control.add_css_class("circular");
        control.add_css_class("collection-card-context-action");

        if is_loading {
''',
    '''        control.add_css_class("circular");
        control.add_css_class("collection-card-context-action");
        if let Some(key) = playback_key.as_deref() {
            control.set_widget_name(&format!("home-play-control:{key}"));
        }

        if is_loading {
''',
    "Tag Home playback control",
)

replace_once(
    "src/browser.rs",
    '''        } else {
            let control_event = if is_active {
                BrowserEvent::TogglePlayback
            } else {
                play_event
            };
            let icon_name = if is_active && playback.playing {
''',
    '''        } else {
            let icon_name = if is_active && playback.playing {
''',
    "Remove fixed playback event",
)

replace_once(
    "src/browser.rs",
    '''            let sender = event_tx.clone();
            control.connect_clicked(move |button| {
                if inline_loading_on_click {
                    let loading = ExpressiveLoadingIndicator::new();
                    button.set_child(Some(loading.widget()));
                    button.set_sensitive(false);
                    button.add_css_class("loading");
                    button.set_tooltip_text(Some(match language {
                        AppLanguage::Portuguese => "Carregando coleção…",
                        AppLanguage::English => "Loading collection…",
                        AppLanguage::Spanish => "Cargando colección…",
                    }));
                }

                let _ = sender.send(control_event.clone());
            });
''',
    '''            let sender = event_tx.clone();
            control.connect_clicked(move |button| {
                let active = button.has_css_class("active");
                if inline_loading_on_click && !active {
                    let loading = ExpressiveLoadingIndicator::new();
                    button.set_child(Some(loading.widget()));
                    button.set_sensitive(false);
                    button.add_css_class("loading");
                    button.set_tooltip_text(Some(match language {
                        AppLanguage::Portuguese => "Carregando coleção…",
                        AppLanguage::English => "Loading collection…",
                        AppLanguage::Spanish => "Cargando colección…",
                    }));
                }

                let event = if active {
                    BrowserEvent::TogglePlayback
                } else {
                    play_event.clone()
                };
                let _ = sender.send(event);
            });
''',
    "Dynamic Home playback event",
)

replace_once(
    "src/app/controller/playback.rs",
    '''        if matches!(self.browser.route(), BrowserRoute::All) {
            self.browser.update_home_playback_state(playing, language);
        }
''',
    '''        if matches!(self.browser.route(), BrowserRoute::All) {
            let playback = self.browser_playback_state();
            self.browser
                .update_home_playback_state(&playback, language);
        }
''',
    "Pass full playback identity",
)

print("Home playback identity update applied")
