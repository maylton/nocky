//! Controller surface for the desktop Nocky Connect entry point.
//!
//! The visual surface is owned by `ui::nocky_connect`; this controller only
//! coordinates persistence, LAN discovery and future handoff actions.

use super::AppController;
use crate::{
    connect::{
        default_connect_config_dir, scan_once, NockyConnectDeviceDescriptor,
        NockyConnectDeviceIdentity, NockyConnectDeviceList, NockyConnectDiscoveredDevice,
    },
    ui::nocky_connect::{
        build_nocky_connect_popover, render_nocky_connect_devices, NockyConnectDeviceSelected,
    },
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

    pub(crate) fn open_nocky_connect_surface(self: &Rc<Self>) {
        self.persist_playback_session_now();

        let local_descriptor = build_local_desktop_descriptor().ok();
        let device_list = Rc::new(RefCell::new(NockyConnectDeviceList::new()));
        let surface = build_nocky_connect_popover(local_descriptor.as_ref());
        let on_selected = self.build_device_selected_handler(&surface.popover);

        render_nocky_connect_devices(
            &surface.device_list,
            &device_list.borrow(),
            Some(on_selected.clone()),
        );
        let anchor = self.nocky_connect_popover_anchor();
        surface.popover.set_parent(&anchor);
        {
            let popover = surface.popover.clone();
            surface.popover.connect_closed(move |_| {
                popover.unparent();
            });
        }

        {
            let popover = surface.popover.clone();
            surface.close_button.connect_clicked(move |_| {
                popover.popdown();
            });
        }

        {
            let device_list_box = surface.device_list.clone();
            let status_label = surface.status.clone();
            let device_list = device_list.clone();
            let on_selected = on_selected.clone();
            surface.refresh_button.connect_clicked(move |button| {
                start_desktop_device_scan(
                    button.clone(),
                    status_label.clone(),
                    device_list_box.clone(),
                    device_list.clone(),
                    on_selected.clone(),
                );
            });
        }

        surface.popover.popup();
        start_desktop_device_scan(
            surface.refresh_button,
            surface.status,
            surface.device_list,
            device_list,
            on_selected,
        );
    }

    fn nocky_connect_popover_anchor(&self) -> gtk::Widget {
        let root: gtk::Widget = self.footer_right_controls.clone().upcast();
        find_descendant_with_css_class(&root, "footer-connect-button").unwrap_or(root)
    }

    fn build_device_selected_handler(
        self: &Rc<Self>,
        popover: &gtk::Popover,
    ) -> NockyConnectDeviceSelected {
        let weak = Rc::downgrade(self);
        let popover = popover.clone();
        Rc::new(move |descriptor, _address| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            controller.show_toast(&format!(
                "Nocky Connect handoff for {} is next",
                descriptor.device_name
            ));
            popover.popdown();
        })
    }
}

fn start_desktop_device_scan(
    refresh_button: gtk::Button,
    status_label: gtk::Label,
    device_list_box: gtk::Box,
    device_list: Rc<RefCell<NockyConnectDeviceList>>,
    on_selected: NockyConnectDeviceSelected,
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
            render_nocky_connect_devices(
                &device_list_box,
                &device_list.borrow(),
                Some(on_selected.clone()),
            );
            let count = device_list.borrow().len();
            status_label.set_text(match count {
                0 => "No devices found yet. Try again while the Android app is open.",
                1 => "LAN discovery • 1 device available",
                _ => "LAN discovery • multiple devices available",
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

fn find_descendant_with_css_class(root: &gtk::Widget, class_name: &str) -> Option<gtk::Widget> {
    if root.has_css_class(class_name) {
        return Some(root.clone());
    }

    let mut child = root.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();
        if let Some(found) = find_descendant_with_css_class(&widget, class_name) {
            return Some(found);
        }
        child = next;
    }

    None
}

fn desktop_device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Nocky Desktop".to_string())
}
