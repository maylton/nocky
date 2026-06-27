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
    time::{Duration, Instant},
};

pub struct VisualThemeManager {
    _provider: gtk::CssProvider,
    palette_provider: gtk::CssProvider,
    current: Cell<VisualTheme>,
    artwork: RefCell<Option<PathBuf>>,
    palette_tx: mpsc::Sender<(u64, MaterialPalette)>,
    generation: Arc<AtomicU64>,
    current_palette: Cell<MaterialPalette>,
    palette_animation_generation: Cell<u64>,
    animations_enabled: Cell<bool>,
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
        let fallback = MaterialPalette::fallback();

        let manager = Rc::new(Self {
            _provider: provider,
            palette_provider,
            current: Cell::new(VisualTheme::Noctalia),
            artwork: RefCell::new(None),
            palette_tx,
            generation,
            current_palette: Cell::new(fallback),
            palette_animation_generation: Cell::new(0),
            animations_enabled: Cell::new(true),
        });
        manager.apply_palette(fallback);

        let weak = Rc::downgrade(&manager);
        glib::timeout_add_local(Duration::from_millis(80), move || {
            let Some(manager) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            while let Ok((generation, palette)) = palette_rx.try_recv() {
                let latest = manager.generation.load(Ordering::Acquire);
                if generation == latest && manager.current.get() == VisualTheme::MaterialExpressive
                {
                    manager.transition_to_palette(palette);
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
        self.animations_enabled
            .set(adw::is_animations_enabled(root));

        if theme == VisualTheme::MaterialExpressive {
            self.request_palette();
        } else {
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.palette_animation_generation
                .set(self.palette_animation_generation.get().wrapping_add(1));
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
    fn transition_to_palette(self: &Rc<Self>, target: MaterialPalette) {
        let start = self.current_palette.get();
        let token = self.palette_animation_generation.get().wrapping_add(1);
        self.palette_animation_generation.set(token);

        if !self.animations_enabled.get() || start == target {
            self.current_palette.set(target);
            self.apply_palette(target);
            return;
        }

        let weak = Rc::downgrade(self);
        let started = Instant::now();
        let duration = Duration::from_millis(420);

        glib::timeout_add_local(Duration::from_millis(32), move || {
            let Some(manager) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            if manager.palette_animation_generation.get() != token
                || manager.current.get() != VisualTheme::MaterialExpressive
            {
                return glib::ControlFlow::Break;
            }

            let progress =
                (started.elapsed().as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0);
            let eased = progress * progress * (3.0 - 2.0 * progress);
            let palette = start.interpolate(target, eased);

            manager.current_palette.set(palette);
            manager.apply_palette(palette);

            if progress >= 1.0 {
                manager.current_palette.set(target);
                manager.apply_palette(target);
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn apply_palette(&self, palette: MaterialPalette) {
        self.palette_provider.load_from_string(&palette.to_css());
    }
}
