use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::PathBuf,
    rc::Rc,
};

pub struct ThemeBridge {
    provider: gtk::CssProvider,
    monitor: RefCell<Option<gio::FileMonitor>>,
    theme_path: PathBuf,
    noctalia_enabled: Cell<bool>,
}

impl ThemeBridge {
    pub fn install() -> Rc<Self> {
        let display = gdk::Display::default().expect("A display is required");

        let base_provider = gtk::CssProvider::new();
        base_provider.load_from_string(include_str!("../assets/style.css"));
        gtk::style_context_add_provider_for_display(
            &display,
            &base_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let config_dir = glib::user_config_dir().join("nocky");
        let _ = fs::create_dir_all(&config_dir);
        let theme_path = config_dir.join("theme.css");
        let legacy_theme = glib::user_config_dir()
            .join("noctalia-music")
            .join("theme.css");
        if !theme_path.exists() && legacy_theme.is_file() {
            let _ = fs::copy(legacy_theme, &theme_path);
        }

        let bridge = Rc::new(Self {
            provider: gtk::CssProvider::new(),
            monitor: RefCell::new(None),
            theme_path,
            noctalia_enabled: Cell::new(true),
        });

        gtk::style_context_add_provider_for_display(
            &display,
            &bridge.provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );

        bridge.reload();
        bridge.watch(config_dir);
        bridge
    }

    fn reload(&self) {
        if !self.noctalia_enabled.get() {
            self.provider.load_from_string("");
            return;
        }
        match fs::read_to_string(&self.theme_path) {
            Ok(css) => self.provider.load_from_string(&css),
            Err(_) => self.provider.load_from_string(""),
        }
    }

    pub fn set_noctalia_enabled(&self, enabled: bool) {
        self.noctalia_enabled.set(enabled);
        self.reload();
    }

    fn watch(self: &Rc<Self>, directory: PathBuf) {
        let file = gio::File::for_path(directory);
        let Ok(monitor) =
            file.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
        else {
            return;
        };

        let weak = Rc::downgrade(self);
        monitor.connect_changed(move |_, changed, _, _| {
            let Some(bridge) = weak.upgrade() else {
                return;
            };

            if changed.path().as_deref() == Some(bridge.theme_path.as_path()) {
                bridge.reload();
            }
        });

        self.monitor.replace(Some(monitor));
    }
}
