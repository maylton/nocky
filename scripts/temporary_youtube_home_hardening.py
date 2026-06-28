#!/usr/bin/env python3
from pathlib import Path


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s), found {count}: {old[:140]!r}"
        )
    file.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "src/app/controller/mod.rs",
    '''    pub(crate) youtube_search_request_id: Cell<u64>,
    pub(crate) youtube_home_request_id: Cell<u64>,
    pub(crate) youtube_recovery_in_progress: Cell<bool>,
''',
    '''    pub(crate) youtube_search_request_id: Cell<u64>,
    pub(crate) youtube_home_request_id: Cell<u64>,
    pub(crate) youtube_home_loading: Cell<bool>,
    pub(crate) youtube_home_previous_params: RefCell<String>,
    pub(crate) youtube_recovery_in_progress: Cell<bool>,
''',
)

replace(
    "src/app/controller/construction.rs",
    '''                youtube_search_request_id: Cell::new(0),
                youtube_home_request_id: Cell::new(0),
                youtube_recovery_in_progress: Cell::new(false),
''',
    '''                youtube_search_request_id: Cell::new(0),
                youtube_home_request_id: Cell::new(0),
                youtube_home_loading: Cell::new(false),
                youtube_home_previous_params: RefCell::new(String::new()),
                youtube_recovery_in_progress: Cell::new(false),
''',
)

replace(
    "src/app/controller/navigation.rs",
    '''                offline: &self.offline_store.borrow(),
                youtube_home: &youtube_home,
            },
''',
    '''                offline: &self.offline_store.borrow(),
                youtube_home: &youtube_home,
                youtube_home_loading: self.youtube_home_loading.get(),
            },
''',
    expected=2,
)

replace(
    "src/browser.rs",
    '''pub struct BrowserRenderContext<'a> {
    pub history: &'a ListeningHistory,
    pub playback: &'a BrowserPlaybackState,
    pub offline: &'a OfflineStore,
    pub youtube_home: &'a YouTubeHomePage,
}
''',
    '''pub struct BrowserRenderContext<'a> {
    pub history: &'a ListeningHistory,
    pub playback: &'a BrowserPlaybackState,
    pub offline: &'a OfflineStore,
    pub youtube_home: &'a YouTubeHomePage,
    pub youtube_home_loading: bool,
}
''',
)

replace(
    "src/browser.rs",
    '''                self.rebuild_home(
                    tracks,
                    config,
                    youtube,
                    context.youtube_home,
                    context.history,
                    context.playback,
                );
''',
    '''                self.rebuild_home(
                    tracks,
                    config,
                    youtube,
                    context.youtube_home,
                    context.youtube_home_loading,
                    context.history,
                    context.playback,
                );
''',
)

replace(
    "src/browser.rs",
    '''    fn rebuild_home(
        &self,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        youtube_home_page: &YouTubeHomePage,
        history: &ListeningHistory,
        playback: &BrowserPlaybackState,
    ) {
''',
    '''    fn rebuild_home(
        &self,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        youtube_home_page: &YouTubeHomePage,
        youtube_home_loading: bool,
        history: &ListeningHistory,
        playback: &BrowserPlaybackState,
    ) {
''',
)

replace(
    "src/browser.rs",
    '''            next_home.append(&youtube_home_chip_bar(
                youtube_home_page,
                &self.event_tx,
                language,
            ));
            for section in &youtube_home_page.sections {
''',
    '''            next_home.append(&youtube_home_chip_bar(
                youtube_home_page,
                &self.event_tx,
                language,
            ));
            if youtube_home_loading {
                next_home.append(&youtube_home_loading_banner(youtube_home_page, language));
            }
            for section in &youtube_home_page.sections {
''',
)

replace(
    "src/browser.rs",
    '''    rail.set_margin_start(2);
    rail.set_margin_end(2);
''',
    '''    rail.set_margin_start(2);
    rail.set_margin_end(2);
    rail.set_margin_bottom(10);
''',
)

replace(
    "src/browser.rs",
    '''    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroll.set_overlay_scrolling(true);
    scroll.set_child(Some(&rail));
    scroll.add_css_class("home-carousel-scroll");

    section.append(&scroll);
    section
}

fn youtube_feed_section_cards(
''',
    '''    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroll.set_overlay_scrolling(false);
    scroll.set_min_content_height(52);
    scroll.set_propagate_natural_height(true);
    scroll.set_child(Some(&rail));
    scroll.add_css_class("home-carousel-scroll");

    section.append(&scroll);
    section
}

fn youtube_home_loading_banner(
    page: &YouTubeHomePage,
    language: AppLanguage,
) -> gtk::Box {
    let selected_title = page
        .chips
        .iter()
        .find(|chip| chip.params == page.selected_chip_params)
        .map(|chip| chip.title.as_str());
    let message = match (language, selected_title) {
        (AppLanguage::Portuguese, Some(title)) => format!("Carregando {title}…"),
        (AppLanguage::English, Some(title)) => format!("Loading {title}…"),
        (AppLanguage::Spanish, Some(title)) => format!("Cargando {title}…"),
        (AppLanguage::Portuguese, None) => "Atualizando recomendações…".to_string(),
        (AppLanguage::English, None) => "Refreshing recommendations…".to_string(),
        (AppLanguage::Spanish, None) => "Actualizando recomendaciones…".to_string(),
    };

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.set_halign(gtk::Align::Start);
    row.set_margin_start(2);
    row.add_css_class("youtube-home-loading-row");

    let indicator = ExpressiveLoadingIndicator::with_size(18);
    row.append(indicator.widget());

    let label = gtk::Label::new(Some(&message));
    label.set_xalign(0.0);
    label.add_css_class("dim-label");
    row.append(&label);
    row
}

fn youtube_feed_section_cards(
''',
)

replace(
    "src/app/controller/youtube.rs",
    '''    pub(crate) fn load_youtube_home_page(&self, continuation: String, params: String) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_page
                .show_error("YouTube Music runtime is missing. Reinstall with --install-youtube.");
            return;
        };
        let append = !continuation.is_empty();
        let filtered = !params.is_empty();
        let request_id = self.youtube_home_request_id.get().wrapping_add(1);
        self.youtube_home_request_id.set(request_id);
        self.youtube_page.set_loading(
            true,
            if append {
                "Carregando mais recomendações..."
            } else if filtered {
                "Carregando seleção do YouTube Music..."
            } else {
                "Carregando seu feed do YouTube Music..."
            },
        );
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge
                .home_page(
                    (!continuation.is_empty()).then_some(continuation.as_str()),
                    (!params.is_empty()).then_some(params.as_str()),
                )
                .map(|mut page| {
                    cache_home_page_covers(&mut page);
                    page
                });
            let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {
                request_id,
                title: "Para você".to_string(),
                home: true,
                append,
                result,
            });
        });
    }
''',
    '''    pub(crate) fn load_youtube_home_page(&self, continuation: String, params: String) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_page
                .show_error("YouTube Music runtime is missing. Reinstall with --install-youtube.");
            return;
        };
        let append = !continuation.is_empty();
        let filtered = !params.is_empty();
        if !append {
            let current = self.youtube_home_page.borrow();
            if !current.sections.is_empty()
                && current.selected_chip_params == params
                && !self.youtube_home_loading.get()
            {
                return;
            }
        }

        let request_id = self.youtube_home_request_id.get().wrapping_add(1);
        self.youtube_home_request_id.set(request_id);
        if !append {
            let previous = self
                .youtube_home_page
                .borrow()
                .selected_chip_params
                .clone();
            self.youtube_home_previous_params.replace(previous);
            self.youtube_home_page
                .borrow_mut()
                .selected_chip_params = params.clone();
        }
        self.youtube_home_loading.set(true);
        let youtube_active =
            self.config.borrow().startup_source == Some(StartupSource::YouTube);
        if youtube_active {
            self.refresh_browser();
        }

        self.youtube_page.set_loading(
            true,
            if append {
                "Carregando mais recomendações..."
            } else if filtered {
                "Carregando seleção do YouTube Music..."
            } else {
                "Carregando seu feed do YouTube Music..."
            },
        );
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge
                .home_page(
                    (!continuation.is_empty()).then_some(continuation.as_str()),
                    (!params.is_empty()).then_some(params.as_str()),
                )
                .map(|mut page| {
                    cache_home_page_covers(&mut page);
                    page
                });
            let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {
                request_id,
                title: "Para você".to_string(),
                home: true,
                append,
                result,
            });
        });
    }
''',
)

replace(
    "src/background.rs",
    '''pub(crate) fn youtube_home_response_is_current(
    home: bool,
    request_id: u64,
    current_request_id: u64,
) -> bool {
    !home || request_id == current_request_id
}
''',
    '''pub(crate) fn youtube_home_response_is_current(
    home: bool,
    request_id: u64,
    current_request_id: u64,
) -> bool {
    !home || request_id == current_request_id
}

pub(crate) fn youtube_home_sections_changed(
    current: &YouTubeHomePage,
    incoming: &YouTubeHomePage,
) -> bool {
    current.sections != incoming.sections
}
''',
)

replace(
    "src/background.rs",
    '''mod tests {
    use super::youtube_home_response_is_current;

    #[test]
    fn rejects_stale_home_responses_but_accepts_non_home_pages() {
        assert!(youtube_home_response_is_current(true, 7, 7));
        assert!(!youtube_home_response_is_current(true, 6, 7));
        assert!(youtube_home_response_is_current(false, 0, 7));
    }
}
''',
    '''mod tests {
    use super::{youtube_home_response_is_current, youtube_home_sections_changed};
    use crate::youtube::{YouTubeHomePage, YouTubeHomeSection};

    #[test]
    fn rejects_stale_home_responses_but_accepts_non_home_pages() {
        assert!(youtube_home_response_is_current(true, 7, 7));
        assert!(!youtube_home_response_is_current(true, 6, 7));
        assert!(youtube_home_response_is_current(false, 0, 7));
    }

    #[test]
    fn detects_identical_and_changed_home_sections() {
        let current = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                ..YouTubeHomeSection::default()
            }],
            selected_chip_params: "first".to_string(),
            ..YouTubeHomePage::default()
        };
        let same_sections = YouTubeHomePage {
            sections: current.sections.clone(),
            selected_chip_params: "second".to_string(),
            ..YouTubeHomePage::default()
        };
        let changed = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "energy".to_string(),
                title: "Energy".to_string(),
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        assert!(!youtube_home_sections_changed(&current, &same_sections));
        assert!(youtube_home_sections_changed(&current, &changed));
    }
}
''',
)

replace(
    "src/app/controller/background.rs",
    '''    background::{youtube_home_response_is_current, BackgroundMessage},
    config::StartupSource,
''',
    '''    background::{
        youtube_home_response_is_current, youtube_home_sections_changed, BackgroundMessage,
    },
    config::{AppLanguage, StartupSource},
''',
)

replace(
    "src/app/controller/background.rs",
    '''                BackgroundMessage::YouTubeStructuredPage {
                    request_id,
                    title,
                    home,
                    append,
                    result,
                } if youtube_home_response_is_current(
                    home,
                    request_id,
                    self.youtube_home_request_id.get(),
                ) =>
                {
                    match result {
                        Ok(page) => {
                            if home {
                                {
                                    let mut current = self.youtube_home_page.borrow_mut();
                                    if append {
                                        current.merge_page(page.clone());
                                    } else {
                                        let mut next = page.clone();
                                        if next.chips.is_empty()
                                            && !next.selected_chip_params.is_empty()
                                            && !current.chips.is_empty()
                                        {
                                            next.chips = current.chips.clone();
                                        }
                                        *current = next;
                                    }
                                }
                                if self.config.borrow().startup_source
                                    == Some(StartupSource::YouTube)
                                {
                                    self.refresh_browser();
                                }
                            }
                            self.youtube_page.show_structured_page(&title, page, append);
                        }
                        Err(error) if append => {
                            self.youtube_page.set_loading(false, &title);
                            self.show_toast(&format!(
                                "Não foi possível carregar mais recomendações: {error}"
                            ));
                        }
                        Err(error) => self.youtube_page.show_error(&error),
                    }
                }
''',
    '''                BackgroundMessage::YouTubeStructuredPage {
                    request_id,
                    title,
                    home,
                    append,
                    result,
                } if youtube_home_response_is_current(
                    home,
                    request_id,
                    self.youtube_home_request_id.get(),
                ) =>
                {
                    if home {
                        self.youtube_home_loading.set(false);
                    }
                    match result {
                        Ok(page) => {
                            let mut unchanged_filtered_feed = false;
                            if home {
                                {
                                    let mut current = self.youtube_home_page.borrow_mut();
                                    unchanged_filtered_feed = !append
                                        && !page.selected_chip_params.is_empty()
                                        && !youtube_home_sections_changed(&current, &page);
                                    if append {
                                        current.merge_page(page.clone());
                                    } else {
                                        let mut next = page.clone();
                                        if next.chips.is_empty()
                                            && !next.selected_chip_params.is_empty()
                                            && !current.chips.is_empty()
                                        {
                                            next.chips = current.chips.clone();
                                        }
                                        *current = next;
                                    }
                                }
                                self.youtube_home_previous_params.borrow_mut().clear();
                                if self.config.borrow().startup_source
                                    == Some(StartupSource::YouTube)
                                {
                                    self.refresh_browser();
                                }
                            }
                            self.youtube_page.show_structured_page(&title, page, append);
                            if unchanged_filtered_feed {
                                let message = match self.config.borrow().language {
                                    AppLanguage::Portuguese => {
                                        "O YouTube Music retornou as mesmas recomendações para este filtro."
                                    }
                                    AppLanguage::English => {
                                        "YouTube Music returned the same recommendations for this filter."
                                    }
                                    AppLanguage::Spanish => {
                                        "YouTube Music devolvió las mismas recomendaciones para este filtro."
                                    }
                                };
                                self.show_toast(message);
                            }
                        }
                        Err(error) if append => {
                            if home
                                && self.config.borrow().startup_source
                                    == Some(StartupSource::YouTube)
                            {
                                self.refresh_browser();
                            }
                            self.youtube_page.set_loading(false, &title);
                            self.show_toast(&format!(
                                "Não foi possível carregar mais recomendações: {error}"
                            ));
                        }
                        Err(error) => {
                            if home {
                                let previous = std::mem::take(
                                    &mut *self.youtube_home_previous_params.borrow_mut(),
                                );
                                self.youtube_home_page
                                    .borrow_mut()
                                    .selected_chip_params = previous;
                                if self.config.borrow().startup_source
                                    == Some(StartupSource::YouTube)
                                {
                                    self.refresh_browser();
                                }
                            }
                            self.youtube_page.show_error(&error);
                        }
                    }
                }
''',
)
