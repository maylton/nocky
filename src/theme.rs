use crate::config::BlurMode;
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    env, fs,
    io::{BufRead, BufReader},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    sync::mpsc,
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
struct NoctaliaBackdrop {
    enabled: bool,
    blur_intensity: f64,
    tint_intensity: f64,
}

impl Default for NoctaliaBackdrop {
    fn default() -> Self {
        Self {
            enabled: false,
            blur_intensity: 0.5,
            tint_intensity: 0.3,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NiriBlur {
    off: bool,
    passes: f64,
    offset: f64,
    noise: f64,
    saturation: f64,
}

impl Default for NiriBlur {
    fn default() -> Self {
        Self {
            off: false,
            passes: 3.0,
            offset: 3.0,
            noise: 0.02,
            saturation: 1.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompositorKind {
    Niri,
    Hyprland,
    Other,
}

#[derive(Clone, Copy, Debug)]
struct HyprlandBlur {
    enabled: bool,
    size: f64,
    passes: f64,
    noise: f64,
    contrast: f64,
    brightness: f64,
    vibrancy: f64,
}

pub struct ThemeBridge {
    provider: gtk::CssProvider,
    blur_provider: gtk::CssProvider,
    monitor: RefCell<Option<gio::FileMonitor>>,
    noctalia_monitor: RefCell<Option<gio::FileMonitor>>,
    niri_monitors: RefCell<Vec<gio::FileMonitor>>,
    theme_path: PathBuf,
    noctalia_config_path: PathBuf,
    niri_config_path: PathBuf,
    compositor: CompositorKind,
    noctalia_enabled: Cell<bool>,
    blur_mode: Cell<BlurMode>,
    custom_blur_opacity: Cell<f64>,
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

        let noctalia_dir = glib::user_config_dir().join("noctalia");
        let noctalia_config_path = noctalia_dir.join("config.toml");
        let niri_config_path = active_niri_config_path();
        let compositor = detect_compositor();

        let bridge = Rc::new(Self {
            provider: gtk::CssProvider::new(),
            blur_provider: gtk::CssProvider::new(),
            monitor: RefCell::new(None),
            noctalia_monitor: RefCell::new(None),
            niri_monitors: RefCell::new(Vec::new()),
            theme_path,
            noctalia_config_path,
            niri_config_path,
            compositor,
            noctalia_enabled: Cell::new(true),
            blur_mode: Cell::new(BlurMode::Noctalia),
            custom_blur_opacity: Cell::new(0.74),
        });

        gtk::style_context_add_provider_for_display(
            &display,
            &bridge.provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
        // The dynamic Material palette is installed at USER + 80 and
        // paints opaque window/app-shell surfaces. Keep runtime blur above
        // it so Custom/Noctalia transparency is not overwritten later.
        gtk::style_context_add_provider_for_display(
            &display,
            &bridge.blur_provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 96,
        );

        bridge.reload();
        bridge.reload_blur();
        bridge.watch(config_dir);
        bridge.watch_noctalia(noctalia_dir);
        match bridge.compositor {
            CompositorKind::Niri => bridge.refresh_niri_monitors(),
            CompositorKind::Hyprland => bridge.watch_hyprland_reload_events(),
            CompositorKind::Other => {}
        }
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

    pub fn noctalia_shell_detected(&self) -> bool {
        let config_present = self.noctalia_config_path.is_file()
            || glib::user_config_dir().join("noctalia").is_dir()
            || glib::user_config_dir()
                .join("quickshell")
                .join("noctalia")
                .is_dir();

        if !config_present {
            return false;
        }

        Command::new("pgrep")
            .args(["-f", "[n]octalia"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    pub fn set_noctalia_enabled(&self, enabled: bool) {
        self.noctalia_enabled.set(enabled);
        self.reload();
    }

    pub fn set_blur_preferences(&self, mode: BlurMode, custom_opacity: f64) {
        self.blur_mode.set(mode);
        self.custom_blur_opacity
            .set(custom_opacity.clamp(0.45, 0.95));
        self.reload_blur();
    }

    fn reload_blur(&self) {
        let opacity = match self.blur_mode.get() {
            BlurMode::Off => None,
            BlurMode::Custom => Some(self.custom_blur_opacity.get()),
            BlurMode::Noctalia => self.synced_blur_opacity(),
        };

        let Some(surface) = opacity else {
            self.blur_provider.load_from_string(
                r#"
window.background.noctalia-window,
window.background.noctalia-window:backdrop,
window.noctalia-window,
window.noctalia-window:backdrop {
  background-color: @nm_surface;
  background-image: none;
}

window.background.noctalia-window > toastoverlay,
window.background.noctalia-window > toastoverlay:backdrop,
.app-shell,
.app-shell:backdrop {
  background-color: @nm_surface;
  background-image: none;
}

popover.queue-popover > contents,
popover.queue-popover > arrow {
  background-color: @nm_surface_alt;
  background-image: none;
}
window.background.noctalia-window.theme-material-expressive,
window.background.noctalia-window.theme-material-expressive:backdrop,
window.noctalia-window.theme-material-expressive,
window.noctalia-window.theme-material-expressive:backdrop,
window.theme-material-expressive,
window.theme-material-expressive:backdrop,
window.theme-material-expressive > toastoverlay,
window.theme-material-expressive > toastoverlay:backdrop {
  background-color: @m3_surface;
  background-image: none;
}

window.theme-material-expressive .app-shell,
window.theme-material-expressive .app-shell:backdrop {
  background-color: @m3_surface;
  background-image: none;
}
popover.queue-popover.theme-material-expressive > contents,
popover.queue-popover.theme-material-expressive > arrow {
  color: @m3_on_surface;
  background-color: @m3_surface_container_high;
  background-image: none;
  border-color: alpha(@m3_outline, 0.22);
}
dialog.settings-dialog.theme-material-expressive,
dialog.youtube-settings-dialog.theme-material-expressive,
dialog.startup-dialog.theme-material-expressive,
.settings-dialog.theme-material-expressive,
.youtube-settings-dialog.theme-material-expressive,
.startup-dialog.theme-material-expressive {
  color: @m3_on_surface;
  background-color: @m3_surface_container_low;
  background-image:
    radial-gradient(circle at 12% 0%, alpha(@m3_primary, 0.10), transparent 46%);
}

dialog.settings-dialog.theme-material-expressive .material-dialog-toolbar,
dialog.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar,
dialog.startup-dialog.theme-material-expressive .material-dialog-toolbar,
.settings-dialog.theme-material-expressive .material-dialog-toolbar,
.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar,
.startup-dialog.theme-material-expressive .material-dialog-toolbar {
  color: @m3_on_surface;
  background-color: @m3_surface_container_low;
  background-image:
    radial-gradient(circle at 12% 0%, alpha(@m3_primary, 0.08), transparent 48%);
}

dialog.settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
dialog.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
dialog.startup-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.startup-dialog.theme-material-expressive .material-dialog-toolbar headerbar {
  color: @m3_on_surface;
  background-color: @m3_surface_container;
  background-image: none;
}
dialog.settings-dialog.theme-material-expressive {
  padding: 0;
  border: none;
  box-shadow: none;
  background-color: transparent;
  background-image: none;
}

.settings-dialog-shell.theme-material-expressive {
  color: @m3_on_surface;
  background-color: @m3_surface_container;
  background-image:
    radial-gradient(circle at 10% 0%, alpha(@m3_primary, 0.11), transparent 46%);
  border: 1px solid alpha(@m3_outline, 0.22);
  border-radius: 30px;
  box-shadow: 0 18px 46px alpha(black, 0.34);
}

.settings-dialog-surface.theme-material-expressive,
.settings-dialog-shell.theme-material-expressive .material-dialog-toolbar {
  color: @m3_on_surface;
  background-color: transparent;
  background-image: none;
}

.settings-dialog-shell.theme-material-expressive headerbar {
  color: @m3_on_surface;
  background-color: @m3_surface_container_high;
  background-image: none;
  border-color: alpha(@m3_outline, 0.18);
}
"#,
            );
            return;
        };

        let header = (surface + 0.08).min(0.97);
        let card = (surface + 0.10).min(0.97);
        let panel = (surface - 0.12).clamp(0.32, 0.88);
        let footer = (surface + 0.07).min(0.97);

        self.blur_provider.load_from_string(&format!(
            r#"
window.background.noctalia-window,
window.background.noctalia-window:backdrop,
window.noctalia-window,
window.noctalia-window:backdrop,
window.background.noctalia-window > toastoverlay,
window.background.noctalia-window > toastoverlay:backdrop {{
  background-color: transparent;
  background-image: none;
}}

.app-shell,
.app-shell:backdrop {{
  background-color: alpha(@nm_surface, {surface:.3});
  background-image: none;
}}

.noctalia-header,
.noctalia-header:backdrop {{
  background-color: alpha(@nm_surface_alt, {header:.3});
}}

.sidebar,
.sidebar:backdrop {{
  background-color: alpha(@nm_surface_alt, {panel:.3});
}}

.now-playing-card,
.now-playing-card:backdrop {{
  background-color: alpha(@nm_surface_alt, {card:.3});
}}

.library-panel,
.library-panel:backdrop {{
  background-color: alpha(@nm_surface_alt, {panel:.3});
}}

.player-bar,
.player-bar:backdrop {{
  background-color: alpha(@nm_surface_alt, {footer:.3});
}}

popover.queue-popover > contents,
popover.queue-popover > arrow {{
  background-color: alpha(@nm_surface_alt, {card:.3});
  background-image: none;
}}

window.noctalia-window scrolledwindow,
window.noctalia-window scrolledwindow:backdrop,
window.noctalia-window viewport,
window.noctalia-window viewport:backdrop {{
  background-color: transparent;
  background-image: none;
}}

window.noctalia-window searchbar,
window.noctalia-window searchbar:backdrop {{
  background-color: alpha(@nm_surface_alt, {header:.3});
}}
window.background.noctalia-window.theme-material-expressive,
window.background.noctalia-window.theme-material-expressive:backdrop,
window.noctalia-window.theme-material-expressive,
window.noctalia-window.theme-material-expressive:backdrop,
window.theme-material-expressive,
window.theme-material-expressive:backdrop,
window.theme-material-expressive > toastoverlay,
window.theme-material-expressive > toastoverlay:backdrop {{
  background-color: transparent;
  background-image: none;
}}

window.theme-material-expressive .app-shell,
window.theme-material-expressive .app-shell:backdrop {{
  background-color: alpha(@m3_surface, {surface:.3});
  background-image: none;
}}

window.theme-material-expressive .expressive-header,
window.theme-material-expressive .expressive-header:backdrop,
window.theme-material-expressive .noctalia-header,
window.theme-material-expressive .noctalia-header:backdrop {{
  background-color: alpha(@m3_surface_container, {header:.3});
}}

window.theme-material-expressive .sidebar,
window.theme-material-expressive .sidebar:backdrop,
window.theme-material-expressive .search-section-card,
window.theme-material-expressive .search-section-card:backdrop {{
  background-color: alpha(@m3_surface_container_low, {panel:.3});
}}

window.theme-material-expressive .now-playing-card,
window.theme-material-expressive .now-playing-card:backdrop,
window.theme-material-expressive .expressive-player-card,
window.theme-material-expressive .expressive-player-card:backdrop,
window.theme-material-expressive .collection-card,
window.theme-material-expressive .collection-card:backdrop {{
  background-color: alpha(@m3_surface_container, {card:.3});
}}

window.theme-material-expressive .library-panel,
window.theme-material-expressive .library-panel:backdrop,
window.theme-material-expressive .home-section,
window.theme-material-expressive .home-section:backdrop,
window.theme-material-expressive .collection-page,
window.theme-material-expressive .collection-page:backdrop {{
  background-color: alpha(@m3_surface_container_low, {panel:.3});
}}

window.theme-material-expressive .player-bar,
window.theme-material-expressive .player-bar:backdrop,
window.theme-material-expressive .expressive-footer,
window.theme-material-expressive .expressive-footer:backdrop {{
  background-color: alpha(@m3_surface_container_low, {footer:.3});
}}

window.theme-material-expressive scrolledwindow,
window.theme-material-expressive scrolledwindow:backdrop,
window.theme-material-expressive viewport,
window.theme-material-expressive viewport:backdrop {{
  background-color: transparent;
  background-image: none;
}}

window.theme-material-expressive searchbar,
window.theme-material-expressive searchbar:backdrop {{
  background-color: alpha(@m3_surface_container, {header:.3});
}}
window.theme-material-expressive.material-blur-enabled,
window.theme-material-expressive.material-blur-enabled:backdrop,
window.background.noctalia-window.theme-material-expressive.material-blur-enabled,
window.background.noctalia-window.theme-material-expressive.material-blur-enabled:backdrop,
window.noctalia-window.theme-material-expressive.material-blur-enabled,
window.noctalia-window.theme-material-expressive.material-blur-enabled:backdrop,
window.theme-material-expressive.material-blur-enabled > toastoverlay,
window.theme-material-expressive.material-blur-enabled > toastoverlay:backdrop {{
  background-color: transparent;
  background-image: none;
}}

window.theme-material-expressive.material-blur-enabled .app-shell,
window.theme-material-expressive.material-blur-enabled .app-shell:backdrop {{
  background-color: rgba(17, 19, 24, {surface:.3});
  background-image: none;
}}

window.theme-material-expressive.material-blur-enabled .expressive-header,
window.theme-material-expressive.material-blur-enabled .expressive-header:backdrop {{
  background-color: rgba(29, 31, 37, {header:.3});
}}

window.theme-material-expressive.material-blur-enabled .sidebar,
window.theme-material-expressive.material-blur-enabled .sidebar:backdrop,
window.theme-material-expressive.material-blur-enabled .library-panel,
window.theme-material-expressive.material-blur-enabled .library-panel:backdrop,
window.theme-material-expressive.material-blur-enabled .home-section,
window.theme-material-expressive.material-blur-enabled .home-section:backdrop {{
  background-color: rgba(23, 25, 31, {panel:.3});
}}

window.theme-material-expressive.material-blur-enabled .now-playing-card,
window.theme-material-expressive.material-blur-enabled .now-playing-card:backdrop,
window.theme-material-expressive.material-blur-enabled .collection-card,
window.theme-material-expressive.material-blur-enabled .collection-card:backdrop {{
  background-color: rgba(29, 31, 37, {card:.3});
}}

window.theme-material-expressive.material-blur-enabled .player-bar,
window.theme-material-expressive.material-blur-enabled .player-bar:backdrop,
window.theme-material-expressive.material-blur-enabled .expressive-footer,
window.theme-material-expressive.material-blur-enabled .expressive-footer:backdrop {{
  background-color: rgba(23, 25, 31, {footer:.3});
}}

window.theme-material-expressive.material-blur-enabled scrolledwindow,
window.theme-material-expressive.material-blur-enabled viewport {{
  background-color: transparent;
  background-image: none;
}}
window.theme-material-expressive.material-blur-enabled .expressive-body,
window.theme-material-expressive.material-blur-enabled .expressive-dashboard,
window.theme-material-expressive.material-blur-enabled .navigation-rail-revealer,
window.theme-material-expressive.material-blur-enabled .expressive-search-bar {{
  background-color: transparent;
  background-image: none;
}}

popover.queue-popover.theme-material-expressive > contents,
popover.queue-popover.theme-material-expressive > arrow {{
  color: @m3_on_surface;
  background-color: alpha(@m3_surface_container_high, {card:.3});
  background-image: none;
  border-color: alpha(@m3_outline, 0.22);
}}
dialog.settings-dialog.theme-material-expressive,
dialog.youtube-settings-dialog.theme-material-expressive,
dialog.startup-dialog.theme-material-expressive,
.settings-dialog.theme-material-expressive,
.youtube-settings-dialog.theme-material-expressive,
.startup-dialog.theme-material-expressive {{
  color: @m3_on_surface;
  background-color: @m3_surface_container_low;
  background-image:
    radial-gradient(circle at 12% 0%, alpha(@m3_primary, 0.10), transparent 46%);
}}

dialog.settings-dialog.theme-material-expressive .material-dialog-toolbar,
dialog.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar,
dialog.startup-dialog.theme-material-expressive .material-dialog-toolbar,
.settings-dialog.theme-material-expressive .material-dialog-toolbar,
.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar,
.startup-dialog.theme-material-expressive .material-dialog-toolbar {{
  color: @m3_on_surface;
  background-color: @m3_surface_container_low;
  background-image:
    radial-gradient(circle at 12% 0%, alpha(@m3_primary, 0.08), transparent 48%);
}}

dialog.settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
dialog.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
dialog.startup-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.youtube-settings-dialog.theme-material-expressive .material-dialog-toolbar headerbar,
.startup-dialog.theme-material-expressive .material-dialog-toolbar headerbar {{
  color: @m3_on_surface;
  background-color: @m3_surface_container;
  background-image: none;
}}
dialog.settings-dialog.theme-material-expressive {{
  padding: 0;
  border: none;
  box-shadow: none;
  background-color: transparent;
  background-image: none;
}}

.settings-dialog-shell.theme-material-expressive {{
  color: @m3_on_surface;
  background-color: @m3_surface_container;
  background-image:
    radial-gradient(circle at 10% 0%, alpha(@m3_primary, 0.11), transparent 46%);
  border: 1px solid alpha(@m3_outline, 0.22);
  border-radius: 30px;
  box-shadow: 0 18px 46px alpha(black, 0.34);
}}

.settings-dialog-surface.theme-material-expressive,
.settings-dialog-shell.theme-material-expressive .material-dialog-toolbar {{
  color: @m3_on_surface;
  background-color: transparent;
  background-image: none;
}}

.settings-dialog-shell.theme-material-expressive headerbar {{
  color: @m3_on_surface;
  background-color: @m3_surface_container_high;
  background-image: none;
  border-color: alpha(@m3_outline, 0.18);
}}
"#
        ));
    }

    fn synced_blur_opacity(&self) -> Option<f64> {
        let noctalia = read_noctalia_backdrop(&self.noctalia_config_path);

        match self.compositor {
            CompositorKind::Niri => {
                // CachyOS commonly stores niri blur in separate rules files.
                // Follow only the include graph loaded by the active config.
                if let Some(niri) = read_niri_blur(&self.niri_config_path) {
                    if niri.off {
                        return None;
                    }

                    let kernel_strength =
                        ((niri.passes.clamp(1.0, 8.0) * niri.offset.clamp(0.25, 12.0)) / 24.0)
                            .clamp(0.10, 1.0);
                    let tint = noctalia.map_or(0.3, |value| value.tint_intensity.clamp(0.0, 1.0));
                    let saturation_lift = ((niri.saturation - 1.0) * 0.035).clamp(-0.04, 0.06);
                    let noise_lift = (niri.noise.clamp(0.0, 0.20) * 0.18).clamp(0.0, 0.04);

                    return Some(
                        (0.90 - kernel_strength * 0.30
                            + tint * 0.08
                            + saturation_lift
                            + noise_lift)
                            .clamp(0.50, 0.94),
                    );
                }
            }
            CompositorKind::Hyprland => {
                // Query the running compositor instead of parsing hyprland.conf
                // or hyprland.lua. This works with source/require layouts and
                // both the pre-0.55 and 0.55+ configuration formats.
                if let Some(hyprland) = read_hyprland_blur() {
                    if !hyprland.enabled {
                        return None;
                    }

                    let kernel_strength =
                        ((hyprland.size.clamp(1.0, 24.0) * hyprland.passes.clamp(1.0, 8.0)) / 32.0)
                            .clamp(0.10, 1.0);
                    let tint = noctalia.map_or(0.3, |value| value.tint_intensity.clamp(0.0, 1.0));
                    let color_lift = ((hyprland.vibrancy * 0.08)
                        + ((hyprland.contrast - 1.0) * 0.035)
                        + ((hyprland.brightness - 1.0) * 0.025))
                        .clamp(-0.06, 0.09);
                    let noise_lift = (hyprland.noise.clamp(0.0, 0.20) * 0.18).clamp(0.0, 0.04);

                    return Some(
                        (0.90 - kernel_strength * 0.30 + tint * 0.08 + color_lift + noise_lift)
                            .clamp(0.50, 0.94),
                    );
                }
            }
            CompositorKind::Other => {}
        }

        match noctalia {
            Some(backdrop) if backdrop.enabled && backdrop.blur_intensity > 0.0 => Some(
                (0.90 - backdrop.blur_intensity.clamp(0.0, 1.0) * 0.36
                    + backdrop.tint_intensity.clamp(0.0, 1.0) * 0.10)
                    .clamp(0.50, 0.94),
            ),
            Some(_) => None,
            None => Some(self.custom_blur_opacity.get()),
        }
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

    fn watch_noctalia(self: &Rc<Self>, directory: PathBuf) {
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

            if changed.path().as_deref() == Some(bridge.noctalia_config_path.as_path())
                && bridge.blur_mode.get() == BlurMode::Noctalia
            {
                bridge.reload_blur();
            }
        });

        self.noctalia_monitor.replace(Some(monitor));
    }

    fn watch_hyprland_reload_events(self: &Rc<Self>) {
        let Some(socket_path) = hyprland_event_socket_path() else {
            return;
        };

        let (sender, receiver) = mpsc::channel::<()>();
        thread::spawn(move || {
            let Ok(stream) = UnixStream::connect(socket_path) else {
                return;
            };
            let reader = BufReader::new(stream);
            for line in reader.lines().map_while(Result::ok) {
                if line.starts_with("configreloaded>>") && sender.send(()).is_err() {
                    break;
                }
            }
        });

        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(350), move || {
            let Some(bridge) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let mut changed = false;
            while receiver.try_recv().is_ok() {
                changed = true;
            }
            if changed && bridge.blur_mode.get() == BlurMode::Noctalia {
                bridge.reload_blur();
            }
            glib::ControlFlow::Continue
        });
    }

    fn refresh_niri_monitors(self: &Rc<Self>) {
        let (_, config_files) = expand_niri_config(&self.niri_config_path);
        let mut paths = config_files;
        if paths.is_empty() {
            paths.push(self.niri_config_path.clone());
        }

        let mut monitors = Vec::new();
        let mut watched_directories = HashSet::new();

        for path in paths {
            let file = gio::File::for_path(&path);
            if let Ok(monitor) =
                file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
            {
                connect_niri_monitor(self, &monitor);
                monitors.push(monitor);
            }

            if let Some(parent) = path.parent().map(Path::to_path_buf) {
                if watched_directories.insert(parent.clone()) {
                    let directory = gio::File::for_path(parent);
                    if let Ok(monitor) = directory
                        .monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
                    {
                        connect_niri_monitor(self, &monitor);
                        monitors.push(monitor);
                    }
                }
            }
        }

        self.niri_monitors.replace(monitors);
    }
}

fn connect_niri_monitor(bridge: &Rc<ThemeBridge>, monitor: &gio::FileMonitor) {
    let weak = Rc::downgrade(bridge);
    monitor.connect_changed(move |_, _, _, _| {
        let Some(bridge) = weak.upgrade() else {
            return;
        };
        if bridge.blur_mode.get() == BlurMode::Noctalia {
            bridge.reload_blur();
        }
        bridge.refresh_niri_monitors();
    });
}

fn detect_compositor() -> CompositorKind {
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return CompositorKind::Hyprland;
    }
    if env::var_os("NIRI_SOCKET").is_some()
        || env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .is_some_and(|value| value.to_ascii_lowercase().contains("niri"))
    {
        return CompositorKind::Niri;
    }
    CompositorKind::Other
}

fn hyprland_event_socket_path() -> Option<PathBuf> {
    let runtime = env::var_os("XDG_RUNTIME_DIR")?;
    let signature = env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    Some(
        PathBuf::from(runtime)
            .join("hypr")
            .join(signature)
            .join(".socket2.sock"),
    )
}

fn read_hyprland_blur() -> Option<HyprlandBlur> {
    Some(HyprlandBlur {
        enabled: hyprctl_bool_option("decoration.blur.enabled", "decoration:blur:enabled")?,
        size: hyprctl_number_option("decoration.blur.size", "decoration:blur:size").unwrap_or(8.0),
        passes: hyprctl_number_option("decoration.blur.passes", "decoration:blur:passes")
            .unwrap_or(1.0),
        noise: hyprctl_number_option("decoration.blur.noise", "decoration:blur:noise")
            .unwrap_or(0.0117),
        contrast: hyprctl_number_option("decoration.blur.contrast", "decoration:blur:contrast")
            .unwrap_or(0.8916),
        brightness: hyprctl_number_option(
            "decoration.blur.brightness",
            "decoration:blur:brightness",
        )
        .unwrap_or(0.8172),
        vibrancy: hyprctl_number_option("decoration.blur.vibrancy", "decoration:blur:vibrancy")
            .unwrap_or(0.1696),
    })
}

fn hyprctl_bool_option(current: &str, legacy: &str) -> Option<bool> {
    hyprctl_option_value(current)
        .or_else(|| hyprctl_option_value(legacy))
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_i64().map(|number| number != 0))
                .or_else(|| value.as_f64().map(|number| number != 0.0))
        })
}

fn hyprctl_number_option(current: &str, legacy: &str) -> Option<f64> {
    hyprctl_option_value(current)
        .or_else(|| hyprctl_option_value(legacy))
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_i64().map(|number| number as f64))
                .or_else(|| value.as_bool().map(|flag| if flag { 1.0 } else { 0.0 }))
        })
}

fn hyprctl_option_value(option: &str) -> Option<serde_json::Value> {
    let output = Command::new("hyprctl")
        .args(["-j", "getoption", option])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let object: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    for key in ["value", "int", "float", "bool"] {
        if let Some(value) = object.get(key) {
            return Some(value.clone());
        }
    }
    None
}

fn active_niri_config_path() -> PathBuf {
    if let Some(value) = env::var_os("NIRI_CONFIG").filter(|value| !value.is_empty()) {
        return expand_home(Path::new(&value));
    }

    let user = glib::user_config_dir().join("niri").join("config.kdl");
    if user.exists() {
        user
    } else {
        PathBuf::from("/etc/niri/config.kdl")
    }
}

fn expand_home(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path.to_path_buf()
}

fn normalize_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn expand_niri_config(root: &Path) -> (String, Vec<PathBuf>) {
    let mut stack = HashSet::new();
    let mut files = Vec::new();
    let source = expand_niri_file(root, &mut stack, &mut files, 0);
    (source, files)
}

fn expand_niri_file(
    path: &Path,
    stack: &mut HashSet<PathBuf>,
    files: &mut Vec<PathBuf>,
    depth: usize,
) -> String {
    if depth > 24 {
        return String::new();
    }

    let path = normalize_path(&expand_home(path));
    if stack.contains(&path) {
        return String::new();
    }
    if !files.iter().any(|candidate| candidate == &path) {
        files.push(path.clone());
    }

    let Ok(source) = fs::read_to_string(&path) else {
        return String::new();
    };

    stack.insert(path.clone());
    let mut expanded = String::new();

    for raw_line in source.lines() {
        if let Some(target) = parse_include_target(raw_line, &path) {
            expanded.push_str(&expand_niri_file(&target, stack, files, depth + 1));
        } else {
            expanded.push_str(raw_line);
            expanded.push('\n');
        }
    }

    stack.remove(&path);
    expanded
}

fn parse_include_target(line: &str, current_file: &Path) -> Option<PathBuf> {
    let clean = strip_kdl_comment(line);
    let trimmed = clean.trim_start();
    if !trimmed.starts_with("include") {
        return None;
    }

    let first_quote = trimmed.find('"')?;
    let remainder = &trimmed[first_quote + 1..];
    let second_quote = remainder.find('"')?;
    let target = expand_home(Path::new(&remainder[..second_quote]));

    if target.is_absolute() {
        Some(target)
    } else {
        Some(
            current_file
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(target),
        )
    }
}

fn read_niri_blur(root: &Path) -> Option<NiriBlur> {
    let (source, _) = expand_niri_config(root);
    if source.trim().is_empty() {
        return None;
    }

    let mut settings = NiriBlur::default();
    let mut depth = 0_i32;
    let mut blur_depth = None;

    for raw_line in source.lines() {
        let clean = strip_kdl_comment(raw_line);
        let line = clean.trim();
        if line.is_empty() {
            continue;
        }

        let delta = brace_delta(line);
        if blur_depth.is_none() && depth == 0 && is_blur_block_start(line) {
            blur_depth = Some(depth + line.matches('{').count() as i32);
            apply_niri_blur_line(&mut settings, line);
        } else if blur_depth.is_some() {
            apply_niri_blur_line(&mut settings, line);
        }

        depth += delta;
        if blur_depth.is_some_and(|start| depth < start) {
            blur_depth = None;
        }
    }

    Some(settings)
}

fn is_blur_block_start(line: &str) -> bool {
    line.strip_prefix("blur")
        .is_some_and(|rest| rest.trim_start().starts_with('{'))
}

fn apply_niri_blur_line(settings: &mut NiriBlur, line: &str) {
    let body = line.replace("blur", " ").replace(['{', '}'], " ");

    for statement in body.split(';') {
        let tokens = statement.split_whitespace().collect::<Vec<_>>();
        let Some(key) = tokens.first().copied() else {
            continue;
        };

        match key {
            "off" => settings.off = tokens.get(1).copied() != Some("false"),
            "passes" => {
                if let Some(value) = tokens.get(1).and_then(|value| value.parse().ok()) {
                    settings.passes = value;
                }
            }
            "offset" => {
                if let Some(value) = tokens.get(1).and_then(|value| value.parse().ok()) {
                    settings.offset = value;
                }
            }
            "noise" => {
                if let Some(value) = tokens.get(1).and_then(|value| value.parse().ok()) {
                    settings.noise = value;
                }
            }
            "saturation" => {
                if let Some(value) = tokens.get(1).and_then(|value| value.parse().ok()) {
                    settings.saturation = value;
                }
            }
            _ => {}
        }
    }
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0_i32;
    let mut quoted = false;
    let mut escaped = false;

    for character in line.chars() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            continue;
        }

        match character {
            '"' => quoted = true,
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }

    delta
}

fn strip_kdl_comment(line: &str) -> String {
    let mut quoted = false;
    let mut escaped = false;
    let mut characters = line.char_indices().peekable();

    while let Some((index, character)) = characters.next() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            continue;
        }

        if character == '"' {
            quoted = true;
            continue;
        }

        if character == '/'
            && characters
                .peek()
                .is_some_and(|(_, next_character)| *next_character == '/')
        {
            return line[..index].to_string();
        }
    }

    line.to_string()
}

fn read_noctalia_backdrop(path: &Path) -> Option<NoctaliaBackdrop> {
    let source = fs::read_to_string(path).ok()?;
    let mut backdrop = NoctaliaBackdrop::default();
    let mut in_backdrop = false;
    let mut found = false;

    for raw_line in source.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_backdrop = line == "[backdrop]";
            found |= in_backdrop;
            continue;
        }

        if !in_backdrop {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            "enabled" => {
                if let Ok(parsed) = value.parse::<bool>() {
                    backdrop.enabled = parsed;
                }
            }
            "blur_intensity" => {
                if let Ok(parsed) = value.parse::<f64>() {
                    backdrop.blur_intensity = parsed;
                }
            }
            "tint_intensity" => {
                if let Ok(parsed) = value.parse::<f64>() {
                    backdrop.tint_intensity = parsed;
                }
            }
            _ => {}
        }
    }

    found.then_some(backdrop)
}
