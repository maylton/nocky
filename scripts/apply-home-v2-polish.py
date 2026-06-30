#!/usr/bin/env python3
"""Apply the reviewed Home V2 and dynamic-playlist patch to the current checkout."""

from __future__ import annotations

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match in {path}, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# #78: generated radio/mix browse IDs may resolve to a different canonical ID.
replace_once(
    "helpers/nocky_youtube_playlist_create.py",
    '''def _read_metadata(client: Any, playlist_id: str, limit: int) -> dict[str, Any]:
    reader = getattr(client, "get_playlist", None)
    if not callable(reader):
        raise RuntimeError("The installed YouTube Music runtime cannot inspect playlists")

    raw_result = reader(playlist_id, limit=limit)
    if not isinstance(raw_result, dict):
        raise RuntimeError("YouTube Music returned an invalid playlist response")

    result = normalize_playlist_detail(raw_result)
    if result.get("playlist_id") != playlist_id:
        raise RuntimeError("YouTube Music returned mismatched playlist metadata")
    return result
''',
    '''def _is_generated_playlist_id(playlist_id: str) -> bool:
    """Return whether the ID represents a generated radio/mix route.

    YouTube Music may canonicalize these routes on every request. They are useful
    for read-only playback, but they must never inherit editability from an alias.
    """

    return playlist_id.startswith("RD")


def _read_metadata(
    client: Any,
    playlist_id: str,
    limit: int,
    *,
    allow_generated_alias: bool = False,
) -> dict[str, Any]:
    reader = getattr(client, "get_playlist", None)
    if not callable(reader):
        raise RuntimeError("The installed YouTube Music runtime cannot inspect playlists")

    raw_result = reader(playlist_id, limit=limit)
    if not isinstance(raw_result, dict):
        raise RuntimeError("YouTube Music returned an invalid playlist response")

    result = normalize_playlist_detail(raw_result)
    returned_id = str(result.get("playlist_id") or "").strip()
    generated = _is_generated_playlist_id(playlist_id)
    if returned_id != playlist_id and not (allow_generated_alias and generated):
        raise RuntimeError("YouTube Music returned mismatched playlist metadata")

    if generated:
        # Keep the requested route identity for native caching, but never expose a
        # generated/canonical alias as owned or editable.
        result["playlist_id"] = playlist_id
        result["owned"] = False
        result["editable"] = False
    return result
''',
    "generated playlist metadata policy",
)
replace_once(
    "helpers/nocky_youtube_playlist_create.py",
    '''    client = _authenticated_client()
    return _read_metadata(client, playlist_id, safe_limit)
''',
    '''    client = _authenticated_client()
    return _read_metadata(
        client,
        playlist_id,
        safe_limit,
        allow_generated_alias=True,
    )
''',
    "read-only generated alias opt-in",
)

# #79: support nested renderer thumbnail contracts instead of list-only shapes.
replace_once(
    "helpers/nocky_youtube_feed.py",
    '''def _best_thumbnail(value: Any) -> str:
    if isinstance(value, dict):
        value = value.get("thumbnails") or value.get("thumbnail") or []
    candidates = [item for item in (value or []) if isinstance(item, dict)]
    if not candidates:
        return ""
    candidate = max(
        candidates,
        key=lambda item: int(item.get("width") or 0) * int(item.get("height") or 0),
    )
    return _upgrade_thumbnail_url(_text(candidate.get("url")))
''',
    '''def _thumbnail_candidates(value: Any) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    visited: set[int] = set()

    def walk(node: Any) -> None:
        if isinstance(node, dict):
            identity = id(node)
            if identity in visited:
                return
            visited.add(identity)
            url = _text(node.get("url"))
            if url:
                candidates.append(node)
            for child in node.values():
                if isinstance(child, (dict, list, tuple)):
                    walk(child)
        elif isinstance(node, (list, tuple)):
            identity = id(node)
            if identity in visited:
                return
            visited.add(identity)
            for child in node:
                walk(child)

    walk(value)
    return candidates


def _thumbnail_area(item: dict[str, Any]) -> int:
    try:
        width = int(item.get("width") or 0)
        height = int(item.get("height") or 0)
    except (TypeError, ValueError):
        return 0
    return max(0, width) * max(0, height)


def _best_thumbnail(value: Any) -> str:
    candidates = _thumbnail_candidates(value)
    if not candidates:
        return ""
    candidate = max(candidates, key=_thumbnail_area)
    return _upgrade_thumbnail_url(_text(candidate.get("url")))
''',
    "recursive feed thumbnail extraction",
)
replace_once(
    "helpers/nocky_youtube_feed.py",
    '''        "thumbnail_url": _best_thumbnail(
            result.get("thumbnails") or result.get("thumbnail") or []
        ),
''',
    '''        "thumbnail_url": _best_thumbnail(result),
''',
    "item thumbnail source",
)
replace_once(
    "helpers/nocky_youtube_feed.py",
    '''                "thumbnail_url": _best_thumbnail(
                    row.get("thumbnails") or row.get("thumbnail") or []
                ),
''',
    '''                "thumbnail_url": _best_thumbnail(row),
''',
    "section thumbnail source",
)
replace_once(
    "helpers/nocky_youtube.py",
    '''def _best_thumbnail(thumbnails: Any) -> str:
    candidates = [item for item in (thumbnails or []) if isinstance(item, dict)]
    if not candidates:
        return ""
    candidate = max(candidates, key=lambda item: int(item.get("width") or 0) * int(item.get("height") or 0))
    return _upgrade_thumbnail_url(str(candidate.get("url") or ""))


def _thumbnails(result: dict[str, Any]) -> Any:
    return result.get("thumbnails") or result.get("thumbnail") or []
''',
    '''def _thumbnail_candidates(value: Any) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    visited: set[int] = set()

    def walk(node: Any) -> None:
        if isinstance(node, dict):
            identity = id(node)
            if identity in visited:
                return
            visited.add(identity)
            url = str(node.get("url") or "").strip()
            if url:
                candidates.append(node)
            for child in node.values():
                if isinstance(child, (dict, list, tuple)):
                    walk(child)
        elif isinstance(node, (list, tuple)):
            identity = id(node)
            if identity in visited:
                return
            visited.add(identity)
            for child in node:
                walk(child)

    walk(value)
    return candidates


def _thumbnail_area(item: dict[str, Any]) -> int:
    try:
        width = int(item.get("width") or 0)
        height = int(item.get("height") or 0)
    except (TypeError, ValueError):
        return 0
    return max(0, width) * max(0, height)


def _best_thumbnail(thumbnails: Any) -> str:
    candidates = _thumbnail_candidates(thumbnails)
    if not candidates:
        return ""
    candidate = max(candidates, key=_thumbnail_area)
    return _upgrade_thumbnail_url(str(candidate.get("url") or ""))


def _thumbnails(result: dict[str, Any]) -> Any:
    return result
''',
    "recursive shared thumbnail extraction",
)

# #80 and #81: add trailing chip inset and append Home sections in place.
replace_once(
    "src/browser.rs",
    '''    pub fn restore_home_scroll_positions(&self, positions: Vec<f64>) {
        if positions.is_empty() {
            return;
        }

        let home_stack = self.home_stack.clone();
        glib::idle_add_local_once(move || {
            let Some(content) = home_stack.visible_child() else {
                return;
            };

            let mut scrolled_windows = Vec::new();
            collect_scrolled_windows(&content, &mut scrolled_windows);

            for (scrolled, value) in scrolled_windows.into_iter().zip(positions) {
                let adjustment = scrolled.hadjustment();
                let maximum = (adjustment.upper() - adjustment.page_size()).max(0.0);
                adjustment.set_value(value.clamp(0.0, maximum));
            }
        });
    }

    pub fn new() -> Self {
''',
    '''    pub fn restore_home_scroll_positions(&self, positions: Vec<f64>) {
        if positions.is_empty() {
            return;
        }

        let home_stack = self.home_stack.clone();
        glib::idle_add_local_once(move || {
            let Some(content) = home_stack.visible_child() else {
                return;
            };

            let mut scrolled_windows = Vec::new();
            collect_scrolled_windows(&content, &mut scrolled_windows);

            for (scrolled, value) in scrolled_windows.into_iter().zip(positions) {
                let adjustment = scrolled.hadjustment();
                let maximum = (adjustment.upper() - adjustment.page_size()).max(0.0);
                adjustment.set_value(value.clamp(0.0, maximum));
            }
        });
    }

    pub fn append_youtube_home_page(
        &self,
        incoming: &YouTubeHomePage,
        playback: &BrowserPlaybackState,
        config: &AppConfig,
    ) -> bool {
        if !matches!(self.route(), BrowserRoute::All) {
            return false;
        }
        let Some(content) = self.home_stack.visible_child() else {
            return false;
        };
        let Ok(home) = content.downcast::<gtk::Box>() else {
            return false;
        };
        if !home.has_css_class("youtube-home-v2") {
            return false;
        }

        remove_direct_children_with_css(&home, "youtube-home-loading-row");
        remove_direct_children_with_css(&home, "youtube-home-load-more");

        let language = config.language;
        let copy = home_copy(language);
        let card_effects =
            config.visual_theme.is_expressive() && config.expressive_home_card_effects;
        for section in &incoming.sections {
            let cards = youtube_feed_section_cards(section, language);
            if cards.is_empty() {
                continue;
            }
            home.append(&home_section(
                &section.title,
                &section.label,
                copy.waiting_content,
                cards,
                playback,
                config,
                &self.event_tx,
                language,
                card_effects,
            ));
        }
        if let Some(load_more) = youtube_home_load_more_button(incoming, &self.event_tx, language) {
            home.append(&load_more);
        }
        true
    }

    pub fn reset_youtube_home_load_more(&self, language: AppLanguage) {
        let Some(content) = self.home_stack.visible_child() else {
            return;
        };
        let Ok(home) = content.downcast::<gtk::Box>() else {
            return;
        };
        let copy = home_copy(language);
        let mut child = home.first_child();
        while let Some(current) = child {
            if current.has_css_class("youtube-home-load-more") {
                if let Ok(button) = current.clone().downcast::<gtk::Button>() {
                    button.set_label(copy.youtube_load_more);
                    button.set_sensitive(true);
                }
                return;
            }
            child = current.next_sibling();
        }
    }

    pub fn new() -> Self {
''',
    "incremental Home API",
)
replace_once(
    "src/browser.rs",
    '''fn youtube_home_chip_bar(
''',
    '''fn remove_direct_children_with_css(container: &gtk::Box, class_name: &str) {
    let mut child = container.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if current.has_css_class(class_name) {
            container.remove(&current);
        }
    }
}

fn youtube_home_chip_bar(
''',
    "Home child removal helper",
)
replace_once(
    "src/browser.rs",
    '''    rail.set_margin_start(2);
    rail.set_margin_end(2);
    rail.set_margin_bottom(10);
''',
    '''    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(10);
''',
    "chip trailing inset",
)
replace_once(
    "src/browser.rs",
    '''    scroll.set_overlay_scrolling(false);
    scroll.set_min_content_height(52);
    scroll.set_propagate_natural_height(true);
    scroll.set_child(Some(&rail));
    scroll.add_css_class("home-carousel-scroll");
''',
    '''    scroll.set_overlay_scrolling(false);
    scroll.set_hexpand(true);
    scroll.set_min_content_height(52);
    scroll.set_propagate_natural_height(true);
    scroll.set_child(Some(&rail));
    scroll.add_css_class("home-carousel-scroll");
    scroll.add_css_class("youtube-chip-scroll");
''',
    "chip scroll sizing",
)
replace_once(
    "src/browser.rs",
    '''fn youtube_home_loading_banner(page: &YouTubeHomePage, language: AppLanguage) -> gtk::Box {
''',
    '''fn youtube_home_load_more_button(
    page: &YouTubeHomePage,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> Option<gtk::Button> {
    if page.continuation.trim().is_empty() {
        return None;
    }
    let copy = home_copy(language);
    let loading_label = match language {
        AppLanguage::Portuguese => "Carregando…",
        AppLanguage::English => "Loading…",
        AppLanguage::Spanish => "Cargando…",
    };
    let load_more = gtk::Button::with_label(copy.youtube_load_more);
    load_more.set_halign(gtk::Align::Center);
    load_more.add_css_class("pill");
    load_more.add_css_class("suggested-action");
    load_more.add_css_class("youtube-home-load-more");
    let continuation = page.continuation.clone();
    let params = page.selected_chip_params.clone();
    let event_tx = event_tx.clone();
    load_more.connect_clicked(move |button| {
        button.set_label(loading_label);
        button.set_sensitive(false);
        let _ = event_tx.send(BrowserEvent::LoadYouTubeHome {
            continuation: continuation.clone(),
            params: params.clone(),
        });
    });
    Some(load_more)
}

fn youtube_home_loading_banner(page: &YouTubeHomePage, language: AppLanguage) -> gtk::Box {
''',
    "load-more widget helper",
)
replace_once(
    "src/browser.rs",
    '''        if youtube_home && !youtube_home_page.sections.is_empty() {
            next_home.append(&youtube_home_chip_bar(
''',
    '''        if youtube_home && !youtube_home_page.sections.is_empty() {
            next_home.add_css_class("youtube-home-v2");
            next_home.append(&youtube_home_chip_bar(
''',
    "tag Home V2 root",
)
replace_once(
    "src/browser.rs",
    '''            if !youtube_home_page.continuation.trim().is_empty() {
                let load_more = gtk::Button::with_label(copy.youtube_load_more);
                load_more.set_halign(gtk::Align::Center);
                load_more.add_css_class("pill");
                load_more.add_css_class("suggested-action");
                let continuation = youtube_home_page.continuation.clone();
                let params = youtube_home_page.selected_chip_params.clone();
                let event_tx = self.event_tx.clone();
                load_more.connect_clicked(move |_| {
                    let _ = event_tx.send(BrowserEvent::LoadYouTubeHome {
                        continuation: continuation.clone(),
                        params: params.clone(),
                    });
                });
                next_home.append(&load_more);
            }
''',
    '''            if let Some(load_more) = youtube_home_load_more_button(
                youtube_home_page,
                &self.event_tx,
                language,
            ) {
                next_home.append(&load_more);
            }
''',
    "reuse load-more widget",
)
replace_once(
    "src/app/controller/youtube.rs",
    '''        let youtube_active = self.config.borrow().startup_source == Some(StartupSource::YouTube);
        if youtube_active {
            self.refresh_browser();
        }
''',
    '''        let youtube_active = self.config.borrow().startup_source == Some(StartupSource::YouTube);
        if youtube_active && !append {
            self.refresh_browser();
        }
''',
    "avoid append-start rebuild",
)
replace_once(
    "src/app/controller/background.rs",
    '''                        Ok(page) => {
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
''',
    '''                        Ok(page) => {
                            let mut unchanged_filtered_feed = false;
                            if home {
                                let youtube_active = self.config.borrow().startup_source
                                    == Some(StartupSource::YouTube);
                                let appended_in_place = if append && youtube_active {
                                    let playback = self.browser_playback_state();
                                    self.browser.append_youtube_home_page(
                                        &page,
                                        &playback,
                                        &self.config.borrow(),
                                    )
                                } else {
                                    false
                                };
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
                                if youtube_active && (!append || !appended_in_place) {
                                    self.refresh_browser();
                                }
                            }
                            self.youtube_page.show_structured_page(&title, page, append);
''',
    "append Home sections in place",
)
replace_once(
    "src/app/controller/background.rs",
    '''                        Err(error) if append => {
                            if home
                                && self.config.borrow().startup_source
                                    == Some(StartupSource::YouTube)
                            {
                                self.refresh_browser();
                            }
                            self.youtube_page.set_loading(false, &title);
''',
    '''                        Err(error) if append => {
                            if home
                                && self.config.borrow().startup_source
                                    == Some(StartupSource::YouTube)
                            {
                                self.browser.reset_youtube_home_load_more(
                                    self.config.borrow().language,
                                );
                            }
                            self.youtube_page.set_loading(false, &title);
''',
    "restore load-more after failure",
)

# Contract tests.
replace_once(
    "tests/test_youtube_playlist_metadata_packaged.py",
    '''    def test_mismatched_metadata_is_rejected(self) -> None:
''',
    '''    def test_generated_playlist_alias_is_read_only(self) -> None:
        client = FakeClient(
            {
                "id": "RD-canonical",
                "title": "Dynamic mix",
                "owned": True,
                "privacy": "PRIVATE",
                "tracks": [],
            }
        )
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                result = nocky_youtube_playlist_create.fetch_playlist_metadata(
                    {"playlist_id": "RDTMAK5uy_dynamic"}
                )

        self.assertEqual(result["playlist_id"], "RDTMAK5uy_dynamic")
        self.assertFalse(result["owned"])
        self.assertFalse(result["editable"])

    def test_generated_alias_is_still_rejected_for_mutation(self) -> None:
        client = FakeClient(
            {
                "id": "RD-canonical",
                "title": "Dynamic mix",
                "owned": True,
                "privacy": "PRIVATE",
                "tracks": [],
            }
        )
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test": "value"}},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                with self.assertRaisesRegex(RuntimeError, "mismatched"):
                    nocky_youtube_playlist_create.add_playlist_item(
                        {
                            "playlist_id": "RDTMAK5uy_dynamic",
                            "video_id": "abcdefghijk",
                            "owned": True,
                            "editable": True,
                        }
                    )
        self.assertEqual(client.add_calls, [])

    def test_mismatched_metadata_is_rejected(self) -> None:
''',
    "dynamic metadata tests",
)
replace_once(
    "tests/test_youtube_feed.py",
    '''    def test_deduplicates_items_without_flattening_sections(self) -> None:
''',
    '''    def test_extracts_nested_renderer_artwork(self) -> None:
        source = {
            "sections": [
                {
                    "title": "Albums for you",
                    "contents": [
                        {
                            "resultType": "album",
                            "title": "Nested artwork",
                            "browseId": "MPREnested",
                            "thumbnailRenderer": {
                                "musicThumbnailRenderer": {
                                    "thumbnail": {
                                        "thumbnails": [
                                            {
                                                "url": "https://lh3.googleusercontent.com/example=s60",
                                                "width": 60,
                                                "height": 60,
                                            },
                                            {
                                                "url": "https://lh3.googleusercontent.com/example=s240",
                                                "width": 240,
                                                "height": 240,
                                            },
                                        ]
                                    }
                                }
                            },
                        }
                    ],
                }
            ]
        }
        page = build_structured_home(source, section_limit=1)
        thumbnail = page["sections"][0]["items"][0]["thumbnail_url"]
        self.assertIn("example=s1200", thumbnail)

    def test_deduplicates_items_without_flattening_sections(self) -> None:
''',
    "nested artwork test",
)

print("Home V2 polish patch applied")
