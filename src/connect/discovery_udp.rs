use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::{
    decode_discovery_envelope, discovery_response_for_payload, encode_discovery_envelope,
    NockyConnectDeviceDescriptor, NockyConnectDiscoveryEnvelope, NockyConnectDiscoveryKind,
    NOCKY_CONNECT_DISCOVERY_PORT,
};

const DISCOVERY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NockyConnectDiscoveredDevice {
    pub descriptor: NockyConnectDeviceDescriptor,
    pub address: SocketAddr,
}

pub fn scan_once(
    local_descriptor: &NockyConnectDeviceDescriptor,
    timeout: Duration,
) -> io::Result<Vec<NockyConnectDiscoveredDevice>> {
    // Use the fixed discovery port while scanning too. Some platforms reply to
    // the sender port from the incoming broadcast; listening on the same known
    // port makes desktop scan behavior match receive mode and Android receive.
    let socket = bind_discovery_socket()?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(120)))?;

    let message_id = next_discovery_message_id("desktop-hello");
    let hello = NockyConnectDiscoveryEnvelope::hello(message_id, local_descriptor.clone());
    let payload = encode_discovery_envelope(&hello)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let broadcast = SocketAddrV4::new(Ipv4Addr::BROADCAST, NOCKY_CONNECT_DISCOVERY_PORT);
    socket.send_to(payload.as_bytes(), broadcast)?;

    collect_discovery_replies(&socket, local_descriptor, timeout)
}

pub fn receive_once(
    local_descriptor: &NockyConnectDeviceDescriptor,
    timeout: Duration,
) -> io::Result<Vec<NockyConnectDiscoveredDevice>> {
    let socket = bind_discovery_socket()?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(120)))?;

    collect_discovery_replies(&socket, local_descriptor, timeout)
}

fn bind_discovery_socket() -> io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, NOCKY_CONNECT_DISCOVERY_PORT))
}

fn collect_discovery_replies(
    socket: &UdpSocket,
    local_descriptor: &NockyConnectDeviceDescriptor,
    timeout: Duration,
) -> io::Result<Vec<NockyConnectDiscoveredDevice>> {
    let deadline = Instant::now() + timeout;
    let mut devices = HashMap::<String, NockyConnectDiscoveredDevice>::new();
    let mut buffer = vec![0_u8; DISCOVERY_BUFFER_BYTES];

    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((size, address)) => {
                let payload = match std::str::from_utf8(&buffer[..size]) {
                    Ok(payload) => payload,
                    Err(_) => continue,
                };

                if let Ok(Some(response)) = discovery_response_for_payload(
                    payload,
                    local_descriptor,
                    next_discovery_message_id("desktop-announce"),
                ) {
                    let _ = socket.send_to(response.as_bytes(), address);
                }

                let Ok(envelope) = decode_discovery_envelope(payload) else {
                    continue;
                };
                if envelope.descriptor.device_id == local_descriptor.device_id {
                    continue;
                }
                if !matches!(
                    envelope.kind,
                    NockyConnectDiscoveryKind::Hello | NockyConnectDiscoveryKind::Announce
                ) {
                    continue;
                }

                devices.insert(
                    envelope.descriptor.device_id.clone(),
                    NockyConnectDiscoveredDevice {
                        descriptor: envelope.descriptor,
                        address,
                    },
                );
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error),
        }
    }

    Ok(devices.into_values().collect())
}

fn next_discovery_message_id(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{prefix}-{millis}")
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
