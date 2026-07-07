//! Minimal local HTTP receiver for Nocky Connect handoff transfers.
//!
//! The desktop receiver accepts the same two-step flow used by Android:
//! `handoff_offer` on `/nocky-connect/handoff`, followed by a
//! `PlaybackSessionSnapshot` POST on `/nocky-connect/snapshot`.

use std::{
    fmt,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::handoff_http::NOCKY_CONNECT_SNAPSHOT_PATH;
use super::{
    NockyConnectHandoffAccept, NockyConnectHandoffEnvelope, NockyConnectHandoffKind,
    NockyConnectHandoffOffer, NockyConnectHandoffPayload, NockyConnectHandoffResult,
    NockyConnectHandoffResultStatus, HANDOFF_MESSAGE_SCHEMA, NOCKY_CONNECT_PROTOCOL_VERSION,
    PLAYBACK_SESSION_SNAPSHOT_SCHEMA,
};

pub const NOCKY_CONNECT_DESKTOP_HANDOFF_PORT: u16 = 35_187;
pub const NOCKY_CONNECT_HANDOFF_PATH: &str = "/nocky-connect/handoff";

const REQUEST_LIMIT_BYTES: usize = 512 * 1024;
const HEADER_LIMIT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NockyConnectReceivedHandoffSnapshot {
    pub offer: NockyConnectHandoffOffer,
    pub snapshot_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NockyConnectHandoffHttpReceiverError {
    Io(String),
    InvalidRequest(String),
    InvalidJson(String),
    UnsupportedPath(String),
    UnsupportedSchema(String),
    UnsupportedSchemaVersion(u32),
    UnsupportedKind(NockyConnectHandoffKind),
}

impl fmt::Display for NockyConnectHandoffHttpReceiverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "handoff receiver I/O failed: {error}"),
            Self::InvalidRequest(error) => write!(formatter, "invalid handoff request: {error}"),
            Self::InvalidJson(error) => write!(formatter, "invalid handoff JSON: {error}"),
            Self::UnsupportedPath(path) => write!(formatter, "unsupported handoff path {path}"),
            Self::UnsupportedSchema(schema) => write!(formatter, "unsupported handoff schema {schema}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported handoff schema version {version}")
            }
            Self::UnsupportedKind(kind) => write!(formatter, "unsupported handoff kind {kind:?}"),
        }
    }
}

impl std::error::Error for NockyConnectHandoffHttpReceiverError {}

pub fn receive_handoff_offer_and_snapshot(
    local_device_id: &str,
    timeout: Duration,
) -> Result<NockyConnectReceivedHandoffSnapshot, NockyConnectHandoffHttpReceiverError> {
    let listener = TcpListener::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        NOCKY_CONNECT_DESKTOP_HANDOFF_PORT,
    ))
    .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;
    listener
        .set_nonblocking(false)
        .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;

    let (mut offer_stream, _) = listener
        .accept()
        .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;
    offer_stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;
    let offer_request = read_http_request(&mut offer_stream)?;
    if offer_request.path != NOCKY_CONNECT_HANDOFF_PATH {
        write_error_response(&mut offer_stream, 404, "Not Found")?;
        return Err(NockyConnectHandoffHttpReceiverError::UnsupportedPath(
            offer_request.path,
        ));
    }
    let offer_envelope = decode_offer_envelope(&offer_request.body)?;
    let offer = match offer_envelope.payload {
        NockyConnectHandoffPayload::Offer(offer) => offer,
        _ => {
            return Err(NockyConnectHandoffHttpReceiverError::UnsupportedKind(
                offer_envelope.kind,
            ))
        }
    };
    let accept = NockyConnectHandoffEnvelope::accept(
        format!("desktop-accept-message-{}", unix_millis()),
        unix_millis(),
        NockyConnectHandoffAccept {
            offer_id: offer.offer_id.clone(),
            receiver_device_id: local_device_id.to_string(),
        },
    );
    write_json_response(&mut offer_stream, 202, "Accepted", &accept)?;

    let (mut snapshot_stream, _) = listener
        .accept()
        .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;
    snapshot_stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;
    let snapshot_request = read_http_request(&mut snapshot_stream)?;
    if snapshot_request.path != NOCKY_CONNECT_SNAPSHOT_PATH {
        write_error_response(&mut snapshot_stream, 404, "Not Found")?;
        return Err(NockyConnectHandoffHttpReceiverError::UnsupportedPath(
            snapshot_request.path,
        ));
    }
    require_snapshot_schema(&snapshot_request.body)?;

    let result = NockyConnectHandoffEnvelope::result(
        format!("desktop-result-message-{}", unix_millis()),
        unix_millis(),
        NockyConnectHandoffResult {
            offer_id: offer.offer_id.clone(),
            status: NockyConnectHandoffResultStatus::RestoredPaused,
            error_message: None,
        },
    );
    write_json_response(&mut snapshot_stream, 202, "Accepted", &result)?;

    Ok(NockyConnectReceivedHandoffSnapshot {
        offer,
        snapshot_json: snapshot_request.body,
    })
}

struct HttpRequest {
    path: String,
    body: String,
}

fn read_http_request(
    stream: &mut TcpStream,
) -> Result<HttpRequest, NockyConnectHandoffHttpReceiverError> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > REQUEST_LIMIT_BYTES {
            return Err(NockyConnectHandoffHttpReceiverError::InvalidRequest(
                "request too large".to_string(),
            ));
        }
        if request_has_complete_body(&request)? {
            break;
        }
    }

    let request_text = std::str::from_utf8(&request)
        .map_err(|error| NockyConnectHandoffHttpReceiverError::InvalidRequest(error.to_string()))?;
    let (header_text, body) = request_text.split_once("\r\n\r\n").ok_or_else(|| {
        NockyConnectHandoffHttpReceiverError::InvalidRequest("missing header delimiter".to_string())
    })?;
    let request_line = header_text.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    if method != "POST" {
        return Err(NockyConnectHandoffHttpReceiverError::InvalidRequest(
            format!("unsupported method {method}"),
        ));
    }
    Ok(HttpRequest {
        path: path.to_string(),
        body: body.trim_end_matches('\0').to_string(),
    })
}

fn request_has_complete_body(
    request: &[u8],
) -> Result<bool, NockyConnectHandoffHttpReceiverError> {
    let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(false);
    };
    if header_end > HEADER_LIMIT_BYTES {
        return Err(NockyConnectHandoffHttpReceiverError::InvalidRequest(
            "headers too large".to_string(),
        ));
    }
    let header_text = std::str::from_utf8(&request[..header_end])
        .map_err(|error| NockyConnectHandoffHttpReceiverError::InvalidRequest(error.to_string()))?;
    let Some(content_length) = content_length(header_text) else {
        return Ok(false);
    };
    Ok(request.len().saturating_sub(header_end + 4) >= content_length)
}

fn content_length(header_text: &str) -> Option<usize> {
    header_text
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|line| line.split_once(':'))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
}

fn decode_offer_envelope(
    body: &str,
) -> Result<NockyConnectHandoffEnvelope, NockyConnectHandoffHttpReceiverError> {
    let envelope = serde_json::from_str::<NockyConnectHandoffEnvelope>(body)
        .map_err(|error| NockyConnectHandoffHttpReceiverError::InvalidJson(error.to_string()))?;
    require_handoff_envelope(&envelope, NockyConnectHandoffKind::Offer)?;
    Ok(envelope)
}

fn require_handoff_envelope(
    envelope: &NockyConnectHandoffEnvelope,
    expected_kind: NockyConnectHandoffKind,
) -> Result<(), NockyConnectHandoffHttpReceiverError> {
    if envelope.schema != HANDOFF_MESSAGE_SCHEMA {
        return Err(NockyConnectHandoffHttpReceiverError::UnsupportedSchema(
            envelope.schema.clone(),
        ));
    }
    if envelope.schema_version != NOCKY_CONNECT_PROTOCOL_VERSION {
        return Err(
            NockyConnectHandoffHttpReceiverError::UnsupportedSchemaVersion(envelope.schema_version),
        );
    }
    if envelope.kind != expected_kind {
        return Err(NockyConnectHandoffHttpReceiverError::UnsupportedKind(
            envelope.kind,
        ));
    }
    Ok(())
}

fn require_snapshot_schema(body: &str) -> Result<(), NockyConnectHandoffHttpReceiverError> {
    let value = serde_json::from_str::<serde_json::Value>(body)
        .map_err(|error| NockyConnectHandoffHttpReceiverError::InvalidJson(error.to_string()))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if schema != PLAYBACK_SESSION_SNAPSHOT_SCHEMA {
        return Err(NockyConnectHandoffHttpReceiverError::UnsupportedSchema(
            schema.to_string(),
        ));
    }
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default() as u32;
    if version != NOCKY_CONNECT_PROTOCOL_VERSION {
        return Err(NockyConnectHandoffHttpReceiverError::UnsupportedSchemaVersion(
            version,
        ));
    }
    Ok(())
}

fn write_json_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    envelope: &NockyConnectHandoffEnvelope,
) -> Result<(), NockyConnectHandoffHttpReceiverError> {
    let body = serde_json::to_string(envelope)
        .map_err(|error| NockyConnectHandoffHttpReceiverError::InvalidJson(error.to_string()))?;
    write_http_response(stream, status_code, status_text, &body)
}

fn write_error_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
) -> Result<(), NockyConnectHandoffHttpReceiverError> {
    write_http_response(stream, status_code, status_text, "{}")
}

fn write_http_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    body: &str,
) -> Result<(), NockyConnectHandoffHttpReceiverError> {
    let response = format!(
        "HTTP/1.1 {status_code} {status_text}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len(),
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| NockyConnectHandoffHttpReceiverError::Io(error.to_string()))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::{
        NockyConnectRestorePolicy, NockyConnectSnapshotSummary, NockyConnectSource, PlaybackInfo,
        PlaybackSessionSnapshot, PortableQueue,
    };

    #[test]
    fn reads_complete_http_request() {
        let request = b"POST /nocky-connect/snapshot HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";
        assert!(request_has_complete_body(request).expect("complete request"));
    }

    #[test]
    fn accepts_offer_envelope() {
        let envelope = sample_offer();
        let body = serde_json::to_string(&envelope).expect("offer json");

        let decoded = decode_offer_envelope(&body).expect("decode offer");

        assert_eq!(decoded, envelope);
    }

    #[test]
    fn validates_snapshot_schema() {
        let snapshot = PlaybackSessionSnapshot::new(
            "session-1",
            1,
            "android-device",
            1_789_000,
            NockyConnectSource::YouTube,
            PlaybackInfo::paused(0, None),
            PortableQueue {
                title: None,
                current_index: 0,
                repeat_mode: crate::connect::NockyRepeatMode::Off,
                shuffle_enabled: false,
                shuffle_seed: None,
                items: vec![],
            },
        );
        let body = serde_json::to_string(&snapshot).expect("snapshot json");

        require_snapshot_schema(&body).expect("snapshot schema");
    }

    fn sample_offer() -> NockyConnectHandoffEnvelope {
        NockyConnectHandoffEnvelope::offer(
            "offer-message-1",
            1_789_000,
            NockyConnectHandoffOffer {
                offer_id: "offer-1".to_string(),
                sender_device_id: "android-device".to_string(),
                sender_device_name: "Android".to_string(),
                receiver_device_id: "desktop-device".to_string(),
                snapshot_summary: NockyConnectSnapshotSummary {
                    source: NockyConnectSource::YouTube,
                    current_title: Some("Song".to_string()),
                    current_artist: Some("Artist".to_string()),
                    queue_items: 1,
                    position_ms: 0,
                    duration_ms: None,
                    was_playing: false,
                },
                restore_policy: NockyConnectRestorePolicy::RestorePaused,
            },
        )
    }
}
