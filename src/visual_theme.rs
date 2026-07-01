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

const PALETTE_TRANSITION_MS: u64 = 520;
const PALETTE_FRAME_MS: u64 = 32;
const LOADING_INDICATOR_PALETTE_CSS: &str = r#"
window.theme-material-expressive .material-loading-indicator.contained {
  color: @m3_on_primary_container;
  background-color: @m3_primary_container;
  border-radius: 999px;
}

window.theme-frosted-glass .material-loading-indicator.contained {
  color: @m3_on_primary_container;
  background-color: alpha(@m3_primary_container, 0.72);
  border-color: alpha(@m3_primary, 0.32);
}

button .material-loading-indicator.contained {
  color: inherit;
  background-color: transparent;
  border-color: transparent;
}
"#;

pub struct VisualThemeManager {
    _provider: gtk::CssProvider,
    _frosted_provider: gtk::CssProvider,
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

        // The frosted layer only paints glass highlights, borders, and depth.
        // Runtime blur remains authoritative for surface alpha at USER + 96.
        let frosted_provider = gtk::CssProvider::new();
        frosted_provider.load_from_string(theme_css::frosted_glass_css());
        gtk::style_context_add_provider_for_display(
            &display,
            &frosted_provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 104,
        );

        let (palette_tx, palette_rx) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let fallback = MaterialPalette::fallback();

        let manager = Rc::new(Self {
            _provider: provider,
            _frosted_provider: frosted_provider,
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
                if generation == latest && manager.current.get().uses_dynamic_palette() {
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
        root.remove_css_class("theme-frosted-glass");

        match theme {
            VisualTheme::Noctalia => root.add_css_class("theme-noctalia"),
            VisualTheme::MaterialExpressive => {
                root.add_css_class("theme-material-expressive");
            }
            VisualTheme::FrostedGlass => {
                // Frosted Glass intentionally reuses Material geometry, motion,
                // dynamic album colors, and then adds its own glass overlay.
                root.add_css_class("theme-material-expressive");
                root.add_css_class("theme-frosted-glass");
            }
        }
        self.current.set(theme);
        self.animations_enabled
            .set(adw::is_animations_enabled(root));

        if theme.uses_dynamic_palette() {
            self.request_palette();
        } else {
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.palette_animation_generation
                .set(self.palette_animation_generation.get().wrapping_add(1));
        }
    }

    pub fn update_artwork(&self, path: Option<&Path>) {
        self.artwork.replace(path.map(Path::to_path_buf));

        if self.current.get().uses_dynamic_palette() {
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
        let duration = Duration::from_millis(PALETTE_TRANSITION_MS);

        glib::timeout_add_local(Duration::from_millis(PALETTE_FRAME_MS), move || {
            let Some(manager) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            if manager.palette_animation_generation.get() != token
                || !manager.current.get().uses_dynamic_palette()
            {
                return glib::ControlFlow::Break;
            }

            let progress =
                (started.elapsed().as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0);
            let finished = progress >= 1.0;
            let palette = if finished {
                target
            } else {
                start.interpolate(target, emphasized_decelerate(progress))
            };

            if manager.current_palette.get() != palette {
                manager.current_palette.set(palette);
                manager.apply_palette(palette);
            }

            if finished {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn apply_palette(&self, palette: MaterialPalette) {
        let mut css = palette.to_css();
        css.push_str(LOADING_INDICATOR_PALETTE_CSS);
        self.palette_provider.load_from_string(&css);
    }
}

fn emphasized_decelerate(progress: f64) -> f64 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_motion_is_emphasized_and_bounded() {
        assert_eq!(emphasized_decelerate(0.0), 0.0);
        assert_eq!(emphasized_decelerate(1.0), 1.0);
        assert!(emphasized_decelerate(0.5) > 0.5);
    }

    #[test]
    fn dynamic_palette_keeps_loading_indicator_roles() {
        for required in [
            ".material-loading-indicator.contained",
            "@m3_on_primary_container",
            "@m3_primary_container",
            ".theme-frosted-glass",
        ] {
            assert!(LOADING_INDICATOR_PALETTE_CSS.contains(required));
        }
    }
}
