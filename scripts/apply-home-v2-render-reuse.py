#!/usr/bin/env python3
"""Apply Home V2 render reuse and targeted playback updates."""

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
    '''pub struct LibraryBrowser {
    root: gtk::Stack,
    home_stack: gtk::Stack,
    home_generation: Rc<Cell<u64>>,
    search_content: gtk::Box,
''',
    '''pub struct LibraryBrowser {
    root: gtk::Stack,
    home_stack: gtk::Stack,
    home_generation: Rc<Cell<u64>>,
    home_dirty: Cell<bool>,
    search_content: gtk::Box,
''',
    "Home dirty field",
)

replace_once(
    "src/browser.rs",
    '''fn collect_scrolled_windows(widget: &gtk::Widget, output: &mut Vec<gtk::ScrolledWindow>) {
    if let Ok(scrolled) = widget.clone().downcast::<gtk::ScrolledWindow>() {
        output.push(scrolled);
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        collect_scrolled_windows(&current, output);
        child = current.next_sibling();
    }
}

impl LibraryBrowser {
''',
    '''fn collect_scrolled_windows(widget: &gtk::Widget, output: &mut Vec<gtk::ScrolledWindow>) {
    if let Ok(scrolled) = widget.clone().downcast::<gtk::ScrolledWindow>() {
        output.push(scrolled);
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        collect_scrolled_windows(&current, output);
        child = current.next_sibling();
    }
}

fn should_reuse_youtube_home(
    route: &BrowserRoute,
    query: &str,
    youtube_source: bool,
    dirty: bool,
    mounted: bool,
) -> bool {
    matches!(route, BrowserRoute::All)
        && query.trim().is_empty()
        && youtube_source
        && !dirty
        && mounted
}

fn update_active_home_playback_controls(
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

#[cfg(test)]
mod home_render_reuse_tests {
    use super::*;

    #[test]
    fn reuses_a_clean_mounted_youtube_home() {
        assert!(should_reuse_youtube_home(
            &BrowserRoute::All,
            "",
            true,
            false,
            true,
        ));
    }

    #[test]
    fn dirty_or_unmounted_home_must_rebuild() {
        assert!(!should_reuse_youtube_home(
            &BrowserRoute::All,
            "",
            true,
            true,
            true,
        ));
        assert!(!should_reuse_youtube_home(
            &BrowserRoute::All,
            "",
            true,
            false,
            false,
        ));
    }

    #[test]
    fn search_local_and_non_home_routes_never_reuse_youtube_home() {
        assert!(!should_reuse_youtube_home(
            &BrowserRoute::All,
            "query",
            true,
            false,
            true,
        ));
        assert!(!should_reuse_youtube_home(
            &BrowserRoute::All,
            "",
            false,
            false,
            true,
        ));
        assert!(!should_reuse_youtube_home(
            &BrowserRoute::Albums,
            "",
            true,
            false,
            true,
        ));
    }
}

impl LibraryBrowser {
''',
    "Home reuse and targeted playback helpers",
)

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

    pub fn append_youtube_home_page(
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

    pub fn mark_home_dirty(&self) {
        self.home_dirty.set(true);
    }

    pub fn update_home_playback_state(
        &self,
        playing: bool,
        language: AppLanguage,
    ) -> usize {
        let Some(content) = self.home_stack.visible_child() else {
            return 0;
        };
        update_active_home_playback_controls(&content, playing, language)
    }

    fn has_mounted_youtube_home(&self) -> bool {
        self.home_stack
            .visible_child()
            .is_some_and(|content| content.has_css_class("youtube-home-v2"))
    }

    pub fn append_youtube_home_page(
''',
    "Home state methods",
)

replace_once(
    "src/browser.rs",
    '''        if let Some(load_more) = youtube_home_load_more_button(incoming, &self.event_tx, language) {
            home.append(&load_more);
        }
        true
    }
''',
    '''        if let Some(load_more) = youtube_home_load_more_button(incoming, &self.event_tx, language) {
            home.append(&load_more);
        }
        self.home_dirty.set(false);
        true
    }
''',
    "Keep appended Home clean",
)

replace_once(
    "src/browser.rs",
    '''        Self {
            root,
            home_stack,
            home_generation,
            search_content,
''',
    '''        Self {
            root,
            home_stack,
            home_generation,
            home_dirty: Cell::new(true),
            search_content,
''',
    "Initialize Home dirty state",
)

replace_once(
    "src/browser.rs",
    '''        let previous = self.route();
        self.root
            .set_transition_type(route_transition(&previous, &route));
        self.route.replace(route);
        self.refresh(tracks, config, youtube, context, query);
''',
    '''        let previous = self.route();
        self.root
            .set_transition_type(route_transition(&previous, &route));
        let reuse_home = should_reuse_youtube_home(
            &route,
            query,
            config.startup_source == Some(StartupSource::YouTube),
            self.home_dirty.get(),
            self.has_mounted_youtube_home(),
        );
        self.route.replace(route);
        if reuse_home {
            self.root.set_visible_child_name("home");
            return;
        }
        self.refresh(tracks, config, youtube, context, query);
''',
    "Reuse mounted Home on navigation",
)

replace_once(
    "src/browser.rs",
    '''            let generation = self.home_generation.get().wrapping_add(1);
            self.home_generation.set(generation);
            let child_name = format!("home-{generation}");
            let previous = self.home_stack.visible_child();

            self.home_stack.add_named(&next_home, Some(&child_name));
            self.home_stack.set_visible_child_name(&child_name);

            if let Some(previous) = previous {
                let stack = self.home_stack.clone();
                glib::timeout_add_local_once(Duration::from_millis(220), move || {
                    if previous.parent().as_ref() == Some(stack.upcast_ref()) {
                        stack.remove(&previous);
                    }
                });
            }
            return;
''',
    '''            let generation = self.home_generation.get().wrapping_add(1);
            self.home_generation.set(generation);
            let child_name = format!("home-{generation}");
            let previous = self.home_stack.visible_child();

            // A large YouTube Home can contain hundreds of nested widgets after
            // continuation pages are appended. Avoid keeping two full trees alive
            // during a crossfade when a real data refresh is required.
            self.home_stack
                .set_transition_type(gtk::StackTransitionType::None);
            self.home_stack.add_named(&next_home, Some(&child_name));
            self.home_stack.set_visible_child_name(&child_name);

            if let Some(previous) = previous {
                if previous.parent().as_ref() == Some(self.home_stack.upcast_ref()) {
                    self.home_stack.remove(&previous);
                }
            }
            self.home_dirty.set(false);
            return;
''',
    "Avoid duplicate YouTube Home trees",
)

replace_once(
    "src/browser.rs",
    '''        let generation = self.home_generation.get().wrapping_add(1);
        self.home_generation.set(generation);
        let child_name = format!("home-{generation}");
        let previous = self.home_stack.visible_child();

        self.home_stack.add_named(&next_home, Some(&child_name));
''',
    '''        let generation = self.home_generation.get().wrapping_add(1);
        self.home_generation.set(generation);
        let child_name = format!("home-{generation}");
        let previous = self.home_stack.visible_child();

        self.home_stack
            .set_transition_type(gtk::StackTransitionType::Crossfade);
        self.home_stack.add_named(&next_home, Some(&child_name));
''',
    "Restore local Home transition",
)

replace_once(
    "src/browser.rs",
    '''        if let Some(previous) = previous {
            let stack = self.home_stack.clone();
            glib::timeout_add_local_once(Duration::from_millis(220), move || {
                if previous.parent().as_ref() == Some(stack.upcast_ref()) {
                    stack.remove(&previous);
                }
            });
        }
    }

    fn rebuild_albums''',
    '''        if let Some(previous) = previous {
            let stack = self.home_stack.clone();
            glib::timeout_add_local_once(Duration::from_millis(220), move || {
                if previous.parent().as_ref() == Some(stack.upcast_ref()) {
                    stack.remove(&previous);
                }
            });
        }
        self.home_dirty.set(false);
    }

    fn rebuild_albums''',
    "Mark rebuilt Home clean",
)

replace_once(
    "src/app/controller/playback.rs",
    '''        let animate_m3 = playing && self.config.borrow().visual_theme.is_expressive();
        self.home_wave_progress.set_playing(animate_m3);
        self.footer_progress.set_playing(animate_m3);

        if matches!(self.browser.route(), BrowserRoute::All) {
            self.refresh_browser();
        }
''',
    '''        let config = self.config.borrow();
        let animate_m3 = playing && config.visual_theme.is_expressive();
        let language = config.language;
        drop(config);
        self.home_wave_progress.set_playing(animate_m3);
        self.footer_progress.set_playing(animate_m3);

        if matches!(self.browser.route(), BrowserRoute::All) {
            self.browser.update_home_playback_state(playing, language);
        }
''',
    "Target play/pause updates",
)

replace_once(
    "src/app/controller/navigation.rs",
    '''    pub(crate) fn refresh_browser(&self) {
        let home_scroll_positions = self.browser.home_scroll_positions();
''',
    '''    pub(crate) fn refresh_browser(&self) {
        let home_visible = matches!(self.browser.route(), BrowserRoute::All)
            && self.search_query.borrow().trim().is_empty();
        if !home_visible {
            self.browser.mark_home_dirty();
        }

        let home_scroll_positions = self.browser.home_scroll_positions();
''',
    "Invalidate off-screen Home data",
)

print("Home V2 render reuse patch applied")
