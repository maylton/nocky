use crate::config::VisualTheme;
use gtk::{gdk, prelude::*};
use std::{cell::Cell, rc::Rc};

pub struct VisualThemeManager {
    _provider: gtk::CssProvider,
    current: Cell<VisualTheme>,
}

impl VisualThemeManager {
    pub fn install() -> Rc<Self> {
        let display = gdk::Display::default().expect("A display is required");
        let provider = gtk::CssProvider::new();
        provider.load_from_string(concat!(
            include_str!("../assets/themes/noctalia.css"),
            "\n",
            include_str!("../assets/themes/material-expressive.css"),
        ));
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 32,
        );

        Rc::new(Self {
            _provider: provider,
            current: Cell::new(VisualTheme::Noctalia),
        })
    }

    pub fn apply<W>(&self, root: &W, theme: VisualTheme)
    where
        W: IsA<gtk::Widget>,
    {
        root.remove_css_class("theme-noctalia");
        root.remove_css_class("theme-material-expressive");
        root.add_css_class(match theme {
            VisualTheme::Noctalia => "theme-noctalia",
            VisualTheme::MaterialExpressive => "theme-material-expressive",
        });
        self.current.set(theme);
    }
}
