// clickable_lyrics_seek_v3
// lyrics_2_v2
use crate::{
    config::AppLanguage,
    lyrics::{active_index, LyricLine},
};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

const INLINE_SLOTS: usize = 5;
const INLINE_CENTER: usize = INLINE_SLOTS / 2;
const INLINE_PANEL_WIDTH: i32 = 384;
const INLINE_PANEL_HEIGHT: i32 = 158;
const INLINE_PAGE_HEIGHT: i32 = 136;
const INLINE_TEXT_WIDTH: i32 = 360;
const INLINE_FOCUSED_HEIGHT: i32 = 44;
const INLINE_SECONDARY_HEIGHT: i32 = 22;
const SCROLL_DURATION: Duration = Duration::from_millis(220);

type LyricsSeekCallback = Box<dyn Fn(i64)>;

#[derive(Clone, Copy)]
struct LyricsCopy {
    full_title: &'static str,
    full_hint: &'static str,
    inline_title: &'static str,
    inline_hint: &'static str,
}

fn lyrics_copy(language: AppLanguage) -> LyricsCopy {
    match language {
        AppLanguage::Portuguese => LyricsCopy {
            full_title: "As letras aparecerão aqui",
            full_hint: "Reproduza uma música com letras sincronizadas para acompanhar cada verso.",
            inline_title: "As letras aparecerão aqui",
            inline_hint: "Reproduza uma música com letras sincronizadas para ver o contexto.",
        },
        AppLanguage::English => LyricsCopy {
            full_title: "Lyrics will appear here",
            full_hint: "Play a song with synchronized lyrics to follow every line.",
            inline_title: "Lyrics will appear here",
            inline_hint: "Play a song with synchronized lyrics to see the surrounding lines.",
        },
        AppLanguage::Spanish => LyricsCopy {
            full_title: "Las letras aparecerán aquí",
            full_hint: "Reproduce una canción con letras sincronizadas para seguir cada verso.",
            inline_title: "Las letras aparecerán aquí",
            inline_hint: "Reproduce una canción con letras sincronizadas para ver el contexto.",
        },
    }
}

#[derive(Clone)]
pub struct LyricsPresenter {
    inner: Rc<LyricsPresenterInner>,
}

struct LyricsPresenterInner {
    lines: RefCell<Vec<LyricLine>>,
    visible_indices: RefCell<Vec<usize>>,
    active_index: Cell<Option<usize>>,
    full_scroll: gtk::ScrolledWindow,
    full_box: gtk::Box,
    seek_callback: RefCell<Option<LyricsSeekCallback>>,
    full_labels: RefCell<Vec<gtk::Label>>,
    inline_stack: gtk::Stack,
    inline_viewport: gtk::ScrolledWindow,
    inline_pages: Vec<InlinePage>,
    inline_visible: Cell<usize>,
    scroll_generation: Rc<Cell<u64>>,
    language: Cell<AppLanguage>,
}

struct InlinePage {
    root: gtk::Box,
    labels: Vec<gtk::Label>,
}

impl LyricsPresenter {
    pub fn new(language: AppLanguage) -> Self {
        let full_box = gtk::Box::new(gtk::Orientation::Vertical, 22);
        full_box.set_margin_top(56);
        full_box.set_margin_bottom(56);
        full_box.set_margin_start(36);
        full_box.set_margin_end(36);
        full_box.set_halign(gtk::Align::Fill);
        full_box.set_hexpand(true);
        full_box.add_css_class("lyrics-page");

        let full_scroll = gtk::ScrolledWindow::new();
        full_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        full_scroll.set_vexpand(true);
        full_scroll.set_child(Some(&full_box));
        full_scroll.add_css_class("lyrics-scroll");

        let page_a = inline_page();
        let page_b = inline_page();
        let inline_stack = gtk::Stack::new();
        inline_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        inline_stack.set_transition_duration(170);
        inline_stack.set_margin_top(4);
        inline_stack.set_margin_bottom(2);
        inline_stack.set_vexpand(false);
        inline_stack.set_valign(gtk::Align::Center);
        inline_stack.set_size_request(INLINE_PANEL_WIDTH, INLINE_PANEL_HEIGHT);
        inline_stack.set_hexpand(false);
        inline_stack.set_halign(gtk::Align::Center);
        inline_stack.set_overflow(gtk::Overflow::Hidden);
        inline_stack.add_css_class("inline-lyrics-panel");
        inline_stack.add_named(&page_a.root, Some("lyrics-a"));
        inline_stack.add_named(&page_b.root, Some("lyrics-b"));
        inline_stack.set_visible_child_name("lyrics-a");

        let inline_viewport = gtk::ScrolledWindow::new();
        inline_viewport.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
        inline_viewport.set_propagate_natural_width(false);
        inline_viewport.set_propagate_natural_height(false);
        inline_viewport.set_min_content_width(INLINE_PANEL_WIDTH);
        inline_viewport.set_max_content_width(INLINE_PANEL_WIDTH);
        inline_viewport.set_min_content_height(INLINE_PANEL_HEIGHT);
        inline_viewport.set_max_content_height(INLINE_PANEL_HEIGHT);
        inline_viewport.set_size_request(INLINE_PANEL_WIDTH, INLINE_PANEL_HEIGHT);
        inline_viewport.set_hexpand(false);
        inline_viewport.set_halign(gtk::Align::Center);
        inline_viewport.set_vexpand(false);
        inline_viewport.set_valign(gtk::Align::Center);
        inline_viewport.set_child(Some(&inline_stack));
        inline_viewport.add_css_class("inline-lyrics-viewport");

        let presenter = Self {
            inner: Rc::new(LyricsPresenterInner {
                lines: RefCell::new(Vec::new()),
                visible_indices: RefCell::new(Vec::new()),
                active_index: Cell::new(None),
                full_scroll,
                full_box,
                seek_callback: RefCell::new(None),
                full_labels: RefCell::new(Vec::new()),
                inline_stack,
                inline_viewport,
                inline_pages: vec![page_a, page_b],
                inline_visible: Cell::new(0),
                scroll_generation: Rc::new(Cell::new(0)),
                language: Cell::new(language),
            }),
        };

        presenter.show_default_state();
        presenter
    }

    // complete_surface_localization_v3
    pub fn set_language(&self, language: AppLanguage) {
        self.inner.language.set(language);
        if self.inner.lines.borrow().is_empty() {
            self.show_default_state();
        }
    }

    fn show_default_state(&self) {
        let text = lyrics_copy(self.inner.language.get());
        self.show_state(
            text.full_title,
            Some(text.full_hint),
            text.inline_title,
            Some(text.inline_hint),
        );
    }

    pub fn full_widget(&self) -> &gtk::ScrolledWindow {
        &self.inner.full_scroll
    }

    pub fn inline_widget(&self) -> &gtk::ScrolledWindow {
        &self.inner.inline_viewport
    }

    pub fn set_lines(&self, lines: &[LyricLine]) {
        self.cancel_scroll();
        self.inner.active_index.set(None);
        self.inner.lines.replace(lines.to_vec());
        self.inner.visible_indices.replace(
            lines
                .iter()
                .enumerate()
                .filter_map(|(index, line)| (!line.text.trim().is_empty()).then_some(index))
                .collect(),
        );

        self.clear_full_widgets();
        let mut labels = Vec::with_capacity(lines.len());
        for line in lines {
            let label = gtk::Label::new(Some(line.text.trim()));
            label.set_wrap(true);
            label.set_justify(gtk::Justification::Center);
            label.set_halign(gtk::Align::Center);
            label.set_hexpand(true);
            label.add_css_class("lyric-line");
            label.add_css_class("lyric-seek-target");
            label.set_cursor_from_name(Some("pointer"));
            label.set_tooltip_text(Some("Ir para este trecho"));

            let timestamp_us = line.timestamp_us;
            let presenter = self.clone();
            let click = gtk::GestureClick::new();
            click.set_button(gtk::gdk::BUTTON_PRIMARY);
            click.connect_released(move |_, _, _, _| {
                presenter.emit_seek(timestamp_us);
            });
            label.add_controller(click);

            self.inner.full_box.append(&label);
            labels.push(label);
        }
        self.inner.full_labels.replace(labels);

        let adjustment = self.inner.full_scroll.vadjustment();
        adjustment.set_value(adjustment.lower());
        self.render_inline(None, self.inner.inline_stack.is_mapped());
    }

    pub fn show_message(&self, message: &str, hint: Option<&str>) {
        self.show_state(message, hint, message, hint);
    }

    pub fn show_state(
        &self,
        full_title: &str,
        full_hint: Option<&str>,
        inline_title: &str,
        inline_hint: Option<&str>,
    ) {
        self.cancel_scroll();
        self.inner.lines.borrow_mut().clear();
        self.inner.visible_indices.borrow_mut().clear();
        self.inner.active_index.set(None);
        self.clear_full_widgets();

        let title = gtk::Label::new(Some(full_title));
        title.set_wrap(true);
        title.set_justify(gtk::Justification::Center);
        title.set_halign(gtk::Align::Center);
        title.add_css_class("title-2");
        self.inner.full_box.append(&title);

        if let Some(hint) = full_hint.filter(|hint| !hint.trim().is_empty()) {
            let hint = gtk::Label::new(Some(hint));
            hint.set_wrap(true);
            hint.set_justify(gtk::Justification::Center);
            hint.set_halign(gtk::Align::Center);
            hint.add_css_class("dim-label");
            self.inner.full_box.append(&hint);
        }

        self.render_inline_message(
            inline_title,
            inline_hint,
            self.inner.inline_stack.is_mapped(),
        );
    }

    pub fn connect_seek<F>(&self, callback: F)
    where
        F: Fn(i64) + 'static,
    {
        self.inner.seek_callback.replace(Some(Box::new(callback)));
    }

    fn emit_seek(&self, timestamp_us: i64) {
        if let Some(callback) = self.inner.seek_callback.borrow().as_ref() {
            callback(timestamp_us.max(0));
        }
    }

    pub fn update_timestamp(&self, timestamp_us: i64) {
        let current = {
            let lines = self.inner.lines.borrow();
            active_index(&lines, timestamp_us)
        };
        let previous = self.inner.active_index.replace(current);
        if previous == current {
            return;
        }

        self.update_full_classes(current);
        self.render_inline(current, true);
        if let Some(index) = current {
            self.scroll_to(index, true);
        }
    }

    pub fn recenter(&self, animate: bool) {
        let index = self
            .inner
            .active_index
            .get()
            .or_else(|| (!self.inner.full_labels.borrow().is_empty()).then_some(0));
        if let Some(index) = index {
            self.scroll_to(index, animate);
        }
    }

    fn clear_full_widgets(&self) {
        self.inner.full_labels.borrow_mut().clear();
        while let Some(child) = self.inner.full_box.first_child() {
            self.inner.full_box.remove(&child);
        }
    }

    fn update_full_classes(&self, current: Option<usize>) {
        for (index, label) in self.inner.full_labels.borrow().iter().enumerate() {
            if Some(index) == current {
                label.remove_css_class("past-lyric");
                label.add_css_class("current-lyric");
            } else {
                label.remove_css_class("current-lyric");
                if current.is_some_and(|active| index < active) {
                    label.add_css_class("past-lyric");
                } else {
                    label.remove_css_class("past-lyric");
                }
            }
        }
    }

    fn render_inline(&self, current: Option<usize>, animate: bool) {
        let lines = self.inner.lines.borrow();
        let visible = self.inner.visible_indices.borrow();
        if visible.is_empty() {
            self.render_inline_message("♪", None, animate);
            return;
        }

        let active_visible = current
            .and_then(|index| visible.binary_search(&index).ok())
            .unwrap_or(0);
        let target = self.inline_target_page(animate);
        fill_inline_lines(
            &self.inner.inline_pages[target],
            &lines,
            &visible,
            active_visible,
        );
        self.show_inline_page(target);
    }

    fn render_inline_message(&self, title: &str, hint: Option<&str>, animate: bool) {
        let target = self.inline_target_page(animate);
        let page = &self.inner.inline_pages[target];
        for (slot, label) in page.labels.iter().enumerate() {
            set_inline_label_text(label, "", slot == INLINE_CENTER);
        }
        set_inline_label_text(&page.labels[INLINE_CENTER], title, true);
        if let Some(hint) = hint.filter(|hint| !hint.trim().is_empty()) {
            if INLINE_CENTER + 1 < page.labels.len() {
                set_inline_label_text(&page.labels[INLINE_CENTER + 1], hint, false);
            }
        }
        self.show_inline_page(target);
    }

    fn inline_target_page(&self, animate: bool) -> usize {
        let visible = self.inner.inline_visible.get();
        if animate && self.inner.inline_stack.is_mapped() {
            1 - visible
        } else {
            visible
        }
    }

    fn show_inline_page(&self, target: usize) {
        if target == self.inner.inline_visible.get() {
            return;
        }
        self.inner
            .inline_stack
            .set_visible_child_name(if target == 0 { "lyrics-a" } else { "lyrics-b" });
        self.inner.inline_visible.set(target);
    }

    fn scroll_to(&self, index: usize, animate: bool) {
        if !self.inner.full_scroll.is_mapped() {
            return;
        }
        let Some(label) = self.inner.full_labels.borrow().get(index).cloned() else {
            return;
        };

        let scroll = self.inner.full_scroll.clone();
        let generation = self.inner.scroll_generation.clone();
        let token = generation.get().wrapping_add(1);
        generation.set(token);

        glib::idle_add_local_once(move || {
            if generation.get() != token || !scroll.is_mapped() {
                return;
            }

            let adjustment = scroll.vadjustment();
            let Some(content) = scroll.child() else {
                return;
            };
            let Some(bounds) = label.compute_bounds(&content) else {
                return;
            };
            let lower = adjustment.lower();
            let upper = (adjustment.upper() - adjustment.page_size()).max(lower);
            let center = bounds.y() as f64 + bounds.height() as f64 / 2.0;
            let target = (center - adjustment.page_size() / 2.0).clamp(lower, upper);
            let start = adjustment.value();

            if !animate || !animations_enabled() || (target - start).abs() < 1.0 {
                adjustment.set_value(target);
                return;
            }

            let started = Instant::now();
            glib::timeout_add_local(Duration::from_millis(16), move || {
                if generation.get() != token {
                    return glib::ControlFlow::Break;
                }

                let progress =
                    (started.elapsed().as_secs_f64() / SCROLL_DURATION.as_secs_f64()).min(1.0);
                let eased = 1.0 - (1.0 - progress).powi(3);
                adjustment.set_value(start + (target - start) * eased);

                if progress >= 1.0 {
                    adjustment.set_value(target);
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        });
    }

    fn cancel_scroll(&self) {
        let next = self.inner.scroll_generation.get().wrapping_add(1);
        self.inner.scroll_generation.set(next);
    }
}

fn inline_page() -> InlinePage {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.set_size_request(INLINE_PANEL_WIDTH, INLINE_PAGE_HEIGHT);
    root.set_hexpand(false);
    root.set_halign(gtk::Align::Center);
    root.set_vexpand(false);
    root.set_valign(gtk::Align::Center);
    root.set_overflow(gtk::Overflow::Hidden);
    root.add_css_class("inline-lyrics-page");

    let mut labels = Vec::with_capacity(INLINE_SLOTS);
    for index in 0..INLINE_SLOTS {
        let label = gtk::Label::new(None);
        label.set_justify(gtk::Justification::Center);
        label.set_halign(gtk::Align::Center);
        label.set_hexpand(false);
        label.set_width_request(INLINE_TEXT_WIDTH);
        label.set_width_chars(-1);
        label.set_max_width_chars(-1);
        label.set_overflow(gtk::Overflow::Hidden);

        if index == INLINE_CENTER {
            label.set_wrap(false);
            label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            label.set_single_line_mode(true);
            label.set_ellipsize(gtk::pango::EllipsizeMode::None);
            label.set_lines(1);
            label.set_size_request(INLINE_TEXT_WIDTH, INLINE_FOCUSED_HEIGHT);
        } else {
            label.set_wrap(false);
            label.set_single_line_mode(true);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_lines(1);
            label.set_size_request(INLINE_TEXT_WIDTH, INLINE_SECONDARY_HEIGHT);
        }
        label.add_css_class("inline-lyric-line");
        match index {
            INLINE_CENTER => label.add_css_class("inline-lyric-current"),
            1 | 3 => label.add_css_class("inline-lyric-near"),
            _ => label.add_css_class("inline-lyric-far"),
        }
        root.append(&label);
        labels.push(label);
    }
    InlinePage { root, labels }
}

fn normalize_inline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn should_wrap_inline(focused: bool, natural_width: i32) -> bool {
    focused && natural_width > INLINE_TEXT_WIDTH
}

fn set_inline_label_text(label: &gtk::Label, text: &str, focused: bool) {
    let normalized = normalize_inline_text(text);
    label.set_size_request(
        INLINE_TEXT_WIDTH,
        if focused {
            INLINE_FOCUSED_HEIGHT
        } else {
            INLINE_SECONDARY_HEIGHT
        },
    );

    let natural_width = if normalized.is_empty() {
        0
    } else {
        label.create_pango_layout(Some(&normalized)).pixel_size().0
    };

    let should_wrap = should_wrap_inline(focused, natural_width);

    label.set_wrap(should_wrap);
    label.set_single_line_mode(!should_wrap);

    if should_wrap {
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_lines(2);
    } else {
        label.set_ellipsize(if focused {
            gtk::pango::EllipsizeMode::None
        } else {
            gtk::pango::EllipsizeMode::End
        });
        label.set_lines(1);
    }

    label.set_text(&normalized);
}

fn fill_inline_lines(
    page: &InlinePage,
    lines: &[LyricLine],
    visible: &[usize],
    active_visible: usize,
) {
    for (slot, label) in page.labels.iter().enumerate() {
        set_inline_label_text(label, "", slot == INLINE_CENTER);
    }

    for (slot, offset) in (-2_isize..=2).enumerate() {
        let position = active_visible as isize + offset;
        if position < 0 || position >= visible.len() as isize {
            continue;
        }
        let line_index = visible[position as usize];
        set_inline_label_text(
            &page.labels[slot],
            lines[line_index].text.trim(),
            slot == INLINE_CENTER,
        );
    }
}

fn animations_enabled() -> bool {
    gtk::Settings::default()
        .map(|settings| settings.property::<bool>("gtk-enable-animations"))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_text_normalization_removes_hidden_layout_changes() {
        assert_eq!(
            normalize_inline_text("  one\n\ttwo   three  "),
            "one two three"
        );
    }

    #[test]
    fn only_the_focused_line_can_wrap() {
        assert!(!should_wrap_inline(false, INLINE_TEXT_WIDTH + 200));
        assert!(!should_wrap_inline(true, INLINE_TEXT_WIDTH));
        assert!(should_wrap_inline(true, INLINE_TEXT_WIDTH + 1));
    }

    #[test]
    fn inline_geometry_is_constant() {
        assert_eq!(INLINE_PANEL_WIDTH, 384);
        assert_eq!(INLINE_PANEL_HEIGHT, 158);
        assert_eq!(INLINE_PAGE_HEIGHT, 136);
        assert_eq!(INLINE_FOCUSED_HEIGHT, 44);
        assert_eq!(INLINE_SECONDARY_HEIGHT, 22);
    }
}
