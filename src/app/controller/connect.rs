//! Controller surface for the desktop Nocky Connect entry point.
//!
//! The visual surface is owned by `ui::nocky_connect`; this controller only
//! coordinates persistence, LAN discovery and future handoff actions.

use super::AppController;
use crate::{
    app::state::PlaybackSource,
    connect::{
        default_connect_config_dir, receive_handoff_offer_and_snapshot, resolve_handoff_target,
        scan_once, send_handoff_offer_http, send_handoff_snapshot_http, DesktopPlaybackState,
        NockyConnectDeviceDescriptor, NockyConnectDeviceIdentity, NockyConnectDeviceList,
        NockyConnectDiscoveredDevice, NockyConnectGateway, NockyConnectHandoffEndpoint,
        NockyConnectHandoffEnvelope, NockyConnectHandoffKind, NockyConnectHandoffOffer,
        NockyConnectHandoffPayload, NockyConnectRestorePolicy, NockyConnectSnapshotSummary,
        NockyConnectSource, NockyPlaybackState, NockyRepeatMode, RestoredDesktopSnapshot,
        NOCKY_CONNECT_DESKTOP_HANDOFF_PORT,
    },
    playback::queue::QueueSourceKind,
    ui::nocky_connect::{
        build_nocky_connect_popover, render_nocky_connect_devices, NockyConnectDeviceSelected,
    },
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const NOCKY_CONNECT_SCAN_TIMEOUT: Duration = Duration::from_secs(6);
const NOCKY_CONNECT_DEVICE_STALE_AFTER: Duration = Duration::from_secs(30);
const NOCKY_CONNECT_HANDOFF_HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const NOCKY_CONNECT_HANDOFF_RECEIVE_TIMEOUT: Duration = Duration::from_secs(45);

static DESKTOP_HANDOFF_RECEIVER_ACTIVE: AtomicBool = AtomicBool::new(false);

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
        start_desktop_handoff_receive(Rc::downgrade(self));

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
        Rc::new(move |descriptor, address| {
            let Some(controller) = weak.upgrade() else {
                return;
            };

            let envelope = match controller.build_handoff_offer(&descriptor) {
                Ok(envelope) => envelope,
                Err(error) => {
                    controller.show_toast(&format!("Could not prepare handoff offer: {error}"));
                    return;
                }
            };
            let snapshot_json = match controller.build_handoff_snapshot_json() {
                Ok(snapshot_json) => snapshot_json,
                Err(error) => {
                    controller.show_toast(&format!("Could not prepare handoff snapshot: {error}"));
                    return;
                }
            };
            let summary = handoff_offer_summary(&envelope);
            let encoded_bytes = snapshot_json.len();
            let target = match resolve_handoff_target(&descriptor, address) {
                Ok(target) => target,
                Err(_) => {
                    controller.show_toast(&format!(
                        "Handoff snapshot ready for {} · {summary} · {encoded_bytes} bytes · receiver endpoint pending",
                        descriptor.device_name
                    ));
                    popover.popdown();
                    return;
                }
            };

            controller.show_toast(&format!(
                "Sending handoff snapshot to {} · {summary} · {encoded_bytes} bytes",
                descriptor.device_name
            ));
            popover.popdown();
            start_desktop_handoff_send(
                weak.clone(),
                descriptor.device_name.clone(),
                target,
                envelope,
                snapshot_json,
            );
        })
    }

    fn build_handoff_offer(
        &self,
        receiver: &NockyConnectDeviceDescriptor,
    ) -> Result<NockyConnectHandoffEnvelope, String> {
        self.persist_playback_session_now();

        let sender = build_local_desktop_descriptor()?;
        let queue = self.playback_queue_v2.borrow();
        let current = queue
            .current()
            .ok_or_else(|| "current queue is empty".to_string())?;
        let source = queue
            .source_kind()
            .map_err(|error| error.to_string())?
            .map(connect_source_from_queue_source_kind)
            .unwrap_or(NockyConnectSource::Unknown);
        let position_ms = self.player.position_us().max(0) as u64 / 1_000;
        let duration_ms = duration_ms(current.media.duration_seconds).or_else(|| {
            let player_duration = self.player.duration_us();
            (player_duration > 0).then_some(player_duration as u64 / 1_000)
        });
        let current_artist =
            (!current.media.artist.trim().is_empty()).then(|| current.media.artist.clone());
        let created_at_epoch_ms = unix_millis();
        let offer_id = format!("desktop-offer-{created_at_epoch_ms}");

        Ok(NockyConnectHandoffEnvelope::offer(
            format!("desktop-offer-message-{created_at_epoch_ms}"),
            created_at_epoch_ms,
            NockyConnectHandoffOffer {
                offer_id,
                sender_device_id: sender.device_id,
                sender_device_name: sender.device_name,
                receiver_device_id: receiver.device_id.clone(),
                snapshot_summary: NockyConnectSnapshotSummary {
                    source,
                    current_title: Some(current.media.title.clone()),
                    current_artist,
                    queue_items: queue.len(),
                    position_ms,
                    duration_ms,
                    was_playing: self.player.is_playing(),
                },
                restore_policy: NockyConnectRestorePolicy::RestorePaused,
            },
        ))
    }

    fn build_handoff_snapshot_json(&self) -> Result<String, String> {
        self.persist_playback_session_now();

        let sender = build_local_desktop_descriptor()?;
        let queue = self.playback_queue_v2.borrow();
        let current = queue
            .current()
            .ok_or_else(|| "current queue is empty".to_string())?;
        let position_ms = self.player.position_us().max(0) as u64 / 1_000;
        let duration_ms = duration_ms(current.media.duration_seconds).or_else(|| {
            let player_duration = self.player.duration_us();
            (player_duration > 0).then_some(player_duration as u64 / 1_000)
        });
        let title = self.listening_history_context.borrow().title.clone();
        let title = (!title.trim().is_empty()).then_some(title);
        let now = unix_millis();
        let playback_state = DesktopPlaybackState {
            state: if self.player.is_playing() {
                NockyPlaybackState::Playing
            } else {
                NockyPlaybackState::Paused
            },
            position_ms,
            duration_ms,
            repeat_mode: if self.repeat_button.is_active() {
                NockyRepeatMode::All
            } else {
                NockyRepeatMode::Off
            },
            shuffle_enabled: self.shuffle_enabled.get(),
            ..Default::default()
        };

        NockyConnectGateway::new(sender.device_id)
            .export_snapshot_json(
                &queue,
                title,
                playback_state,
                format!("desktop-session-{now}"),
                1,
            )
            .map_err(|error| error.to_string())
    }

    fn apply_received_handoff_snapshot(&self, payload: &str) -> Result<String, String> {
        let receiver = build_local_desktop_descriptor()?;
        let restored = NockyConnectGateway::new(receiver.device_id)
            .prepare_restore(payload)
            .map_err(|error| error.to_string())?;
        self.apply_restored_desktop_snapshot(restored)
    }

    fn apply_restored_desktop_snapshot(
        &self,
        restored: RestoredDesktopSnapshot,
    ) -> Result<String, String> {
        if restored.queue.is_empty() {
            return Err("received queue is empty".to_string());
        }
        let source = restored
            .queue
            .source_kind()
            .map_err(|error| error.to_string())?
            .unwrap_or(QueueSourceKind::YouTube);
        let current_id = restored
            .queue
            .current_id()
            .or_else(|| restored.queue.entries().first().map(|entry| entry.id))
            .ok_or_else(|| "received queue has no current item".to_string())?;
        let position_us = restored
            .state
            .position_ms
            .saturating_mul(1_000)
            .min(i64::MAX as u64) as i64;
        let item_count = restored.queue.len();
        let current_title = restored
            .queue
            .current()
            .map(|entry| entry.media.title.clone())
            .or_else(|| restored.title.clone())
            .unwrap_or_else(|| "queue".to_string());
        let snapshot = restored.queue.snapshot();

        self.maybe_record_listening();
        let _ = self.player.pause();
        self.update_play_icons(false);
        self.playback_source.set(PlaybackSource::None);
        self.state.borrow_mut().current = None;
        self.youtube_state.borrow_mut().take();
        self.queue_v2_pending_entry.set(None);
        self.queue_dragged_entry.set(None);
        self.active_queue_source.set(source);
        self.playback_queue_v2.replace(restored.queue);
        self.queue_last_saved_snapshot.replace(snapshot);

        let shuffle_enabled = restored.state.shuffle_enabled;
        self.shuffle_enabled.set(shuffle_enabled);
        self.shuffle_button.set_active(shuffle_enabled);
        self.footer_shuffle_button.set_active(shuffle_enabled);
        let repeat_enabled = restored.state.repeat_mode != NockyRepeatMode::Off;
        self.repeat_button.set_active(repeat_enabled);
        self.footer_repeat_button.set_active(repeat_enabled);
        self.shuffle_navigation.borrow_mut().clear();
        self.reset_shuffle_navigation(shuffle_enabled);
        self.pending_resume_position_us.set(Some(position_us));
        self.startup_restore_autoplay.set(Some(false));
        self.playback_session_restore_attempts.set(0);

        self.play_queue_entry(current_id, false);
        let _ = self.player.pause();
        self.update_play_icons(false);
        self.persist_queue_now();
        self.persist_playback_session_now();
        self.publish_mpris_capabilities();
        self.update_footer_source();

        Ok(format!("restored paused · {current_title} · {item_count} items"))
    }
}

fn start_desktop_handoff_send(
    weak: std::rc::Weak<AppController>,
    device_name: String,
    target: crate::connect::NockyConnectHandoffTarget,
    envelope: NockyConnectHandoffEnvelope,
    snapshot_json: String,
) {
    let (sender, receiver) = mpsc::channel::<Result<String, String>>();
    thread::spawn(move || {
        let result = send_handoff_offer_http(&target, &envelope, NOCKY_CONNECT_HANDOFF_HTTP_TIMEOUT)
            .map_err(|error| error.to_string())
            .and_then(|response| match response.kind {
                NockyConnectHandoffKind::Accept => send_handoff_snapshot_http(
                    &target,
                    &snapshot_json,
                    NOCKY_CONNECT_HANDOFF_HTTP_TIMEOUT,
                )
                .map(|_| "accepted and snapshot delivered".to_string())
                .map_err(|error| error.to_string()),
                NockyConnectHandoffKind::Decline => Ok("declined handoff".to_string()),
                _ => Ok("responded to handoff".to_string()),
            });
        let _ = sender.send(result);
    });

    glib::timeout_add_local(Duration::from_millis(120), move || match receiver.try_recv() {
        Ok(Ok(detail)) => {
            if let Some(controller) = weak.upgrade() {
                controller.show_toast(&format!("Nocky Connect: {device_name} {detail}"));
            }
            glib::ControlFlow::Break
        }
        Ok(Err(error)) => {
            if let Some(controller) = weak.upgrade() {
                controller.show_toast(&format!(
                    "Nocky Connect: could not send snapshot to {device_name}: {error}"
                ));
            }
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => {
            if let Some(controller) = weak.upgrade() {
                controller.show_toast("Nocky Connect: handoff sender stopped unexpectedly");
            }
            glib::ControlFlow::Break
        }
    });
}

fn start_desktop_handoff_receive(weak: std::rc::Weak<AppController>) {
    if DESKTOP_HANDOFF_RECEIVER_ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }

    let local_device_id = match build_local_desktop_descriptor() {
        Ok(descriptor) => descriptor.device_id,
        Err(error) => {
            DESKTOP_HANDOFF_RECEIVER_ACTIVE.store(false, Ordering::SeqCst);
            if let Some(controller) = weak.upgrade() {
                controller.show_toast(&format!("Nocky Connect: receiver unavailable: {error}"));
            }
            return;
        }
    };
    let (sender, receiver) = mpsc::channel::<Result<String, String>>();
    thread::spawn(move || {
        let result = receive_handoff_offer_and_snapshot(
            &local_device_id,
            NOCKY_CONNECT_HANDOFF_RECEIVE_TIMEOUT,
        )
        .map(|received| received.snapshot_json)
        .map_err(|error| error.to_string());
        let _ = sender.send(result);
    });

    glib::timeout_add_local(Duration::from_millis(150), move || match receiver.try_recv() {
        Ok(Ok(snapshot_json)) => {
            DESKTOP_HANDOFF_RECEIVER_ACTIVE.store(false, Ordering::SeqCst);
            if let Some(controller) = weak.upgrade() {
                match controller.apply_received_handoff_snapshot(&snapshot_json) {
                    Ok(detail) => controller.show_toast(&format!("Nocky Connect: {detail}")),
                    Err(error) => controller
                        .show_toast(&format!("Nocky Connect: could not restore snapshot: {error}")),
                }
            }
            glib::ControlFlow::Break
        }
        Ok(Err(error)) => {
            DESKTOP_HANDOFF_RECEIVER_ACTIVE.store(false, Ordering::SeqCst);
            if let Some(controller) = weak.upgrade() {
                controller.show_toast(&format!("Nocky Connect: receiver stopped: {error}"));
            }
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(mpsc::TryRecvError::Disconnected) => {
            DESKTOP_HANDOFF_RECEIVER_ACTIVE.store(false, Ordering::SeqCst);
            if let Some(controller) = weak.upgrade() {
                controller.show_toast("Nocky Connect: receiver stopped unexpectedly");
            }
            glib::ControlFlow::Break
        }
    });
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
    )
    .with_handoff_endpoint(NockyConnectHandoffEndpoint::local_http(
        NOCKY_CONNECT_DESKTOP_HANDOFF_PORT,
    )))
}

fn handoff_offer_summary(envelope: &NockyConnectHandoffEnvelope) -> String {
    match &envelope.payload {
        NockyConnectHandoffPayload::Offer(offer) => format!(
            "{} items · restore paused",
            offer.snapshot_summary.queue_items
        ),
        _ => "not an offer".to_string(),
    }
}

fn connect_source_from_queue_source_kind(kind: QueueSourceKind) -> NockyConnectSource {
    match kind {
        QueueSourceKind::Local => NockyConnectSource::Local,
        QueueSourceKind::YouTube => NockyConnectSource::YouTube,
    }
}

fn duration_ms(duration_seconds: u64) -> Option<u64> {
    (duration_seconds > 0).then_some(duration_seconds.saturating_mul(1_000))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
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
