//! Lifecycle helpers for Desktop-side Nocky Connect services.
//!
//! This module keeps long-running/background service ownership out of the app
//! controller. UI code should only request that services are started and then
//! observe their results through small channels/callbacks.

use super::receive_handoff_offer_and_snapshot;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

static DESKTOP_HANDOFF_RECEIVER_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Handle returned when the Desktop handoff receiver starts successfully.
pub struct DesktopHandoffReceiver {
    pub receiver: mpsc::Receiver<Result<String, String>>,
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

/// Release the singleton receiver guard after a receiver has finished.
pub fn mark_desktop_handoff_receiver_stopped() {
    DESKTOP_HANDOFF_RECEIVER_ACTIVE.store(false, Ordering::SeqCst);
}
