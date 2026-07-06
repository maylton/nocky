//! Minimal local HTTP client for Nocky Connect handoff transfers.
//!
//! This module intentionally uses `std::net::TcpStream` instead of adding a new
//! HTTP dependency. It sends JSON payloads to a LAN peer and reads one JSON
//! handoff response.

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

pub const NOCKY_CONNECT_SNAPSHOT_PATH: &str = "/nocky-connect/snapshot";

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
            Self::InvalidResponse(error) => {
                write!(formatter, "invalid handoff HTTP response: {error}")
            }
            Self::HttpStatus(status) => write!(formatter, "handoff HTTP request failed: {status}"),
            Self::UnsupportedSchema(schema) => write!(formatter, "unsupported handoff schema {schema}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported handoff schema version {version}")
            }
            Self::UnsupportedKind(kind) => {
                write!(formatter, "unsupported handoff response kind {kind:?}")
            }
        }
    }
}

impl std::error::Error for NockyConnectHandoffHttpError {}

pub fn send_handoff_offer_http(
    target: &NockyConnectHandoffTarget,
    envelope: &NockyConnectHandoffEnvelope,
    timeout: Duration,
) -> Result<NockyConnectHandoffEnvelope, NockyConnectHandoffHttpError> {
    let body = serde_json::to_string(envelope)
        .map_err(|error| NockyConnectHandoffHttpError::Json(error.to_string()))?;
    let response = send_json_http(target, &target.path, &body, timeout)?;
    decode_handoff_response(
        &response,
        &[NockyConnectHandoffKind::Accept, NockyConnectHandoffKind::Decline],
    )
}

pub fn send_handoff_snapshot_http(
    target: &NockyConnectHandoffTarget,
    snapshot_json: &str,
    timeout: Duration,
) -> Result<NockyConnectHandoffEnvelope, NockyConnectHandoffHttpError> {
    let response = send_json_http(target, NOCKY_CONNECT_SNAPSHOT_PATH, snapshot_json, timeout)?;
    decode_handoff_response(&response, &[NockyConnectHandoffKind::Result])
}

fn send_json_http(
    target: &NockyConnectHandoffTarget,
    path: &str,
    body: &str,
    timeout: Duration,
) -> Result<Vec<u8>, NockyConnectHandoffHttpError> {
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

    let request = build_json_post_request(target, path, body);
    stream
        .write_all(&request)
        .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| NockyConnectHandoffHttpError::Io(error.to_string()))?;

    read_http_response(&mut stream)
}

fn read_http_response(stream: &mut TcpStream) -> Result<Vec<u8>, NockyConnectHandoffHttpError> {
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
        if response_has_complete_body(&response)? {
            break;
        }
    }
    Ok(response)
}

fn response_has_complete_body(response: &[u8]) -> Result<bool, NockyConnectHandoffHttpError> {
    let Some(header_end) = find_header_end(response) else {
        return Ok(false);
    };
    let header_text = std::str::from_utf8(&response[..header_end])
        .map_err(|error| NockyConnectHandoffHttpError::InvalidResponse(error.to_string()))?;
    let Some(content_length) = content_length(header_text) else {
        return Ok(false);
    };
    Ok(response.len().saturating_sub(header_end + 4) >= content_length)
}

fn find_header_end(response: &[u8]) -> Option<usize> {
    response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
}

fn content_length(header_text: &str) -> Option<usize> {
    header_text
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|line| line.split_once(':'))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
}

pub(crate) fn build_handoff_offer_request(
    target: &NockyConnectHandoffTarget,
    envelope: &NockyConnectHandoffEnvelope,
) -> Result<Vec<u8>, NockyConnectHandoffHttpError> {
    let body = serde_json::to_string(envelope)
        .map_err(|error| NockyConnectHandoffHttpError::Json(error.to_string()))?;
    Ok(build_json_post_request(target, &target.path, &body))
}

pub(crate) fn build_handoff_snapshot_request(
    target: &NockyConnectHandoffTarget,
    snapshot_json: &str,
) -> Vec<u8> {
    build_json_post_request(target, NOCKY_CONNECT_SNAPSHOT_PATH, snapshot_json)
}

fn build_json_post_request(target: &NockyConnectHandoffTarget, path: &str, body: &str) -> Vec<u8> {
    let body_bytes = body.as_bytes();
    let path = normalized_path(path);
    let headers = format!(
        "POST {path} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        target.host,
        target.port,
        body_bytes.len(),
    );

    let mut request = headers.into_bytes();
    request.extend_from_slice(body_bytes);
    request
}

pub(crate) fn decode_handoff_offer_response(
    response: &[u8],
) -> Result<NockyConnectHandoffEnvelope, NockyConnectHandoffHttpError> {
    decode_handoff_response(
        response,
        &[NockyConnectHandoffKind::Accept, NockyConnectHandoffKind::Decline],
    )
}

pub(crate) fn decode_handoff_snapshot_response(
    response: &[u8],
) -> Result<NockyConnectHandoffEnvelope, NockyConnectHandoffHttpError> {
    decode_handoff_response(response, &[NockyConnectHandoffKind::Result])
}

fn decode_handoff_response(
    response: &[u8],
    accepted_kinds: &[NockyConnectHandoffKind],
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

    let envelope = serde_json::from_str::<NockyConnectHandoffEnvelope>(body.trim_end_matches('\0'))
        .map_err(|error| NockyConnectHandoffHttpError::Json(error.to_string()))?;
    require_supported_handoff_response(&envelope, accepted_kinds)?;
    Ok(envelope)
}

fn require_supported_handoff_response(
    envelope: &NockyConnectHandoffEnvelope,
    accepted_kinds: &[NockyConnectHandoffKind],
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
    if accepted_kinds.contains(&envelope.kind) {
        Ok(())
    } else {
        Err(NockyConnectHandoffHttpError::UnsupportedKind(envelope.kind))
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
        NockyConnectHandoffAccept, NockyConnectHandoffOffer, NockyConnectRestorePolicy,
        NockyConnectSnapshotSummary, NockyConnectSource,
    };

    #[test]
    fn builds_post_request_for_handoff_offer() {
        let target = test_target();
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
    fn builds_post_request_for_snapshot_transfer() {
        let target = test_target();
        let request = build_handoff_snapshot_request(&target, r#"{"schema":"snapshot"}"#);
        let text = String::from_utf8(request).expect("utf-8 request");

        assert!(text.starts_with("POST /nocky-connect/snapshot HTTP/1.1\r\n"));
        assert!(text.contains("Host: 192.168.0.8:35187\r\n"));
        assert!(text.contains("Content-Length: 21\r\n"));
    }

    #[test]
    fn detects_complete_response_body_from_content_length() {
        let response = b"HTTP/1.1 202 Accepted\r\nContent-Length: 2\r\n\r\n{}";
        assert!(response_has_complete_body(response).expect("complete check"));
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
    fn decodes_result_response_for_snapshot_transfer() {
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

        let decoded = decode_handoff_snapshot_response(response.as_bytes()).expect("decode result");

        assert_eq!(decoded, result);
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

    fn test_target() -> NockyConnectHandoffTarget {
        NockyConnectHandoffTarget {
            host: "192.168.0.8".to_string(),
            port: 35187,
            path: "/nocky-connect/handoff".to_string(),
            transport: NockyConnectHandoffTransport::LocalHttp,
        }
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
