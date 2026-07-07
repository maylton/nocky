//! Desktop Nocky Connect device picker UI.
//!
//! This module owns only the GTK surface and row rendering. Discovery, state and
//! handoff orchestration remain in the application controller.

use crate::connect::{
    NockyConnectDeviceDescriptor, NockyConnectDeviceList, NockyConnectDeviceListEntry,
    NockyConnectDevicePlatform,
};
use gtk::prelude::*;
use std::{
    net::SocketAddr,
    rc::Rc,
    time::{Duration, Instant},
};

const DEVICE_AVAILABLE_NOW_WINDOW: Duration = Duration::from_secs(30);
const CONNECT_ROW_HEIGHT: i32 = 58;
const CONNECT_ROW_HORIZONTAL_PADDING: i32 = 12;
const CONNECT_ROW_VERTICAL_PADDING: i32 = 8;
const CONNECT_ROW_OUTER_GUTTER: i32 = 4;

pub(crate) type NockyConnectDeviceSelected =
    Rc<dyn Fn(NockyConnectDeviceDescriptor, SocketAddr) + 'static>;

pub(crate) struct NockyConnectPopoverParts {
    pub(crate) popover: gtk::Popover,
    pub(crate) status: gtk::Label,
    pub(crate) device_list: gtk::Box,
    pub(crate) refresh_button: gtk::Button,
    pub(crate) close_button: gtk::Button,
}

pub(crate) fn build_nocky_connect_popover(
    _local_descriptor: Option<&NockyConnectDeviceDescriptor>,
) -> NockyConnectPopoverParts {
    let popover = gtk::Popover::new();
    popover.set_position(gtk::PositionType::Top);
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    popover.set_size_request(360, 320);
    popover.add_css_class("queue2-popover");

    let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(18);
    root.set_margin_end(18);
    root.add_css_class("queue2-page");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.set_margin_bottom(4);

    let title = gtk::Label::new(Some("Connect"));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.add_css_class("title-1");
    title.add_css_class("queue2-page-title");
    header.append(&title);

    let close_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Close")
        .build();
    close_button.add_css_class("flat");
    close_button.add_css_class("circular");
    close_button.set_valign(gtk::Align::Start);
    header.append(&close_button);

    root.append(&header);

    let device_list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    device_list.add_css_class("queue2-list");
    device_list.append(&build_this_device_row());

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_min_content_height(132);
    scroll.set_max_content_height(188);
    scroll.set_hexpand(true);
    scroll.set_child(Some(&device_list));
    scroll.add_css_class("queue2-page-scroll");
    root.append(&scroll);

    let status = gtk::Label::new(Some("Same Wi-Fi network"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    status.add_css_class("queue2-page-source");

    let refresh_button = gtk::Button::with_label("Find devices");
    refresh_button.add_css_class("pill");
    refresh_button.add_css_class("queue2-page-action");
    refresh_button.set_halign(gtk::Align::Fill);
    refresh_button.set_margin_top(4);
    root.append(&refresh_button);
    root.append(&status);

    popover.set_child(Some(&root));

    NockyConnectPopoverParts {
        popover,
        status,
        device_list,
        refresh_button,
        close_button,
    }
}

pub(crate) fn render_nocky_connect_devices(
    list: &gtk::Box,
    device_list: &NockyConnectDeviceList,
    on_selected: Option<NockyConnectDeviceSelected>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    list.append(&build_this_device_row());

    let entries = device_list.entries();
    if entries.is_empty() {
        list.append(&build_empty_device_state());
        return;
    }

    let now = Instant::now();
    for entry in entries {
        list.append(&build_device_button(entry, now, on_selected.clone()));
    }
}

fn build_this_device_row() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("queue2-row");
    row.add_css_class("active");
    row.set_size_request(-1, CONNECT_ROW_HEIGHT);
    row.set_margin_top(CONNECT_ROW_OUTER_GUTTER);
    row.set_margin_bottom(CONNECT_ROW_OUTER_GUTTER);
    row.set_margin_start(CONNECT_ROW_OUTER_GUTTER);
    row.set_margin_end(CONNECT_ROW_OUTER_GUTTER);

    let icon = gtk::Image::from_icon_name("computer-symbolic");
    icon.set_pixel_size(22);
    icon.set_valign(gtk::Align::Center);
    icon.set_margin_start(CONNECT_ROW_HORIZONTAL_PADDING);
    icon.set_margin_top(CONNECT_ROW_VERTICAL_PADDING);
    icon.set_margin_bottom(CONNECT_ROW_VERTICAL_PADDING);
    row.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.set_valign(gtk::Align::Center);
    labels.set_margin_top(CONNECT_ROW_VERTICAL_PADDING);
    labels.set_margin_bottom(CONNECT_ROW_VERTICAL_PADDING);

    let name_label = gtk::Label::new(Some("This computer"));
    name_label.set_xalign(0.0);
    name_label.add_css_class("heading");

    let detail = gtk::Label::new(Some("Normal"));
    detail.set_xalign(0.0);
    detail.add_css_class("dim-label");

    labels.append(&name_label);
    labels.append(&detail);
    row.append(&labels);

    let check = gtk::Image::from_icon_name("object-select-symbolic");
    check.set_pixel_size(16);
    check.set_valign(gtk::Align::Center);
    check.set_margin_end(CONNECT_ROW_HORIZONTAL_PADDING);
    check.set_margin_top(CONNECT_ROW_VERTICAL_PADDING);
    check.set_margin_bottom(CONNECT_ROW_VERTICAL_PADDING);
    row.append(&check);
    row
}

fn build_empty_device_state() -> gtk::Box {
    let empty = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    empty.add_css_class("queue2-row");
    empty.set_size_request(-1, CONNECT_ROW_HEIGHT);
    empty.set_margin_top(CONNECT_ROW_OUTER_GUTTER);
    empty.set_margin_bottom(CONNECT_ROW_OUTER_GUTTER);
    empty.set_margin_start(CONNECT_ROW_OUTER_GUTTER);
    empty.set_margin_end(CONNECT_ROW_OUTER_GUTTER);

    let icon = gtk::Image::from_icon_name("network-workgroup-symbolic");
    icon.set_pixel_size(22);
    icon.set_valign(gtk::Align::Center);
    icon.set_margin_start(CONNECT_ROW_HORIZONTAL_PADDING);
    icon.set_margin_top(CONNECT_ROW_VERTICAL_PADDING);
    icon.set_margin_bottom(CONNECT_ROW_VERTICAL_PADDING);
    empty.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.set_valign(gtk::Align::Center);
    labels.set_margin_top(CONNECT_ROW_VERTICAL_PADDING);
    labels.set_margin_bottom(CONNECT_ROW_VERTICAL_PADDING);
    labels.set_margin_end(CONNECT_ROW_HORIZONTAL_PADDING);

    let title = gtk::Label::new(Some("No Nocky devices found"));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let detail = gtk::Label::new(Some("Open Nocky on the same Wi-Fi"));
    detail.set_xalign(0.0);
    detail.set_wrap(true);
    detail.add_css_class("dim-label");

    labels.append(&title);
    labels.append(&detail);
    empty.append(&labels);
    empty
}

fn build_device_button(
    entry: &NockyConnectDeviceListEntry,
    now: Instant,
    on_selected: Option<NockyConnectDeviceSelected>,
) -> gtk::Button {
    let descriptor = &entry.descriptor;
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("queue2-row");
    button.set_halign(gtk::Align::Fill);
    button.set_size_request(-1, CONNECT_ROW_HEIGHT);
    button.set_margin_top(CONNECT_ROW_OUTER_GUTTER);
    button.set_margin_bottom(CONNECT_ROW_OUTER_GUTTER);
    button.set_margin_start(CONNECT_ROW_OUTER_GUTTER);
    button.set_margin_end(CONNECT_ROW_OUTER_GUTTER);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.set_margin_top(CONNECT_ROW_VERTICAL_PADDING);
    row.set_margin_bottom(CONNECT_ROW_VERTICAL_PADDING);
    row.set_margin_start(CONNECT_ROW_HORIZONTAL_PADDING);
    row.set_margin_end(CONNECT_ROW_HORIZONTAL_PADDING);

    let icon = gtk::Image::from_icon_name(platform_icon_name(descriptor.platform));
    icon.set_pixel_size(22);
    icon.set_valign(gtk::Align::Center);
    row.append(&icon);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.set_valign(gtk::Align::Center);

    let title = gtk::Label::new(Some(&descriptor.device_name));
    title.set_xalign(0.0);
    title.add_css_class("heading");

    let subtitle = gtk::Label::new(Some(&device_subtitle(entry, now)));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");

    labels.append(&title);
    labels.append(&subtitle);
    row.append(&labels);

    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.set_pixel_size(16);
    arrow.set_valign(gtk::Align::Center);
    row.append(&arrow);

    button.set_child(Some(&row));

    if let Some(on_selected) = on_selected {
        let descriptor = descriptor.clone();
        let address = entry.address;
        button.connect_clicked(move |_| {
            on_selected(descriptor.clone(), address);
        });
    }

    button
}

fn device_subtitle(entry: &NockyConnectDeviceListEntry, now: Instant) -> String {
    let platform = platform_label(entry.descriptor.platform);
    let age = now
        .checked_duration_since(entry.last_seen)
        .unwrap_or_default();

    if age <= DEVICE_AVAILABLE_NOW_WINDOW {
        platform.to_string()
    } else {
        format!("{platform} · seen {} ago", relative_age(age))
    }
}

fn relative_age(age: Duration) -> String {
    let seconds = age.as_secs();
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }

    let hours = minutes / 60;
    format!("{hours}h")
}

fn platform_label(platform: NockyConnectDevicePlatform) -> &'static str {
    match platform {
        NockyConnectDevicePlatform::Android => "Nocky Connect",
        NockyConnectDevicePlatform::LinuxDesktop => "Nocky Connect",
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
