use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::{
    decode_discovery_envelope, discovery_response_for_payload, encode_discovery_envelope,
    NockyConnectDeviceDescriptor, NockyConnectDiscoveryEnvelope, NockyConnectDiscoveryKind,
    NOCKY_CONNECT_DISCOVERY_PORT,
};

const DISCOVERY_BUFFER_BYTES: usize = 64 * 1024;
const SHARED_DISCOVERY_DEVICE_STALE_AFTER: Duration = Duration::from_secs(300);

static SHARED_DISCOVERY_DEVICES: OnceLock<Mutex<HashMap<String, SharedDiscoveryDevice>>> =
    OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NockyConnectDiscoveredDevice {
    pub descriptor: NockyConnectDeviceDescriptor,
    pub address: SocketAddr,
}

#[derive(Clone, Debug)]
struct SharedDiscoveryDevice {
    device: NockyConnectDiscoveredDevice,
    last_seen: Instant,
}

pub fn scan_once(
    local_descriptor: &NockyConnectDeviceDescriptor,
    timeout: Duration,
) -> io::Result<Vec<NockyConnectDiscoveredDevice>> {
    debug_discovery(
        "scan",
        format!(
            "starting; timeout={timeout:?}; local_device_id={}; local_name={}",
            local_descriptor.device_id, local_descriptor.device_name
        ),
    );

    let socket = bind_scan_socket()?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(120)))?;
    debug_discovery("scan", "broadcast=true; read_timeout=120ms");

    let message_id = next_discovery_message_id("desktop-hello");
    let hello = NockyConnectDiscoveryEnvelope::hello(message_id.clone(), local_descriptor.clone());
    let payload = encode_discovery_envelope(&hello)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let broadcast = SocketAddrV4::new(Ipv4Addr::BROADCAST, NOCKY_CONNECT_DISCOVERY_PORT);
    debug_discovery(
        "scan",
        format!(
            "sending hello; message_id={message_id}; bytes={}; target={broadcast}",
            payload.len()
        ),
    );
    let sent = socket.send_to(payload.as_bytes(), broadcast)?;
    debug_discovery("scan", format!("sent {sent} bytes"));

    let shared_devices = recent_shared_discovered_devices(SHARED_DISCOVERY_DEVICE_STALE_AFTER);
    if !shared_devices.is_empty() && shared_devices.iter().any(has_handoff_endpoint) {
        debug_discovery(
            "scan",
            format!(
                "using {} recently observed shared device(s)",
                shared_devices.len()
            ),
        );
        return Ok(shared_devices);
    }
    if !shared_devices.is_empty() {
        debug_discovery(
            "scan",
            format!(
                "ignoring {} endpoint-less shared device(s) while waiting for actionable discovery",
                shared_devices.len()
            ),
        );
    }

    collect_discovery_replies("scan", &socket, local_descriptor, timeout)
}

pub fn receive_once(
    local_descriptor: &NockyConnectDeviceDescriptor,
    timeout: Duration,
) -> io::Result<Vec<NockyConnectDiscoveredDevice>> {
    debug_discovery(
        "receive",
        format!(
            "starting; timeout={timeout:?}; local_device_id={}; local_name={}",
            local_descriptor.device_id, local_descriptor.device_name
        ),
    );

    let socket = bind_discovery_socket("receive", NOCKY_CONNECT_DISCOVERY_PORT)?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(120)))?;
    debug_discovery("receive", "broadcast=true; read_timeout=120ms");

    collect_discovery_replies("receive", &socket, local_descriptor, timeout)
}

fn bind_scan_socket() -> io::Result<UdpSocket> {
    bind_discovery_socket("scan", 0)
}

fn bind_discovery_socket(mode: &str, port: u16) -> io::Result<UdpSocket> {
    let address = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    debug_discovery(mode, format!("binding UDP socket on {address}"));
    match UdpSocket::bind(address) {
        Ok(socket) => {
            let local_addr = socket
                .local_addr()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            debug_discovery(mode, format!("bound UDP socket on {local_addr}"));
            Ok(socket)
        }
        Err(error) => {
            debug_discovery(mode, format!("bind failed: {error}"));
            Err(error)
        }
    }
}

fn collect_discovery_replies(
    mode: &str,
    socket: &UdpSocket,
    local_descriptor: &NockyConnectDeviceDescriptor,
    timeout: Duration,
) -> io::Result<Vec<NockyConnectDiscoveredDevice>> {
    let deadline = Instant::now() + timeout;
    let mut devices = HashMap::<String, NockyConnectDiscoveredDevice>::new();
    let mut buffer = vec![0_u8; DISCOVERY_BUFFER_BYTES];
    debug_discovery(mode, "collect loop started");

    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((size, address)) => {
                debug_discovery(mode, format!("packet received; bytes={size}; from={address}"));
                let payload = match std::str::from_utf8(&buffer[..size]) {
                    Ok(payload) => payload,
                    Err(error) => {
                        debug_discovery(mode, format!("packet ignored: invalid utf-8: {error}"));
                        continue;
                    }
                };

                match discovery_response_for_payload(
                    payload,
                    local_descriptor,
                    next_discovery_message_id("desktop-announce"),
                ) {
                    Ok(Some(response)) => match socket.send_to(response.as_bytes(), address) {
                        Ok(sent) => debug_discovery(
                            mode,
                            format!("sent announce response; bytes={sent}; target={address}"),
                        ),
                        Err(error) => debug_discovery(
                            mode,
                            format!("failed to send announce response to {address}: {error}"),
                        ),
                    },
                    Ok(None) => debug_discovery(mode, "no announce response needed for packet"),
                    Err(error) => {
                        debug_discovery(mode, format!("response helper rejected packet: {error}"))
                    }
                }

                let envelope = match decode_discovery_envelope(payload) {
                    Ok(envelope) => envelope,
                    Err(error) => {
                        debug_discovery(mode, format!("packet ignored: decode failed: {error}"));
                        continue;
                    }
                };
                debug_discovery(
                    mode,
                    format!(
                        "decoded packet; kind={:?}; remote_device_id={}; remote_name={}; remote_platform={:?}",
                        envelope.kind,
                        envelope.descriptor.device_id,
                        envelope.descriptor.device_name,
                        envelope.descriptor.platform
                    ),
                );

                if envelope.descriptor.device_id == local_descriptor.device_id {
                    debug_discovery(mode, "packet ignored: same local device_id");
                    continue;
                }
                if !matches!(
                    envelope.kind,
                    NockyConnectDiscoveryKind::Hello | NockyConnectDiscoveryKind::Announce
                ) {
                    debug_discovery(mode, "packet ignored: unsupported discovery kind");
                    continue;
                }

                let device_id = envelope.descriptor.device_id.clone();
                let device = NockyConnectDiscoveredDevice {
                    descriptor: envelope.descriptor,
                    address,
                };
                record_shared_discovered_device(device_id.clone(), device.clone());
                devices.insert(device_id.clone(), device);
                debug_discovery(
                    mode,
                    format!("device recorded; device_id={device_id}; total={}", devices.len()),
                );
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                merge_shared_discovered_devices(&mut devices);
                if !devices.is_empty() && devices.values().any(has_handoff_endpoint) {
                    debug_discovery(
                        mode,
                        format!(
                            "returning early with {} shared/observed actionable device(s)",
                            devices.len()
                        ),
                    );
                    return Ok(devices.into_values().collect());
                }
            }
            Err(error) => {
                debug_discovery(mode, format!("recv failed: {error}"));
                return Err(error);
            }
        }
    }

    merge_shared_discovered_devices(&mut devices);
    debug_discovery(mode, format!("collect loop finished; found={}", devices.len()));
    Ok(devices.into_values().collect())
}

fn shared_discovery_devices() -> &'static Mutex<HashMap<String, SharedDiscoveryDevice>> {
    SHARED_DISCOVERY_DEVICES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn record_shared_discovered_device(device_id: String, device: NockyConnectDiscoveredDevice) {
    if let Ok(mut shared) = shared_discovery_devices().lock() {
        shared.insert(
            device_id,
            SharedDiscoveryDevice {
                device,
                last_seen: Instant::now(),
            },
        );
    }
}

pub fn recent_shared_discovered_devices(max_age: Duration) -> Vec<NockyConnectDiscoveredDevice> {
    let now = Instant::now();
    let Ok(mut shared) = shared_discovery_devices().lock() else {
        return Vec::new();
    };

    shared.retain(|_, entry| match now.checked_duration_since(entry.last_seen) {
        Some(age) => age <= max_age,
        None => true,
    });

    shared
        .values()
        .map(|entry| entry.device.clone())
        .collect()
}

fn merge_shared_discovered_devices(devices: &mut HashMap<String, NockyConnectDiscoveredDevice>) {
    for device in recent_shared_discovered_devices(SHARED_DISCOVERY_DEVICE_STALE_AFTER) {
        devices
            .entry(device.descriptor.device_id.clone())
            .or_insert(device);
    }
}

fn has_handoff_endpoint(device: &NockyConnectDiscoveredDevice) -> bool {
    device.descriptor.handoff_endpoint.is_some()
}

fn next_discovery_message_id(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{prefix}-{millis}")
}

fn debug_discovery(mode: &str, message: impl AsRef<str>) {
    eprintln!("[Nocky Connect][desktop][{mode}] {}", message.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_ids_include_prefix() {
        let message_id = next_discovery_message_id("desktop-hello");

        assert!(message_id.starts_with("desktop-hello-"));
    }
}
