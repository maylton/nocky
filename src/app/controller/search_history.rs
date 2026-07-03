//! Recent-search popover and local history actions.

use super::AppController;
use crate::{
    config::AppLanguage,
    ui::widgets::material_button::{
        apply_material_button, apply_material_icon_button, MaterialButtonSize, MaterialButtonSpec,
        MaterialButtonVariant, MaterialIconButtonSpec, MaterialIconButtonVariant,
    },
};
use gtk::prelude::*;
use std::rc::Rc;

#[derive(Clone, Copy)]
struct SearchHistoryCopy {
    title: &'static str,
    clear_all: &'static str,
    remove: &'static str,
}

fn search_history_copy(language: AppLanguage) -> SearchHistoryCopy {
    match language {
        AppLanguage::Portuguese => SearchHistoryCopy {
            title: "Buscas recentes",
            clear_all: "Limpar tudo",
            remove: "Remover busca recente",
        },
        AppLanguage::English => SearchHistoryCopy {
            title: "Recent searches",
            clear_all: "Clear all",
            remove: "Remove recent search",
        },
        AppLanguage::Spanish => SearchHistoryCopy {
            title: "Búsquedas recientes",
            clear_all: "Limpiar todo",
            remove: "Eliminar búsqueda reciente",
        },
    }
}

impl AppController {
    pub(crate) fn record_recent_search(self: &Rc<Self>, query: &str) {
        if self.search_history.borrow_mut().record(query) {
            self.refresh_recent_searches(false);
        }
    }

    pub(crate) fn refresh_recent_searches(self: &Rc<Self>, reveal: bool) {
        let queries = self.search_history.borrow().queries().to_vec();
        if queries.is_empty() {
            self.search_history_revealer.set_reveal_child(false);
            self.search_history_revealer.set_child(None::<&gtk::Widget>);
            return;
        }

        let copy = search_history_copy(self.config.borrow().language);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.set_halign(gtk::Align::Fill);
        root.set_hexpand(true);
        root.add_css_class("search-history-content");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("search-history-header");
        let title = gtk::Label::new(Some(copy.title));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("search-history-title");
        let clear = gtk::Button::with_label(copy.clear_all);
        apply_material_button(
            &clear,
            MaterialButtonSpec::new(MaterialButtonVariant::Text, MaterialButtonSize::Compact),
        );
        clear.add_css_class("search-history-clear");
        {
            let weak = Rc::downgrade(self);
            clear.connect_clicked(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.search_history.borrow_mut().clear();
                controller.refresh_recent_searches(false);
            });
        }
        header.append(&title);
        header.append(&clear);
        root.append(&header);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("search-history-list");

        for query in queries {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_selectable(false);
            row.add_css_class("search-history-row");

            let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let icon = gtk::Image::from_icon_name("system-search-symbolic");
            icon.set_pixel_size(16);
            icon.add_css_class("search-history-icon");

            let query_label = gtk::Label::new(Some(&query));
            query_label.set_xalign(0.0);
            query_label.set_hexpand(true);
            let query_button = gtk::Button::new();
            query_button.set_child(Some(&query_label));
            query_button.set_hexpand(true);
            query_button.set_halign(gtk::Align::Fill);
            query_button.add_css_class("search-history-query");
            apply_material_button(
                &query_button,
                MaterialButtonSpec::new(MaterialButtonVariant::Text, MaterialButtonSize::Compact),
            );
            {
                let weak = Rc::downgrade(self);
                let selected_query = query.clone();
                query_button.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    controller.record_recent_search(&selected_query);
                    controller.search_history_revealer.set_reveal_child(false);
                    controller.search_entry.set_text(&selected_query);
                    controller.search_entry.set_position(-1);
                    controller.search_entry.grab_focus();
                });
            }

            let remove = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text(copy.remove)
                .build();
            remove.add_css_class("search-history-remove");
            apply_material_icon_button(
                &remove,
                MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
            );
            {
                let weak = Rc::downgrade(self);
                let removed_query = query;
                remove.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    controller
                        .search_history
                        .borrow_mut()
                        .remove(&removed_query);
                    controller.refresh_recent_searches(true);
                });
            }

            content.append(&icon);
            content.append(&query_button);
            content.append(&remove);
            row.set_child(Some(&content));
            list.append(&row);
        }

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(280);
        scroll.set_child(Some(&list));
        scroll.add_css_class("search-history-scroll");
        root.append(&scroll);
        self.search_history_revealer.set_child(Some(&root));
        self.search_history_revealer
            .set_reveal_child(reveal && self.search_entry.text().trim().is_empty());
    }
}
