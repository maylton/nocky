//! Controller surface for the desktop Nocky Connect entry point.
//!
//! This surface is evolving toward a Spotify Connect-like device picker: it shows
//! the current device, discovers nearby Nocky devices on the LAN, and renders the
//! discovered devices as selectable rows. Actual handoff is wired in a later step.

use super::AppController;
use crate::connect::{
    default_connect_config_dir, scan_once, NockyConnectDeviceDescriptor,
    NockyConnectDeviceIdentity, NockyConnectDeviceList, NockyConnectDevicePlatform,
    NockyConnectDiscoveredDevice,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const NOCKY_CONNECT_SCAN_TIMEOUT: Duration = Duration::from_secs(6);
const NOCKY_CONNECT_DEVICE_STALE_AFTER: Duration = Duration::from_secs(30);

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

        let local_descriptor = build_local_desktop_descriptor().ok();
        let device_list = Rc::new(RefCell::new(NockyConnectDeviceList::new()));

        let surface = gtk::Window::builder()
            .title("Nocky Connect")
            .transient_for(&self.window)
            .modal(true)
            .default_width(460)
            .default_height(520)
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
            "Choose where this Nocky session should be available on your local network.",
        ));
        description.add_css_class("dim-label");
        description.set_wrap(true);
        description.set_justify(gtk::Justification::Center);
        description.set_halign(gtk::Align::Center);
        content.append(&description);

        content.append(&build_section_label("This device"));
        content.append(&build_this_device_card(local_descriptor.as_ref()));

        content.append(&build_section_label("Available on your network"));

        let status_label = gtk::Label::new(Some("Scanning for nearby Nocky devices…"));
        status_label.add_css_class("dim-label");
        status_label.set_wrap(true);
        status_label.set_xalign(0.0);
        content.append(&status_label);

        let device_list_box = gtk::ListBox::new();
        device_list_box.add_css_class("boxed-list");
        device_list_box.set_selection_mode(gtk::SelectionMode::None);
        content.append(&device_list_box);
        render_device_list(&device_list_box, &device_list.borrow());

        let refresh_button = gtk::Button::with_label("Refresh devices");
        refresh_button.add_css_class("suggested-action");
        refresh_button.set_halign(gtk::Align::Fill);
        content.append(&refresh_button);

        let troubleshooting = gtk::Label::new(Some(
            "No devices? Make sure both apps are open, on the same network, and UDP 34987 is allowed in the desktop firewall.",
        ));
        troubleshooting.add_css_class("dim-label");
        troubleshooting.set_wrap(true);
        troubleshooting.set_xalign(0.0);
        content.append(&troubleshooting);

        {
            let device_list_box = device_list_box.clone();
            let status_label = status_label.clone();
            let device_list = device_list.clone();
            refresh_button.connect_clicked(move |button| {
                start_desktop_device_scan(
                    button.clone(),
                    status_label.clone(),
                    device_list_box.clone(),
                    device_list.clone(),
                );
            });
        }

        surface.set_child(Some(&content));
        surface.present();

        start_desktop_device_scan(refresh_button, status_label, device_list_box, device_list);
    }
}

fn build_section_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("heading");
    label.set_halign(gtk::Align::Start);
    label.set_xalign(0.0);
    label
}

fn build_this_device_card(descriptor: Option<&NockyConnectDeviceDescriptor>) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("card");
    card.set_margin_top(2);
    card.set_margin_bottom(2);
    card.set_margin_start(0);
    card.set_margin_end(0);

    let icon = gtk::Image::from_icon_name("computer-symbolic");
    icon.set_pixel_size(24);
    icon.set_valign(gtk::Align::Center);
    card.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);

    let name = descriptor
        .map(|descriptor| descriptor.device_name.as_str())
        .unwrap_or("Nocky Desktop");
    let name_label = gtk::Label::new(Some(name));
    name_label.add_css_class("heading");
    name_label.set_halign(gtk::Align::Start);
    name_label.set_xalign(0.0);
    labels.append(&name_label);

    let detail = gtk::Label::new(Some("This desktop"));
    detail.add_css_class("dim-label");
    detail.set_halign(gtk::Align::Start);
    detail.set_xalign(0.0);
    labels.append(&detail);

    card.append(&labels);

    let check = gtk::Image::from_icon_name("object-select-symbolic");
    check.set_pixel_size(18);
    check.set_valign(gtk::Align::Center);
    card.append(&check);

    card
}

fn start_desktop_device_scan(
    refresh_button: gtk::Button,
    status_label: gtk::Label,
    device_list_box: gtk::ListBox,
    device_list: Rc<RefCell<NockyConnectDeviceList>>,
) {
    refresh_button.set_sensitive(false);
    status_label.set_text("Scanning for up to 6 seconds…");

    let (sender, receiver) = mpsc::channel::<Result<Vec<NockyConnectDiscoveredDevice>, String>>();
    thread::spawn(move || {
        let _ = sender.send(run_desktop_device_scan());
    });

    glib::timeout_add_local(Duration::from_millis(150), move || match receiver.try_recv() {
        Ok(Ok(devices)) => {
            let now = Instant::now();
            {
                let mut list = device_list.borrow_mut();
                list.update_with_discovered(devices, now);
                list.remove_stale(now, NOCKY_CONNECT_DEVICE_STALE_AFTER);
            }
            render_device_list(&device_list_box, &device_list.borrow());
            let count = device_list.borrow().len();
            status_label.set_text(match count {
                0 => "No devices found yet. Try again while the Android app is open.",
                1 => "1 device available.",
                _ => "Multiple devices available.",
            });
            refresh_button.set_sensitive(true);
            glib::ControlFlow::Break
        }
        Ok(Err(error)) => {
            status_label.set_text(&format!("Discovery failed: {error}"));
            refresh_button.set_sensitive(true);
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => {
            status_label.set_text("Discovery failed: worker stopped unexpectedly.");
            refresh_button.set_sensitive(true);
            glib::ControlFlow::Break
        }
    });
}

fn render_device_list(list_box: &gtk::ListBox, device_list: &NockyConnectDeviceList) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let entries = device_list.entries();
    if entries.is_empty() {
        list_box.append(&build_empty_device_row());
        return;
    }

    for entry in entries {
        list_box.append(&build_device_row(entry.descriptor.clone(), entry.address));
    }
}

fn build_empty_device_row() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);

    let label = gtk::Label::new(Some("No devices found yet"));
    label.add_css_class("dim-label");
    label.set_margin_top(14);
    label.set_margin_bottom(14);
    label.set_margin_start(14);
    label.set_margin_end(14);
    label.set_halign(gtk::Align::Start);
    label.set_xalign(0.0);
    row.set_child(Some(&label));
    row
}

fn build_device_row(
    descriptor: NockyConnectDeviceDescriptor,
    address: std::net::SocketAddr,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(true);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(14);
    content.set_margin_end(14);

    let icon = gtk::Image::from_icon_name(platform_icon_name(descriptor.platform));
    icon.set_pixel_size(24);
    icon.set_valign(gtk::Align::Center);
    content.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);

    let title = gtk::Label::new(Some(&descriptor.device_name));
    title.add_css_class("heading");
    title.set_halign(gtk::Align::Start);
    title.set_xalign(0.0);
    labels.append(&title);

    let subtitle = gtk::Label::new(Some(&format!(
        "{} · {} · last seen now",
        platform_label(descriptor.platform),
        address
    )));
    subtitle.add_css_class("dim-label");
    subtitle.set_wrap(true);
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_xalign(0.0);
    labels.append(&subtitle);

    content.append(&labels);

    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.set_pixel_size(18);
    arrow.set_valign(gtk::Align::Center);
    content.append(&arrow);

    row.set_child(Some(&content));
    row
}

fn run_desktop_device_scan() -> Result<Vec<NockyConnectDiscoveredDevice>, String> {
    let descriptor = build_local_desktop_descriptor()?;
    scan_once(&descriptor, NOCKY_CONNECT_SCAN_TIMEOUT).map_err(|error| error.to_string())
}

fn build_local_desktop_descriptor() -> Result<NockyConnectDeviceDescriptor, String> {
    let identity = NockyConnectDeviceIdentity::new(default_connect_config_dir());
    let device_id = identity.get_or_create().map_err(|error| error.to_string())?;
    Ok(NockyConnectDeviceDescriptor::linux_desktop(
        device_id,
        desktop_device_name(),
        Some(env!("CARGO_PKG_VERSION").to_string()),
    ))
}

fn platform_label(platform: NockyConnectDevicePlatform) -> &'static str {
    match platform {
        NockyConnectDevicePlatform::Android => "Android",
        NockyConnectDevicePlatform::LinuxDesktop => "Linux desktop",
        NockyConnectDevicePlatform::Unknown => "Unknown device",
    }
}

fn platform_icon_name(platform: NockyConnectDevicePlatform) -> &'static str {
    match platform {
        NockyConnectDevicePlatform::Android => "smartphone-symbolic",
        NockyConnectDevicePlatform::LinuxDesktop => "computer-symbolic",
        NockyConnectDevicePlatform::Unknown => "network-workgroup-symbolic",
    }
}

fn desktop_device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Nocky Desktop".to_string())
}
