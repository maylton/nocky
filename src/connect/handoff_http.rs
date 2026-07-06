//! Minimal local HTTP client for Nocky Connect handoff offers.
//!
//! This module intentionally uses `std::net::TcpStream` instead of adding a new
//! HTTP dependency. It sends one JSON handoff envelope and expects one JSON
//! handoff response from a LAN peer.

use std::{
    fmt,
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use super::{
    NockyConnectHandoffEnvelope, NockyConnectHandoffKind, NockyConnectHandoffTarget,
    NockyConnectHandoffTransport, HANDOFF_MESSAGE_SCHEMA, NOCKY_CONNECT_PROTOCOL_VERSION,
};

const HANDOFF_HTTP_RESPONSE_LIMIT_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NockyConnectHandoffHttpError {
    UnsupportedTransport,
    Json(String),
    Io(String),
    InvalidResponse(String),
    HttpStatus(String),
    UnsupportedSchema(String),
    UnsupportedSchemaVersion(u32),
    UnsupportedKind(NockyConnectHandoffKind),
}

impl fmt::Display for NockyConnectHandoffHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTransport => write!(formatter, "unsupported handoff HTTP transport"),
            Self::Json(error) => write!(formatter, "invalid handoff JSON: {error}"),
            Self::Io(error) => write!(formatter, "handoff HTTP I/O failed: {error}"),
            Self::InvalidResponse(error) => write!(formatter, "invalid handoff HTTP response: {error}"),
            Self::HttpStatus(status) => write!(formatter, "handoff HTTP request failed: {status}"),
            Self::UnsupportedSchema(schema) => write!(formatter, "unsupported handoff schema {schema}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported handoff schema version {version}")
            }
            Self::UnsupportedKind(kind) => write!(formatter, "unsupported handoff response kind {kind:?}"),
        }
    }
}

impl std::error::Error for NockyConnectHandoffHttpError {}

pub fn send_handoff_offer_http(
    target: &NockyConnectHandoffTarget,
    envelope: &NockyConnectHandoffEnvelope,
    timeout: Duration,
) -> Result<NockyConnectHandoffEnvelope, NockyConnectHandoffHttpError> {
    if target.transport != NockyConnectHandoffTransport::LocalHttp {
        return Err(NockyConnectHandoffHttpError::UnsupportedTransport);
    }

    let mut stream = TcpStream::connect((target.host.as_str(), target.port))
        .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;

    let request = build_handoff_offer_request(target, envelope)?;
    stream
        .write_all(&request)
        .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;

    let mut response = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;
        if read == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..read]);
        if response.len() > HANDOFF_HTTP_RESPONSE_LIMIT_BYTES {
            return Err(NockyConnectHandoffHttpError::InvalidResponse(
                "response too large".to_string(),
            ));
        }
    }

    decode_handoff_offer_response(&response)
}

pub(crate) fn build_handoff_offer_request(
    target: &NockyConnectHandoffTarget,
    envelope: &NockyConnectHandoffEnvelope,
) -> Result<Vec<u8>, NockyConnectHandoffHttpError> {
    let body = serde_json::to_string(envelope)
        .map_err(|error| NockyConnectHandoffHttpError::Json(error.to_string()))?;
    let body_bytes = body.as_bytes();
    let path = normalized_path(&target.path);
    let headers = format!(
        "POST {path} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        target.host,
        target.port,
        body_bytes.len(),
    );

    let mut request = headers.into_bytes();
    request.extend_from_slice(body_bytes);
    Ok(request)
}

pub(crate) fn decode_handoff_offer_response(
    response: &[u8],
) -> Result<NockyConnectHandoffEnvelope, NockyConnectHandoffHttpError> {
    let response_text = std::str::from_utf8(response)
        .map_err(|error| NockyConnectHandoffHttpError::InvalidResponse(error.to_string()))?;
    let (header_text, body) = response_text.split_once("\r\n\r\n").ok_or_else(|| {
        NockyConnectHandoffHttpError::InvalidResponse("missing header delimiter".to_string())
    })?;
    let status_line = header_text.lines().next().unwrap_or_default();
    if !status_line.contains(" 202 ") && !status_line.contains(" 200 ") {
        return Err(NockyConnectHandoffHttpError::HttpStatus(
            status_line.to_string(),
        ));
    }

    let envelope = serde_json::from_str::<NockyConnectHandoffEnvelope>(body)
        .map_err(|error| NockyConnectHandoffHttpError::Json(error.to_string()))?;
    require_supported_handoff_response(&envelope)?;
    Ok(envelope)
}

fn require_supported_handoff_response(
    envelope: &NockyConnectHandoffEnvelope,
) -> Result<(), NockyConnectHandoffHttpError> {
    if envelope.schema != HANDOFF_MESSAGE_SCHEMA {
        return Err(NockyConnectHandoffHttpError::UnsupportedSchema(
            envelope.schema.clone(),
        ));
    }
    if envelope.schema_version != NOCKY_CONNECT_PROTOCOL_VERSION {
        return Err(NockyConnectHandoffHttpError::UnsupportedSchemaVersion(
            envelope.schema_version,
        ));
    }
    match envelope.kind {
        NockyConnectHandoffKind::Accept | NockyConnectHandoffKind::Decline => Ok(()),
        kind => Err(NockyConnectHandoffHttpError::UnsupportedKind(kind)),
    }
}

fn normalized_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::{
        NockyConnectHandoffAccept, NockyConnectHandoffOffer, NockyConnectHandoffPayload,
        NockyConnectRestorePolicy, NockyConnectSnapshotSummary, NockyConnectSource,
    };

    #[test]
    fn builds_post_request_for_handoff_offer() {
        let target = NockyConnectHandoffTarget {
            host: "192.168.0.8".to_string(),
            port: 35187,
            path: "/nocky-connect/handoff".to_string(),
            transport: NockyConnectHandoffTransport::LocalHttp,
        };
        let envelope = sample_offer();

        let request = build_handoff_offer_request(&target, &envelope).expect("request");
        let text = String::from_utf8(request).expect("utf-8 request");

        assert!(text.starts_with("POST /nocky-connect/handoff HTTP/1.1\r\n"));
        assert!(text.contains("Host: 192.168.0.8:35187\r\n"));
        assert!(text.contains("Content-Type: application/json; charset=utf-8\r\n"));
        assert!(text.contains("\r\n\r\n"));
        assert!(text.contains("handoff_offer"));
        assert!(!text.contains("cookies"));
        assert!(!text.contains("stream_url"));
    }

    #[test]
    fn decodes_accept_response() {
        let accept = NockyConnectHandoffEnvelope::accept(
            "accept-message-1",
            1_789_001,
            NockyConnectHandoffAccept {
                offer_id: "offer-1".to_string(),
                receiver_device_id: "android-1".to_string(),
            },
        );
        let body = serde_json::to_string(&accept).expect("accept json");
        let response = format!(
            "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body,
        );

        let decoded = decode_handoff_offer_response(response.as_bytes()).expect("decode accept");

        assert_eq!(decoded, accept);
    }

    #[test]
    fn rejects_result_response_for_offer_send() {
        let result = NockyConnectHandoffEnvelope::result(
            "result-message-1",
            1_789_002,
            crate::connect::NockyConnectHandoffResult {
                offer_id: "offer-1".to_string(),
                status: crate::connect::NockyConnectHandoffResultStatus::RestoredPaused,
                error_message: None,
            },
        );
        let body = serde_json::to_string(&result).expect("result json");
        let response = format!("HTTP/1.1 202 Accepted\r\n\r\n{}", body);

        let error = decode_handoff_offer_response(response.as_bytes()).expect_err("kind should fail");

        assert_eq!(
            error,
            NockyConnectHandoffHttpError::UnsupportedKind(NockyConnectHandoffKind::Result),
        );
    }

    fn sample_offer() -> NockyConnectHandoffEnvelope {
        NockyConnectHandoffEnvelope::offer(
            "offer-message-1",
            1_789_000,
            NockyConnectHandoffOffer {
                offer_id: "offer-1".to_string(),
                sender_device_id: "desktop-1".to_string(),
                sender_device_name: "Nocky Desktop".to_string(),
                receiver_device_id: "android-1".to_string(),
                snapshot_summary: NockyConnectSnapshotSummary {
                    source: NockyConnectSource::YouTube,
                    current_title: Some("Juno".to_string()),
                    current_artist: Some("Sabrina Carpenter".to_string()),
                    queue_items: 89,
                    position_ms: 2_267,
                    duration_ms: Some(223_000),
                    was_playing: true,
                },
                restore_policy: NockyConnectRestorePolicy::RestorePaused,
            },
        )
    }
}
