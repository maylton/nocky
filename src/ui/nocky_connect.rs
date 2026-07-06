//! Desktop Nocky Connect device picker UI.
//!
//! This module owns only the GTK surface and row rendering. Discovery, state and
//! handoff orchestration remain in the application controller.

use crate::connect::{
    NockyConnectDeviceDescriptor, NockyConnectDeviceList, NockyConnectDevicePlatform,
};
use gtk::prelude::*;

pub(crate) struct NockyConnectPopoverParts {
    pub(crate) popover: gtk::Popover,
    pub(crate) status: gtk::Label,
    pub(crate) device_list: gtk::Box,
    pub(crate) refresh_button: gtk::Button,
}

pub(crate) fn build_nocky_connect_popover(
    local_descriptor: Option<&NockyConnectDeviceDescriptor>,
) -> NockyConnectPopoverParts {
    let popover = gtk::Popover::new();
    popover.set_position(gtk::PositionType::Top);
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    popover.set_size_request(404, 420);
    popover.add_css_class("queue2-popover");
    popover.add_css_class("nocky-connect-popover");

    let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.add_css_class("queue2-page");
    root.add_css_class("nocky-connect-panel");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.add_css_class("queue2-page-header");

    let title_column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    title_column.set_hexpand(true);

    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    title_row.add_css_class("queue2-page-title-row");

    let icon = gtk::Image::from_icon_name("network-workgroup-symbolic");
    icon.set_pixel_size(18);
    icon.add_css_class("queue2-page-title-icon");

    let title = gtk::Label::new(Some("Nocky Connect"));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    title.add_css_class("queue2-page-title");

    title_row.append(&icon);
    title_row.append(&title);

    let subtitle = gtk::Label::new(Some("Choose where this session should be available."));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("queue2-page-source");

    title_column.append(&title_row);
    title_column.append(&subtitle);
    header.append(&title_column);
    root.append(&header);

    root.append(&build_section_header(
        "This device",
        "computer-symbolic",
        "Available for Nocky Connect.",
    ));
    root.append(&build_this_device_row(local_descriptor));

    root.append(&build_section_header(
        "Available devices",
        "view-list-symbolic",
        "Nearby devices found on your local network.",
    ));

    let status = gtk::Label::new(Some("Scanning for nearby Nocky devices…"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    status.add_css_class("queue2-page-source");
    root.append(&status);

    let device_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    device_list.add_css_class("queue2-list");
    device_list.add_css_class("nocky-connect-device-list");

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_min_content_height(108);
    scroll.set_max_content_height(170);
    scroll.set_hexpand(true);
    scroll.set_child(Some(&device_list));
    scroll.add_css_class("queue2-page-scroll");
    root.append(&scroll);

    let refresh_button = gtk::Button::with_label("Refresh devices");
    refresh_button.add_css_class("pill");
    refresh_button.add_css_class("suggested-action");
    refresh_button.add_css_class("queue2-page-action");
    refresh_button.set_halign(gtk::Align::Fill);
    root.append(&refresh_button);

    let troubleshooting = gtk::Label::new(Some(
        "No devices? Keep both apps open, use the same network, and allow UDP 34987 in the desktop firewall.",
    ));
    troubleshooting.set_xalign(0.0);
    troubleshooting.set_wrap(true);
    troubleshooting.add_css_class("dim-label");
    troubleshooting.add_css_class("queue2-state-description");
    root.append(&troubleshooting);

    popover.set_child(Some(&root));

    NockyConnectPopoverParts {
        popover,
        status,
        device_list,
        refresh_button,
    }
}

pub(crate) fn render_nocky_connect_devices(
    list: &gtk::Box,
    device_list: &NockyConnectDeviceList,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let entries = device_list.entries();
    if entries.is_empty() {
        list.append(&build_empty_device_state());
        return;
    }

    for entry in entries {
        list.append(&build_device_button(&entry.descriptor, &entry.address.to_string()));
    }
}

fn build_section_header(title: &str, icon_name: &str, subtitle: &str) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.add_css_class("queue2-section-header");

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(15);
    icon.add_css_class("queue2-section-icon");

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
    labels.set_hexpand(true);

    let title = gtk::Label::new(Some(title));
    title.set_xalign(0.0);
    title.add_css_class("queue2-section-title");

    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("queue2-state-description");

    labels.append(&title);
    labels.append(&subtitle);
    header.append(&icon);
    header.append(&labels);
    header
}

fn build_this_device_row(descriptor: Option<&NockyConnectDeviceDescriptor>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("queue2-row");
    row.add_css_class("active");
    row.set_margin_top(4);
    row.set_margin_bottom(4);
    row.set_margin_start(4);
    row.set_margin_end(4);

    let icon = gtk::Image::from_icon_name("computer-symbolic");
    icon.set_pixel_size(22);
    icon.set_valign(gtk::Align::Center);
    row.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
    labels.set_hexpand(true);

    let name = descriptor
        .map(|descriptor| descriptor.device_name.as_str())
        .unwrap_or("Nocky Desktop");
    let name_label = gtk::Label::new(Some(name));
    name_label.set_xalign(0.0);
    name_label.add_css_class("queue2-track-title");

    let detail = gtk::Label::new(Some("This desktop"));
    detail.set_xalign(0.0);
    detail.add_css_class("dim-label");
    detail.add_css_class("queue2-track-meta");

    labels.append(&name_label);
    labels.append(&detail);
    row.append(&labels);

    let check = gtk::Image::from_icon_name("object-select-symbolic");
    check.set_pixel_size(16);
    check.set_valign(gtk::Align::Center);
    row.append(&check);
    row
}

fn build_empty_device_state() -> gtk::Box {
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 7);
    empty.set_margin_top(12);
    empty.set_margin_bottom(12);
    empty.set_margin_start(12);
    empty.set_margin_end(12);
    empty.set_halign(gtk::Align::Fill);
    empty.set_valign(gtk::Align::Center);
    empty.add_css_class("queue2-state");
    empty.add_css_class("queue2-empty-state");

    let icon = gtk::Image::from_icon_name("network-workgroup-symbolic");
    icon.set_pixel_size(30);
    icon.add_css_class("queue2-state-icon");

    let title = gtk::Label::new(Some("No devices found yet"));
    title.add_css_class("queue2-state-title");

    let description = gtk::Label::new(Some(
        "Open Nocky Connect on another device or refresh discovery.",
    ));
    description.set_wrap(true);
    description.set_justify(gtk::Justification::Center);
    description.add_css_class("dim-label");
    description.add_css_class("queue2-state-description");

    empty.append(&icon);
    empty.append(&title);
    empty.append(&description);
    empty
}

fn build_device_button(descriptor: &NockyConnectDeviceDescriptor, address: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("queue2-row");
    button.add_css_class("nocky-connect-device-row");
    button.set_halign(gtk::Align::Fill);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_margin_top(4);
    row.set_margin_bottom(4);
    row.set_margin_start(4);
    row.set_margin_end(4);

    let icon = gtk::Image::from_icon_name(platform_icon_name(descriptor.platform));
    icon.set_pixel_size(22);
    icon.set_valign(gtk::Align::Center);
    row.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
    labels.set_hexpand(true);

    let title = gtk::Label::new(Some(&descriptor.device_name));
    title.set_xalign(0.0);
    title.add_css_class("queue2-track-title");

    let subtitle = gtk::Label::new(Some(&format!(
        "{} · {address} · last seen now",
        platform_label(descriptor.platform),
    )));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("queue2-track-meta");

    labels.append(&title);
    labels.append(&subtitle);
    row.append(&labels);

    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.set_pixel_size(16);
    arrow.set_valign(gtk::Align::Center);
    row.append(&arrow);

    button.set_child(Some(&row));
    button
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
