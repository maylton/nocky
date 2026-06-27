//! Queue controller methods for `AppController`.

use super::*;

impl AppController {
    // functional_carousel_queue_blur_fix_v1
    // queue2_interface_v1
    pub(crate) fn rebuild_queue_popover(
        self: &Rc<Self>,
        list: &gtk::Box,
        summary: &gtk::Label,
        clear_upcoming: &gtk::Button,
        popover: &gtk::Popover,
    ) {
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        let presentation = {
            let queue = self.playback_queue_v2.borrow();
            QueuePresentation::from_queue(&queue, self.active_queue_source.get())
        };

        let language = self.config.borrow().language;
        let current_index = presentation.current_index;
        let count = presentation.total;
        let source_label = match (language, presentation.source) {
            (AppLanguage::Portuguese, QueueSourceKind::Local) => "Biblioteca local",
            (AppLanguage::Portuguese, QueueSourceKind::YouTube) => "YouTube Music",
            (AppLanguage::English, QueueSourceKind::Local) => "Local library",
            (AppLanguage::English, QueueSourceKind::YouTube) => "YouTube Music",
            (AppLanguage::Spanish, QueueSourceKind::Local) => "Biblioteca local",
            (AppLanguage::Spanish, QueueSourceKind::YouTube) => "YouTube Music",
        };
        let summary_text = match language {
            AppLanguage::Portuguese => format!(
                "{source_label} • {} {} • {count} {}",
                presentation.upcoming_count,
                if presentation.upcoming_count == 1 {
                    "próxima"
                } else {
                    "próximas"
                },
                if count == 1 { "faixa" } else { "faixas" }
            ),
            AppLanguage::English => format!(
                "{source_label} • {} up next • {count} {}",
                presentation.upcoming_count,
                if count == 1 { "track" } else { "tracks" }
            ),
            AppLanguage::Spanish => format!(
                "{source_label} • {} {} • {count} {}",
                presentation.upcoming_count,
                if presentation.upcoming_count == 1 {
                    "siguiente"
                } else {
                    "siguientes"
                },
                if count == 1 { "pista" } else { "pistas" }
            ),
        };
        summary.set_text(&summary_text);
        clear_upcoming.set_sensitive(presentation.can_clear_upcoming());

        if presentation.items.is_empty() {
            // queue2_interface_polish_v1: richer empty state
            let empty = gtk::Box::new(gtk::Orientation::Vertical, 7);
            empty.set_margin_top(18);
            empty.set_margin_bottom(18);
            empty.set_margin_start(12);
            empty.set_margin_end(12);
            empty.set_halign(gtk::Align::Fill);
            empty.set_valign(gtk::Align::Center);
            empty.add_css_class("queue2-state");
            empty.add_css_class("queue2-empty-state");

            let icon = gtk::Image::from_icon_name("view-list-symbolic");
            icon.set_pixel_size(34);
            icon.add_css_class("queue2-state-icon");

            let title = gtk::Label::new(Some(match language {
                AppLanguage::Portuguese => "A fila está vazia",
                AppLanguage::English => "The queue is empty",
                AppLanguage::Spanish => "La cola está vacía",
            }));
            title.add_css_class("queue2-state-title");

            let description = gtk::Label::new(Some(match language {
                AppLanguage::Portuguese => {
                    "Use “Reproduzir em seguida” ou “Adicionar ao fim” nas faixas."
                }
                AppLanguage::English => "Use “Play next” or “Add to end” from any track.",
                AppLanguage::Spanish => {
                    "Usa “Reproducir después” o “Añadir al final” en una pista."
                }
            }));
            description.set_wrap(true);
            description.set_justify(gtk::Justification::Center);
            description.add_css_class("dim-label");
            description.add_css_class("queue2-state-description");

            empty.append(&icon);
            empty.append(&title);
            empty.append(&description);
            list.append(&empty);
            return;
        }

        let mut rendered_section = None;
        for item in presentation.items.iter().cloned() {
            if rendered_section != Some(item.section) {
                let section_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                section_header.set_margin_top(if rendered_section.is_some() { 12 } else { 4 });
                section_header.set_margin_bottom(4);
                section_header.set_margin_start(8);
                section_header.set_margin_end(8);
                section_header.add_css_class("queue2-section-header");

                let icon_name = match item.section {
                    QueueSection::Played => "document-open-recent-symbolic",
                    QueueSection::Current => "media-playback-start-symbolic",
                    QueueSection::Upcoming => "view-list-symbolic",
                };
                let icon = gtk::Image::from_icon_name(icon_name);
                icon.set_pixel_size(15);
                icon.add_css_class("queue2-section-icon");

                let section_count = presentation.section_count(item.section);
                let section_title = match (language, item.section) {
                    (AppLanguage::Portuguese, QueueSection::Played) => "Reproduzidas",
                    (AppLanguage::Portuguese, QueueSection::Current) => "Tocando agora",
                    (AppLanguage::Portuguese, QueueSection::Upcoming) => "Próximas",
                    (AppLanguage::English, QueueSection::Played) => "Previously played",
                    (AppLanguage::English, QueueSection::Current) => "Now playing",
                    (AppLanguage::English, QueueSection::Upcoming) => "Up next",
                    (AppLanguage::Spanish, QueueSection::Played) => "Reproducidas",
                    (AppLanguage::Spanish, QueueSection::Current) => "Reproduciendo ahora",
                    (AppLanguage::Spanish, QueueSection::Upcoming) => "Siguientes",
                };
                let section_label =
                    gtk::Label::new(Some(&format!("{section_title} · {section_count}")));
                section_label.set_xalign(0.0);
                section_label.set_hexpand(true);
                section_label.add_css_class("queue2-section-title");

                section_header.append(&icon);
                section_header.append(&section_label);
                list.append(&section_header);
                rendered_section = Some(item.section);
            }

            let position = item.position;
            let entry = item.entry;
            let is_current = item.is_current;

            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_top(4);
            row.set_margin_bottom(4);
            row.set_margin_start(4);
            row.set_margin_end(4);
            row.add_css_class("queue2-row");
            if is_current {
                row.add_css_class("active");
            }

            // queue2_drag_indicator_v1
            // Keep the widget that owns GestureDrag parented and intact.
            // A compact accent marker moves through the list to show the
            // destination without duplicating the whole track row.
            let drag_icon = gtk::Image::from_icon_name("list-drag-handle-symbolic");
            drag_icon.set_pixel_size(18);
            drag_icon.set_can_target(false);

            // queue2_interface_polish_v1: semantic drag handle with keyboard operation
            let drag_handle = gtk::Button::new();
            drag_handle.set_size_request(34, 34);
            drag_handle.set_halign(gtk::Align::Center);
            drag_handle.set_valign(gtk::Align::Center);
            drag_handle.set_focusable(true);
            drag_handle.set_cursor_from_name(Some("grab"));
            drag_handle.set_tooltip_text(Some(match language {
                AppLanguage::Portuguese => "Arraste ou use Alt+↑ / Alt+↓ para reordenar",
                AppLanguage::English => "Drag or use Alt+↑ / Alt+↓ to reorder",
                AppLanguage::Spanish => "Arrastra o usa Alt+↑ / Alt+↓ para reordenar",
            }));
            drag_handle.add_css_class("flat");
            drag_handle.add_css_class("circular");
            drag_handle.add_css_class("queue2-drag-handle");
            drag_handle.set_child(Some(&drag_icon));

            let drag_origin = Rc::new(Cell::new(position));
            let drag_target = Rc::new(Cell::new(position));
            let drag_indicator: Rc<RefCell<Option<gtk::Box>>> = Rc::new(RefCell::new(None));

            let drag_gesture = gtk::GestureDrag::new();
            drag_gesture.set_button(gdk::BUTTON_PRIMARY);
            drag_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

            {
                let weak = Rc::downgrade(self);
                let handle = drag_handle.clone();
                let row = row.clone();
                let list = list.clone();
                let drag_origin = drag_origin.clone();
                let drag_target = drag_target.clone();
                let drag_indicator = drag_indicator.clone();
                let id = entry.id;

                drag_gesture.connect_drag_begin(move |_, _, _| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };

                    drag_origin.set(position);
                    drag_target.set(position);
                    controller.queue_dragged_entry.set(Some(id));
                    handle.set_cursor_from_name(Some("grabbing"));
                    row.set_opacity(0.48);
                    row.add_css_class("queue2-live-dragging");

                    let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    indicator.set_height_request(9);
                    indicator.set_margin_start(38);
                    indicator.set_margin_end(16);
                    indicator.set_can_target(false);
                    indicator.add_css_class("queue2-drop-indicator");

                    let accent_line = gtk::ProgressBar::new();
                    accent_line.set_fraction(1.0);
                    accent_line.set_height_request(3);
                    accent_line.set_hexpand(true);
                    accent_line.set_valign(gtk::Align::Center);
                    accent_line.set_can_target(false);
                    accent_line.add_css_class("queue2-drop-indicator-line");

                    indicator.append(&accent_line);
                    list.insert_child_after(&indicator, Some(&row));
                    drag_indicator.replace(Some(indicator));
                });
            }

            {
                let list = list.clone();
                let row = row.clone();
                let drag_target = drag_target.clone();
                let drag_indicator = drag_indicator.clone();

                drag_gesture.connect_drag_update(move |_, _, offset_y| {
                    let indicator = {
                        let stored = drag_indicator.borrow();
                        stored.as_ref().cloned()
                    };
                    let Some(indicator) = indicator else {
                        return;
                    };

                    let row_widget: gtk::Widget = row.clone().upcast();
                    let indicator_widget: gtk::Widget = indicator.clone().upcast();
                    let Some(row_origin) =
                        row.compute_point(&list, &gtk::graphene::Point::new(0.0, 0.0))
                    else {
                        return;
                    };
                    let pointer_y =
                        row_origin.y() as f64 + row.height().max(1) as f64 / 2.0 + offset_y;

                    let mut queue_rows = Vec::with_capacity(count.saturating_sub(1));
                    let mut child = list.first_child();
                    while let Some(widget) = child {
                        let next = widget.next_sibling();
                        if widget != row_widget
                            && widget != indicator_widget
                            && widget.has_css_class("queue2-row")
                        {
                            queue_rows.push(widget);
                        }
                        child = next;
                    }

                    let mut target = 0usize;
                    for candidate in &queue_rows {
                        let Some(origin) =
                            candidate.compute_point(&list, &gtk::graphene::Point::new(0.0, 0.0))
                        else {
                            continue;
                        };
                        let midpoint = origin.y() as f64 + candidate.height().max(1) as f64 / 2.0;
                        if pointer_y > midpoint {
                            target = target.saturating_add(1);
                        }
                    }
                    target = target.min(count.saturating_sub(1));

                    if target == drag_target.get() {
                        return;
                    }

                    let previous_sibling = if let Some(target_row) = queue_rows.get(target) {
                        target_row.prev_sibling()
                    } else {
                        queue_rows.last().cloned()
                    };

                    list.reorder_child_after(&indicator, previous_sibling.as_ref());
                    drag_target.set(target);
                });
            }

            {
                let weak = Rc::downgrade(self);
                let handle = drag_handle.clone();
                let row = row.clone();
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let drag_origin = drag_origin.clone();
                let drag_target = drag_target.clone();
                let drag_indicator = drag_indicator.clone();
                let fallback_id = entry.id;

                drag_gesture.connect_drag_end(move |_, _, _| {
                    handle.set_cursor_from_name(Some("grab"));
                    row.set_opacity(1.0);
                    row.remove_css_class("queue2-live-dragging");

                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    let id = controller
                        .queue_dragged_entry
                        .replace(None)
                        .unwrap_or(fallback_id);
                    let origin = drag_origin.get();
                    let target = drag_target.get();
                    let indicator = drag_indicator.borrow_mut().take();

                    let idle_list = list.clone();
                    let idle_summary = summary.clone();
                    let idle_clear_upcoming = clear_upcoming.clone();
                    let idle_queue_popover = queue_popover.clone();

                    glib::idle_add_local_once(move || {
                        if let Some(indicator) = indicator {
                            if indicator.parent().is_some() {
                                idle_list.remove(&indicator);
                            }
                        }

                        if target != origin {
                            if let Err(error) = controller
                                .playback_queue_v2
                                .borrow_mut()
                                .move_entry(id, target)
                            {
                                controller.show_toast(&error.to_string());
                            }
                        }

                        controller.rebuild_queue_popover(
                            &idle_list,
                            &idle_summary,
                            &idle_clear_upcoming,
                            &idle_queue_popover,
                        );
                    });
                });
            }

            {
                let weak = Rc::downgrade(self);
                let handle = drag_handle.clone();
                let row = row.clone();
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let drag_indicator = drag_indicator.clone();

                drag_gesture.connect_cancel(move |_, _| {
                    handle.set_cursor_from_name(Some("grab"));
                    row.set_opacity(1.0);
                    row.remove_css_class("queue2-live-dragging");

                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    controller.queue_dragged_entry.set(None);
                    let indicator = drag_indicator.borrow_mut().take();

                    let idle_list = list.clone();
                    let idle_summary = summary.clone();
                    let idle_clear_upcoming = clear_upcoming.clone();
                    let idle_queue_popover = queue_popover.clone();

                    glib::idle_add_local_once(move || {
                        if let Some(indicator) = indicator {
                            if indicator.parent().is_some() {
                                idle_list.remove(&indicator);
                            }
                        }
                        controller.rebuild_queue_popover(
                            &idle_list,
                            &idle_summary,
                            &idle_clear_upcoming,
                            &idle_queue_popover,
                        );
                    });
                });
            }

            drag_handle.add_controller(drag_gesture);

            // queue2_interface_polish_v1: Alt+Up / Alt+Down mirrors pointer reordering.
            let key_controller = gtk::EventControllerKey::new();
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;

                key_controller.connect_key_pressed(move |_, key, _, state| {
                    if !state.contains(gdk::ModifierType::ALT_MASK) {
                        return glib::Propagation::Proceed;
                    }

                    let target = match key {
                        gdk::Key::Up if position > 0 => Some(position - 1),
                        gdk::Key::Down if position + 1 < count => Some(position + 1),
                        _ => None,
                    };
                    let Some(target) = target else {
                        return glib::Propagation::Proceed;
                    };

                    let Some(controller) = weak.upgrade() else {
                        return glib::Propagation::Proceed;
                    };

                    if let Err(error) = controller
                        .playback_queue_v2
                        .borrow_mut()
                        .move_entry(id, target)
                    {
                        controller.show_toast(&error.to_string());
                        return glib::Propagation::Stop;
                    }

                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );

                    let focus_list = list.clone();
                    glib::idle_add_local_once(move || {
                        let mut child = focus_list.first_child();
                        for _ in 0..target {
                            child = child.and_then(|widget| widget.next_sibling());
                        }
                        if let Some(row) = child {
                            if let Some(handle) = row.first_child() {
                                handle.grab_focus();
                            }
                        }
                    });

                    glib::Propagation::Stop
                });
            }
            drag_handle.add_controller(key_controller);
            row.append(&drag_handle);

            let play_area = gtk::Button::new();
            play_area.set_hexpand(true);
            play_area.set_halign(gtk::Align::Fill);
            play_area.add_css_class("flat");
            play_area.add_css_class("queue-popover-row");
            play_area.set_tooltip_text(Some(match language {
                AppLanguage::Portuguese => "Reproduzir esta faixa",
                AppLanguage::English => "Play this track",
                AppLanguage::Spanish => "Reproducir esta pista",
            }));

            let information = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            information.set_margin_top(8);
            information.set_margin_bottom(8);
            information.set_margin_start(10);
            information.set_margin_end(8);

            // queue2_completion_core_v1: real artwork with fixed natural size.
            let artwork = build_cover(42);
            artwork.stack.add_css_class("queue2-cover");
            artwork.set_path_immediate(entry.media.cover_path.as_deref());
            information.append(&artwork.stack);

            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_hexpand(true);

            let title = gtk::Label::new(Some(&entry.media.title));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.add_css_class("heading");

            let artist_text = if entry.media.artist.trim().is_empty() {
                match &entry.media.source {
                    QueueSource::Local { .. } => match language {
                        AppLanguage::Portuguese => "Artista desconhecido",
                        AppLanguage::English => "Unknown artist",
                        AppLanguage::Spanish => "Artista desconocido",
                    },
                    QueueSource::YouTube { .. } => "YouTube Music",
                }
            } else {
                entry.media.artist.as_str()
            };
            let artist = gtk::Label::new(Some(artist_text));
            artist.set_xalign(0.0);
            artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
            artist.add_css_class("dim-label");

            text.append(&title);
            text.append(&artist);
            information.append(&text);

            let source = gtk::Label::new(Some(match &entry.media.source {
                QueueSource::Local { .. } => "LOCAL",
                QueueSource::YouTube { .. } => "YOUTUBE",
            }));
            source.add_css_class("caption");
            source.add_css_class("dim-label");
            information.append(&source);

            if is_current {
                let playing = gtk::Image::from_icon_name("audio-volume-high-symbolic");
                playing.set_pixel_size(16);
                playing.add_css_class("accent");
                playing.add_css_class("queue-playing-indicator");
                information.append(&playing);
                play_area.add_css_class("active");
                play_area.set_can_target(false);
                play_area.set_focusable(false);
            }

            play_area.set_child(Some(&information));
            if !is_current {
                let weak = Rc::downgrade(self);
                let queue_popover = popover.clone();
                let id = entry.id;
                play_area.connect_clicked(move |_| {
                    if let Some(controller) = weak.upgrade() {
                        controller.play_queue_entry(id, true);
                        queue_popover.popdown();
                    }
                });
            }
            row.append(&play_area);

            let move_top = gtk::Button::builder()
                .icon_name("go-top-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Mover para o topo",
                    AppLanguage::English => "Move to top",
                    AppLanguage::Spanish => "Mover al inicio",
                })
                .build();
            move_top.add_css_class("flat");
            move_top.add_css_class("circular");
            move_top.set_sensitive(!is_current && position > 0);
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                move_top.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    if let Err(error) = controller.playback_queue_v2.borrow_mut().move_entry(id, 0)
                    {
                        controller.show_toast(&error.to_string());
                        return;
                    }
                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );
                });
            }
            row.append(&move_top);

            let play_next = gtk::Button::builder()
                .icon_name("media-skip-forward-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Tocar em seguida",
                    AppLanguage::English => "Play next",
                    AppLanguage::Spanish => "Reproducir después",
                })
                .build();
            play_next.add_css_class("flat");
            play_next.add_css_class("circular");
            let play_next_target = current_index.map(|index| index + 1).unwrap_or(0);
            play_next.set_sensitive(
                !is_current
                    && item.section == QueueSection::Upcoming
                    && position != play_next_target,
            );
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                play_next.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    let target = controller
                        .playback_queue_v2
                        .borrow()
                        .current_index()
                        .map(|index| index + 1)
                        .unwrap_or(0);
                    if let Err(error) = controller
                        .playback_queue_v2
                        .borrow_mut()
                        .move_entry(id, target)
                    {
                        controller.show_toast(&error.to_string());
                        return;
                    }
                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );
                });
            }
            row.append(&play_next);

            let move_up = gtk::Button::builder()
                .icon_name("go-up-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Mover para cima",
                    AppLanguage::English => "Move up",
                    AppLanguage::Spanish => "Mover hacia arriba",
                })
                .build();
            move_up.add_css_class("flat");
            move_up.add_css_class("circular");
            move_up.set_sensitive(position > 0);
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                move_up.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    let result = controller
                        .playback_queue_v2
                        .borrow_mut()
                        .move_entry(id, position.saturating_sub(1));
                    if let Err(error) = result {
                        controller.show_toast(&error.to_string());
                        return;
                    }
                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );
                });
            }
            row.append(&move_up);

            let move_down = gtk::Button::builder()
                .icon_name("go-down-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Mover para baixo",
                    AppLanguage::English => "Move down",
                    AppLanguage::Spanish => "Mover hacia abajo",
                })
                .build();
            move_down.add_css_class("flat");
            move_down.add_css_class("circular");
            move_down.set_sensitive(position + 1 < count);
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                move_down.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    let result = controller
                        .playback_queue_v2
                        .borrow_mut()
                        .move_entry(id, position.saturating_add(1));
                    if let Err(error) = result {
                        controller.show_toast(&error.to_string());
                        return;
                    }
                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );
                });
            }
            row.append(&move_down);

            let remove = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Remover da fila",
                    AppLanguage::English => "Remove from queue",
                    AppLanguage::Spanish => "Quitar de la cola",
                })
                .build();
            remove.add_css_class("flat");
            remove.add_css_class("circular");
            remove.set_sensitive(!is_current);
            {
                let weak = Rc::downgrade(self);
                let row = row.clone();
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                remove.connect_clicked(move |button| {
                    button.set_sensitive(false);
                    row.add_css_class("queue2-row-leaving");

                    let weak = weak.clone();
                    let list = list.clone();
                    let summary = summary.clone();
                    let clear_upcoming = clear_upcoming.clone();
                    let queue_popover = queue_popover.clone();

                    glib::timeout_add_local_once(Duration::from_millis(150), move || {
                        let Some(controller) = weak.upgrade() else {
                            return;
                        };
                        let result = controller.playback_queue_v2.borrow_mut().remove(id);
                        if let Err(error) = result {
                            controller.show_toast(&error.to_string());
                            return;
                        }
                        controller.rebuild_queue_popover(
                            &list,
                            &summary,
                            &clear_upcoming,
                            &queue_popover,
                        );
                    });
                });
            }
            row.append(&remove);

            row.add_css_class("queue2-row-entering");
            list.append(&row);
            let entering_row = row.clone();
            glib::idle_add_local_once(move || {
                entering_row.remove_css_class("queue2-row-entering");
            });
        }

        if current_index.is_some_and(|position| position.saturating_add(1) >= count) {
            // queue2_interface_polish_v1: explicit end-of-queue state
            let end_state = gtk::Box::new(gtk::Orientation::Horizontal, 9);
            end_state.set_halign(gtk::Align::Fill);
            end_state.set_valign(gtk::Align::Center);
            end_state.add_css_class("queue2-end-state");

            let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
            icon.set_pixel_size(18);
            icon.add_css_class("queue2-end-icon");

            let label = gtk::Label::new(Some(match language {
                AppLanguage::Portuguese => "Fim da fila",
                AppLanguage::English => "End of queue",
                AppLanguage::Spanish => "Fin de la cola",
            }));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.add_css_class("dim-label");

            end_state.append(&icon);
            end_state.append(&label);
            list.append(&end_state);
        }
    }

    pub(crate) fn refresh_queue_page(self: &Rc<Self>) {
        self.ensure_active_queue_v2();

        let source = self.active_queue_source.get();
        let (snapshot, presentation) = {
            let queue = self.playback_queue_v2.borrow();
            (
                queue.snapshot(),
                QueuePresentation::from_queue(&queue, source),
            )
        };
        let unchanged = self.queue_page_last_source.get() == Some(source)
            && self.queue_page_last_snapshot.borrow().as_ref() == Some(&snapshot);

        if unchanged {
            return;
        }

        self.rebuild_queue_popover(
            &self.queue_page_list,
            &self.queue_page_summary,
            &self.queue_page_clear_upcoming,
            &self.queue_page_popover_proxy,
        );
        self.refresh_queue_page_header_badges(&presentation);
        self.queue_page_clear_all
            .set_sensitive(!snapshot.entries.is_empty());
        self.queue_page_last_source.set(Some(source));
        self.queue_page_last_snapshot.replace(Some(snapshot));
    }

    pub(crate) fn refresh_queue_page_header_badges(&self, presentation: &QueuePresentation) {
        let source_label = match (self.config.borrow().language, presentation.source) {
            (AppLanguage::Portuguese, QueueSourceKind::Local) => "Biblioteca local",
            (AppLanguage::Portuguese, QueueSourceKind::YouTube) => "YouTube Music",
            (AppLanguage::English, QueueSourceKind::Local) => "Local library",
            (AppLanguage::English, QueueSourceKind::YouTube) => "YouTube Music",
            (AppLanguage::Spanish, QueueSourceKind::Local) => "Biblioteca local",
            (AppLanguage::Spanish, QueueSourceKind::YouTube) => "YouTube Music",
        };
        let count = presentation.total;
        let upcoming_text = match self.config.borrow().language {
            AppLanguage::Portuguese => format!(
                "{} {}",
                presentation.upcoming_count,
                if presentation.upcoming_count == 1 {
                    "próxima"
                } else {
                    "próximas"
                }
            ),
            AppLanguage::English => format!("{} {}", presentation.upcoming_count, "up next"),
            AppLanguage::Spanish => format!(
                "{} {}",
                presentation.upcoming_count,
                if presentation.upcoming_count == 1 {
                    "siguiente"
                } else {
                    "siguientes"
                }
            ),
        };
        let total_text = match self.config.borrow().language {
            AppLanguage::Portuguese => {
                format!("{count} {}", if count == 1 { "faixa" } else { "faixas" })
            }
            AppLanguage::English => {
                format!("{count} {}", if count == 1 { "track" } else { "tracks" })
            }
            AppLanguage::Spanish => {
                format!("{count} {}", if count == 1 { "pista" } else { "pistas" })
            }
        };

        self.queue_page_source.set_text(source_label);
        self.queue_page_upcoming_badge.set_text(&upcoming_text);
        self.queue_page_total_badge.set_text(&total_text);
    }

    pub(crate) fn show_footer_playback_queue(self: &Rc<Self>) {
        self.ensure_active_queue_v2();

        let popover = gtk::Popover::new();
        popover.set_has_arrow(true);
        popover.set_autohide(true);
        popover.set_position(gtk::PositionType::Top);
        popover.set_parent(&self.footer_now_playing);
        popover.add_css_class("queue-popover");
        popover.add_css_class("queue2-popover");
        self.apply_popup_visual_theme(&popover);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_size_request(520, -1);
        content.add_css_class("queue-popover-content");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);

        let heading_text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        heading_text.set_hexpand(true);

        let heading = gtk::Label::new(Some(match self.config.borrow().language {
            AppLanguage::Portuguese => "Fila de reprodução",
            AppLanguage::English => "Playback queue",
            AppLanguage::Spanish => "Cola de reproducción",
        }));
        heading.set_xalign(0.0);
        heading.add_css_class("title-3");

        let summary = gtk::Label::new(None);
        summary.set_xalign(0.0);
        summary.add_css_class("dim-label");
        summary.set_tooltip_text(Some(match self.config.borrow().language {
            AppLanguage::Portuguese => "Atalho de reordenação: Alt+↑ / Alt+↓",
            AppLanguage::English => "Reorder shortcut: Alt+↑ / Alt+↓",
            AppLanguage::Spanish => "Atajo para reordenar: Alt+↑ / Alt+↓",
        }));

        heading_text.append(&heading);
        heading_text.append(&summary);
        header.append(&heading_text);

        let clear_upcoming = gtk::Button::builder()
            .icon_name("edit-clear-all-symbolic")
            .tooltip_text(match self.config.borrow().language {
                AppLanguage::Portuguese => "Limpar próximas",
                AppLanguage::English => "Clear upcoming",
                AppLanguage::Spanish => "Limpiar próximas",
            })
            .build();
        clear_upcoming.add_css_class("flat");
        clear_upcoming.add_css_class("circular");
        header.append(&clear_upcoming);
        content.append(&header);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("queue-popover-list");
        list.add_css_class("queue2-list");

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_min_content_width(520);
        scroll.set_max_content_height(480);
        scroll.set_propagate_natural_height(true);
        scroll.set_child(Some(&list));
        scroll.add_css_class("queue-popover-scroll");
        content.append(&scroll);

        {
            let weak = Rc::downgrade(self);
            let list = list.clone();
            let summary = summary.clone();
            let clear_button = clear_upcoming.clone();
            let queue_popover = popover.clone();
            clear_upcoming.connect_clicked(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.playback_queue_v2.borrow_mut().clear_upcoming();
                controller.rebuild_queue_popover(&list, &summary, &clear_button, &queue_popover);
            });
        }

        self.rebuild_queue_popover(&list, &summary, &clear_upcoming, &popover);
        popover.set_child(Some(&content));
        popover.popup();
    }
}
