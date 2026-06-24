// material_dynamic_palette_v1
use crate::{config::VisualTheme, material_palette::MaterialPalette, theme_css};
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

pub struct VisualThemeManager {
    _provider: gtk::CssProvider,
    palette_provider: gtk::CssProvider,
    current: Cell<VisualTheme>,
    artwork: RefCell<Option<PathBuf>>,
    palette_tx: mpsc::Sender<(u64, MaterialPalette)>,
    generation: Arc<AtomicU64>,
}

impl VisualThemeManager {
    pub fn install() -> Rc<Self> {
        let display = gdk::Display::default().expect("A display is required");
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&theme_css::combined_theme_css());
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 32,
        );

        let palette_provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &display,
            &palette_provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 80,
        );

        let (palette_tx, palette_rx) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));

        let manager = Rc::new(Self {
            _provider: provider,
            palette_provider,
            current: Cell::new(VisualTheme::Noctalia),
            artwork: RefCell::new(None),
            palette_tx,
            generation,
        });
        manager.apply_palette(MaterialPalette::fallback());

        let weak = Rc::downgrade(&manager);
        glib::timeout_add_local(Duration::from_millis(80), move || {
            let Some(manager) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            while let Ok((generation, palette)) = palette_rx.try_recv() {
                let latest = manager.generation.load(Ordering::Acquire);
                if generation == latest && manager.current.get() == VisualTheme::MaterialExpressive
                {
                    manager.apply_palette(palette);
                }
            }

            glib::ControlFlow::Continue
        });

        manager
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

        if theme == VisualTheme::MaterialExpressive {
            self.request_palette();
        } else {
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub fn update_artwork(&self, path: Option<&Path>) {
        self.artwork.replace(path.map(Path::to_path_buf));

        if self.current.get() == VisualTheme::MaterialExpressive {
            self.request_palette();
        }
    }

    fn request_palette(&self) {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let path = self.artwork.borrow().clone();
        let sender = self.palette_tx.clone();

        let Some(path) = path else {
            let _ = sender.send((generation, MaterialPalette::fallback()));
            return;
        };

        thread::spawn(move || {
            let palette =
                MaterialPalette::from_cover(&path).unwrap_or_else(MaterialPalette::fallback);
            let _ = sender.send((generation, palette));
        });
    }

    fn apply_palette(&self, palette: MaterialPalette) {
        self.palette_provider.load_from_string(&palette.to_css());
    }
}
