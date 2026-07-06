//! Lifecycle helpers for Desktop-side Nocky Connect services.
//!
//! This module keeps long-running/background service ownership out of the app
//! controller. UI code should only request that services are started and then
//! observe their results through small channels/callbacks.

use super::handoff_http_receiver::receive_handoff_offer_and_snapshot;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

static DESKTOP_HANDOFF_RECEIVER_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Event emitted by the long-running Desktop handoff receiver service.
pub enum DesktopHandoffReceiverEvent {
    SnapshotReceived(String),
    Stopped(String),
}

/// Handle returned when the Desktop handoff receiver starts successfully.
pub struct DesktopHandoffReceiver {
    pub receiver: mpsc::Receiver<Result<String, String>>,
}

/// Handle returned when the long-running Desktop receiver service starts.
pub struct DesktopHandoffReceiverLoop {
    pub receiver: mpsc::Receiver<DesktopHandoffReceiverEvent>,
}

/// Start the Desktop handoff receiver once.
///
/// Returns `None` if a receiver is already active. The caller remains
/// responsible for consuming the receiver channel on the UI/main thread and
/// calling `mark_desktop_handoff_receiver_stopped` when the service finishes.
pub fn try_start_desktop_handoff_receiver(
    local_device_id: String,
    timeout: Duration,
) -> Option<DesktopHandoffReceiver> {
    if DESKTOP_HANDOFF_RECEIVER_ACTIVE.swap(true, Ordering::SeqCst) {
        return None;
    }

    let (sender, receiver) = mpsc::channel::<Result<String, String>>();
    thread::spawn(move || {
        let result = receive_handoff_offer_and_snapshot(&local_device_id, timeout)
            .map(|received| received.snapshot_json)
            .map_err(|error| error.to_string());
        let _ = sender.send(result);
    });

    Some(DesktopHandoffReceiver { receiver })
}

/// Start the Desktop handoff receiver as a long-running singleton service.
///
/// The receiver waits for one handoff, emits the received snapshot, then binds
/// again and waits for the next one. This keeps service lifetime out of the app
/// controller while still allowing the UI layer to decide how to apply received
/// snapshots.
pub fn try_start_desktop_handoff_receiver_loop(
    local_device_id: String,
    timeout: Duration,
) -> Option<DesktopHandoffReceiverLoop> {
    if DESKTOP_HANDOFF_RECEIVER_ACTIVE.swap(true, Ordering::SeqCst) {
        return None;
    }

    let (sender, receiver) = mpsc::channel::<DesktopHandoffReceiverEvent>();
    thread::spawn(move || {
        loop {
            match receive_handoff_offer_and_snapshot(&local_device_id, timeout) {
                Ok(received) => {
                    if sender
                        .send(DesktopHandoffReceiverEvent::SnapshotReceived(received.snapshot_json))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(DesktopHandoffReceiverEvent::Stopped(error.to_string()));
                    break;
                }
            }
        }
        DESKTOP_HANDOFF_RECEIVER_ACTIVE.store(false, Ordering::SeqCst);
    });

    Some(DesktopHandoffReceiverLoop { receiver })
}

/// Release the singleton receiver guard after a one-shot receiver has finished.
pub fn mark_desktop_handoff_receiver_stopped() {
    DESKTOP_HANDOFF_RECEIVER_ACTIVE.store(false, Ordering::SeqCst);
}
