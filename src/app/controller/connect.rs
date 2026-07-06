//! Controller surface for the desktop Nocky Connect entry point.
//!
//! LAN discovery, device picking and accept/deny confirmation will live here.
//! For now this module owns the placeholder action used by the footer button,
//! keeping the regular action table and footer UI small.

use super::AppController;
use crate::connect::{
    default_connect_config_dir, scan_once, NockyConnectDeviceDescriptor,
    NockyConnectDeviceIdentity,
};
use adw::prelude::*;
use gtk::gio;
use std::{rc::Rc, time::Duration};

const NOCKY_CONNECT_DISCOVERY_TIMEOUT: Duration = Duration::from_millis(1_800);

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
            "Mova a sessão entre este desktop e o Android na sua rede local.",
        ));
        description.add_css_class("dim-label");
        description.set_wrap(true);
        description.set_justify(gtk::Justification::Center);
        description.set_halign(gtk::Align::Center);
        content.append(&description);

        let actions = gtk::Box::new(gtk::Orientation::Vertical, 10);
        actions.set_margin_top(8);

        let send_button = build_connect_surface_action(
            "Enviar para Android",
            "Exportar a fila e a posição atual para um Android confiável.",
            "network-workgroup-symbolic",
        );
        let receive_button = build_connect_surface_action(
            "Receber do Android",
            "Preparar o desktop para importar uma sessão pausada do Android.",
            "document-save-symbolic",
        );

        let toast_overlay = self.toast_overlay.clone();
        let send_surface = surface.clone();
        send_button.connect_clicked(move |_| {
            toast_overlay.add_toast(adw::Toast::new(
                "Nocky Connect: procurando dispositivos na rede local…",
            ));
            let message = run_desktop_nocky_connect_scan("Enviar para Android");
            toast_overlay.add_toast(adw::Toast::new(&message));
            send_surface.close();
        });

        let toast_overlay = self.toast_overlay.clone();
        let receive_surface = surface.clone();
        receive_button.connect_clicked(move |_| {
            toast_overlay.add_toast(adw::Toast::new(
                "Nocky Connect: procurando dispositivos na rede local…",
            ));
            let message = run_desktop_nocky_connect_scan("Receber do Android");
            toast_overlay.add_toast(adw::Toast::new(&message));
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

fn run_desktop_nocky_connect_scan(action_label: &str) -> String {
    let identity = NockyConnectDeviceIdentity::new(default_connect_config_dir());
    let device_id = match identity.get_or_create() {
        Ok(device_id) => device_id,
        Err(error) => return format!("Nocky Connect scan failed: {error}"),
    };
    let descriptor = NockyConnectDeviceDescriptor::linux_desktop(
        device_id,
        desktop_device_name(),
        Some(env!("CARGO_PKG_VERSION").to_string()),
    );

    match scan_once(&descriptor, NOCKY_CONNECT_DISCOVERY_TIMEOUT) {
        Ok(devices) if devices.is_empty() => {
            format!("Nocky Connect: nenhum dispositivo encontrado para {action_label}")
        }
        Ok(devices) => {
            let names = devices
                .iter()
                .take(3)
                .map(|device| device.descriptor.device_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Nocky Connect: {} dispositivo(s) encontrado(s): {names}",
                devices.len()
            )
        }
        Err(error) => format!("Nocky Connect scan failed: {error}"),
    }
}

fn desktop_device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Nocky Desktop".to_string())
}
