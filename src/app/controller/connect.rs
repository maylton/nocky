//! Controller surface for the desktop Nocky Connect entry point.
//!
//! LAN discovery, device picking and accept/deny confirmation will live here.
//! For now this module owns the placeholder action used by the footer button,
//! keeping the regular action table and footer UI small.

use super::AppController;
use crate::connect::{
    default_connect_config_dir, receive_once, scan_once, NockyConnectDeviceDescriptor,
    NockyConnectDeviceIdentity,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    rc::Rc,
    sync::mpsc,
    thread,
    time::Duration,
};

const NOCKY_CONNECT_SEND_TIMEOUT: Duration = Duration::from_secs(6);
const NOCKY_CONNECT_RECEIVE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
enum NockyConnectDiscoveryMode {
    Send,
    Receive,
}

impl AppController {
    pub(crate) fn install_nocky_connect_action(self: &Rc<Self>, app: &adw::Application) {
        let connect = gio::SimpleAction::new("nocky-connect", None);
        {
            let weak = Rc::downgrade(self);
            connect.connect_activate(move |_, _| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.open_nocky_connect_surface();
            });
        }
        app.add_action(&connect);
    }

    pub(crate) fn open_nocky_connect_surface(&self) {
        self.persist_playback_session_now();

        let surface = gtk::Window::builder()
            .title("Nocky Connect")
            .transient_for(&self.window)
            .modal(true)
            .default_width(420)
            .default_height(280)
            .resizable(false)
            .build();
        surface.add_css_class("nocky-connect-surface");

        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);

        let title = gtk::Label::new(Some("Nocky Connect"));
        title.add_css_class("title-1");
        title.set_halign(gtk::Align::Center);
        content.append(&title);

        let description = gtk::Label::new(Some(
            "Move the session between this desktop and Android on your local network.",
        ));
        description.add_css_class("dim-label");
        description.set_wrap(true);
        description.set_justify(gtk::Justification::Center);
        description.set_halign(gtk::Align::Center);
        content.append(&description);

        let actions = gtk::Box::new(gtk::Orientation::Vertical, 10);
        actions.set_margin_top(8);

        let send_button = build_connect_surface_action(
            "Send to Android",
            "Search for Android devices for up to 6 seconds.",
            "network-workgroup-symbolic",
        );
        let receive_button = build_connect_surface_action(
            "Receive from Android",
            "Wait 15 seconds for an Android device to start discovery.",
            "document-save-symbolic",
        );

        let toast_overlay = self.toast_overlay.clone();
        let send_surface = surface.clone();
        send_button.connect_clicked(move |_| {
            start_desktop_nocky_connect_discovery(NockyConnectDiscoveryMode::Send, toast_overlay.clone());
            send_surface.close();
        });

        let toast_overlay = self.toast_overlay.clone();
        let receive_surface = surface.clone();
        receive_button.connect_clicked(move |_| {
            start_desktop_nocky_connect_discovery(
                NockyConnectDiscoveryMode::Receive,
                toast_overlay.clone(),
            );
            receive_surface.close();
        });

        actions.append(&send_button);
        actions.append(&receive_button);
        content.append(&actions);

        surface.set_child(Some(&content));
        surface.present();
    }
}

fn build_connect_surface_action(
    title: &str,
    description: &str,
    icon_name: &str,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("nocky-connect-action");
    button.set_halign(gtk::Align::Fill);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.set_margin_top(12);
    row.set_margin_bottom(12);
    row.set_margin_start(14);
    row.set_margin_end(14);

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(24);
    icon.set_valign(gtk::Align::Center);
    row.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.set_valign(gtk::Align::Center);

    let title = gtk::Label::new(Some(title));
    title.add_css_class("heading");
    title.set_halign(gtk::Align::Start);
    title.set_xalign(0.0);
    labels.append(&title);

    let description = gtk::Label::new(Some(description));
    description.add_css_class("dim-label");
    description.set_wrap(true);
    description.set_halign(gtk::Align::Start);
    description.set_xalign(0.0);
    labels.append(&description);

    row.append(&labels);
    button.set_child(Some(&row));
    button
}

fn start_desktop_nocky_connect_discovery(
    mode: NockyConnectDiscoveryMode,
    toast_overlay: adw::ToastOverlay,
) {
    toast_overlay.add_toast(adw::Toast::new(match mode {
        NockyConnectDiscoveryMode::Send => "Nocky Connect: scanning for up to 6 seconds…",
        NockyConnectDiscoveryMode::Receive => "Nocky Connect: waiting up to 15 seconds…",
    }));

    let (sender, receiver) = mpsc::channel::<String>();
    thread::spawn(move || {
        let message = run_desktop_nocky_connect_discovery(mode);
        let _ = sender.send(message);
    });

    glib::timeout_add_local(Duration::from_millis(150), move || match receiver.try_recv() {
        Ok(message) => {
            toast_overlay.add_toast(adw::Toast::new(&message));
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });
}

fn run_desktop_nocky_connect_discovery(mode: NockyConnectDiscoveryMode) -> String {
    let identity = NockyConnectDeviceIdentity::new(default_connect_config_dir());
    let device_id = match identity.get_or_create() {
        Ok(device_id) => device_id,
        Err(error) => return format!("Nocky Connect failed: {error}"),
    };
    let descriptor = NockyConnectDeviceDescriptor::linux_desktop(
        device_id,
        desktop_device_name(),
        Some(env!("CARGO_PKG_VERSION").to_string()),
    );

    let result = match mode {
        NockyConnectDiscoveryMode::Send => scan_once(&descriptor, NOCKY_CONNECT_SEND_TIMEOUT),
        NockyConnectDiscoveryMode::Receive => {
            receive_once(&descriptor, NOCKY_CONNECT_RECEIVE_TIMEOUT)
        }
    };

    match result {
        Ok(devices) if devices.is_empty() => match mode {
            NockyConnectDiscoveryMode::Send => "Nocky Connect: no devices found".to_string(),
            NockyConnectDiscoveryMode::Receive => "Nocky Connect: no incoming device".to_string(),
        },
        Ok(devices) => {
            let names = devices
                .iter()
                .take(3)
                .map(|device| device.descriptor.device_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("Nocky Connect: {} device(s) found: {names}", devices.len())
        }
        Err(error) => format!("Nocky Connect failed: {error}"),
    }
}

fn desktop_device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Nocky Desktop".to_string())
}
