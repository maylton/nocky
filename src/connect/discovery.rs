use serde::{Deserialize, Serialize};

use super::{NockyConnectDeviceDescriptor, NOCKY_CONNECT_PROTOCOL_VERSION};

pub const NOCKY_CONNECT_DISCOVERY_SCHEMA: &str = "io.github.maylton.nocky.connect.LanDiscovery";
pub const NOCKY_CONNECT_DISCOVERY_PORT: u16 = 34987;
pub const NOCKY_CONNECT_DISCOVERY_MAGIC: &str = "NOCKY_CONNECT_DISCOVERY_V1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectDiscoveryEnvelope {
    pub schema: String,
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    pub magic: String,
    #[serde(rename = "message_id")]
    pub message_id: String,
    pub kind: NockyConnectDiscoveryKind,
    pub descriptor: NockyConnectDeviceDescriptor,
}

impl NockyConnectDiscoveryEnvelope {
    pub fn hello(message_id: impl Into<String>, descriptor: NockyConnectDeviceDescriptor) -> Self {
        Self::new(message_id, NockyConnectDiscoveryKind::Hello, descriptor)
    }

    pub fn announce(
        message_id: impl Into<String>,
        descriptor: NockyConnectDeviceDescriptor,
    ) -> Self {
        Self::new(message_id, NockyConnectDiscoveryKind::Announce, descriptor)
    }

    fn new(
        message_id: impl Into<String>,
        kind: NockyConnectDiscoveryKind,
        descriptor: NockyConnectDeviceDescriptor,
    ) -> Self {
        Self {
            schema: NOCKY_CONNECT_DISCOVERY_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            magic: NOCKY_CONNECT_DISCOVERY_MAGIC.to_string(),
            message_id: message_id.into(),
            kind,
            descriptor,
        }
    }

    pub fn require_supported(&self) -> Result<(), NockyConnectDiscoveryError> {
        if self.schema != NOCKY_CONNECT_DISCOVERY_SCHEMA {
            return Err(NockyConnectDiscoveryError::UnsupportedSchema(
                self.schema.clone(),
            ));
        }
        if self.schema_version != NOCKY_CONNECT_PROTOCOL_VERSION {
            return Err(NockyConnectDiscoveryError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.magic != NOCKY_CONNECT_DISCOVERY_MAGIC {
            return Err(NockyConnectDiscoveryError::UnsupportedMagic(
                self.magic.clone(),
            ));
        }
        self.descriptor
            .require_supported()
            .map_err(|error| NockyConnectDiscoveryError::Descriptor(error.to_string()))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectDiscoveryKind {
    #[serde(rename = "hello")]
    Hello,
    #[serde(rename = "announce")]
    Announce,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NockyConnectDiscoveryError {
    UnsupportedSchema(String),
    UnsupportedSchemaVersion(u32),
    UnsupportedMagic(String),
    Descriptor(String),
    Json(String),
}

impl std::fmt::Display for NockyConnectDiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported discovery schema {schema}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported discovery schema version {version}")
            }
            Self::UnsupportedMagic(magic) => {
                write!(formatter, "unsupported discovery magic {magic}")
            }
            Self::Descriptor(error) => write!(formatter, "invalid discovery descriptor: {error}"),
            Self::Json(error) => write!(formatter, "invalid discovery JSON: {error}"),
        }
    }
}

impl std::error::Error for NockyConnectDiscoveryError {}

pub fn encode_discovery_envelope(
    envelope: &NockyConnectDiscoveryEnvelope,
) -> Result<String, NockyConnectDiscoveryError> {
    serde_json::to_string_pretty(envelope)
        .map_err(|error| NockyConnectDiscoveryError::Json(error.to_string()))
}

pub fn decode_discovery_envelope(
    payload: &str,
) -> Result<NockyConnectDiscoveryEnvelope, NockyConnectDiscoveryError> {
    let envelope = serde_json::from_str::<NockyConnectDiscoveryEnvelope>(payload)
        .map_err(|error| NockyConnectDiscoveryError::Json(error.to_string()))?;
    envelope.require_supported()?;
    Ok(envelope)
}

pub fn discovery_response_for_payload(
    payload: &str,
    local_descriptor: &NockyConnectDeviceDescriptor,
    response_message_id: impl Into<String>,
) -> Result<Option<String>, NockyConnectDiscoveryError> {
    let envelope = decode_discovery_envelope(payload)?;
    if envelope.kind != NockyConnectDiscoveryKind::Hello {
        return Ok(None);
    }
    if envelope.descriptor.device_id == local_descriptor.device_id {
        return Ok(None);
    }

    let response =
        NockyConnectDiscoveryEnvelope::announce(response_message_id, local_descriptor.clone());
    encode_discovery_envelope(&response).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desktop_descriptor(device_id: &str) -> NockyConnectDeviceDescriptor {
        NockyConnectDeviceDescriptor::linux_desktop(
            device_id,
            "Linux desktop",
            Some("dev".to_string()),
        )
    }

    #[test]
    fn discovery_hello_round_trips() {
        let descriptor = desktop_descriptor("desktop-device");
        let envelope = NockyConnectDiscoveryEnvelope::hello("message-1", descriptor);

        let payload = encode_discovery_envelope(&envelope).expect("encode discovery envelope");
        let decoded = decode_discovery_envelope(&payload).expect("decode discovery envelope");

        assert_eq!(decoded.schema, NOCKY_CONNECT_DISCOVERY_SCHEMA);
        assert_eq!(decoded.schema_version, NOCKY_CONNECT_PROTOCOL_VERSION);
        assert_eq!(decoded.magic, NOCKY_CONNECT_DISCOVERY_MAGIC);
        assert_eq!(decoded.message_id, "message-1");
        assert_eq!(decoded.kind, NockyConnectDiscoveryKind::Hello);
        assert_eq!(decoded.descriptor.device_id, "desktop-device");
    }

    #[test]
    fn replies_to_remote_hello_with_announce() {
        let remote_descriptor = desktop_descriptor("remote-device");
        let local_descriptor = desktop_descriptor("local-device");
        let hello = NockyConnectDiscoveryEnvelope::hello("hello-1", remote_descriptor);
        let payload = encode_discovery_envelope(&hello).expect("encode hello");

        let response_payload =
            discovery_response_for_payload(&payload, &local_descriptor, "announce-1")
                .expect("response helper should parse hello")
                .expect("remote hello should receive response");
        let response = decode_discovery_envelope(&response_payload).expect("decode response");

        assert_eq!(response.kind, NockyConnectDiscoveryKind::Announce);
        assert_eq!(response.message_id, "announce-1");
        assert_eq!(response.descriptor.device_id, "local-device");
    }

    #[test]
    fn ignores_own_hello() {
        let local_descriptor = desktop_descriptor("local-device");
        let hello = NockyConnectDiscoveryEnvelope::hello("hello-1", local_descriptor.clone());
        let payload = encode_discovery_envelope(&hello).expect("encode hello");

        let response = discovery_response_for_payload(&payload, &local_descriptor, "announce-1")
            .expect("response helper should parse hello");

        assert!(response.is_none());
    }

    #[test]
    fn ignores_announce_packets() {
        let local_descriptor = desktop_descriptor("local-device");
        let remote_descriptor = desktop_descriptor("remote-device");
        let announce =
            NockyConnectDiscoveryEnvelope::announce("announce-remote", remote_descriptor);
        let payload = encode_discovery_envelope(&announce).expect("encode announce");

        let response = discovery_response_for_payload(&payload, &local_descriptor, "announce-1")
            .expect("response helper should parse announce");

        assert!(response.is_none());
    }

    #[test]
    fn rejects_unknown_magic() {
        let payload = r#"{
            "schema":"io.github.maylton.nocky.connect.LanDiscovery",
            "schema_version":1,
            "magic":"OTHER_APP",
            "message_id":"message-1",
            "kind":"hello",
            "descriptor":{
                "schema":"io.github.maylton.nocky.connect.DeviceDescriptor",
                "schema_version":1,
                "device_id":"desktop-device",
                "device_name":"Linux desktop",
                "platform":"linux_desktop",
                "app_name":"Nocky Desktop",
                "app_version":"dev",
                "protocol_version":1,
                "features":["snapshot_export","snapshot_import_paused"]
            }
        }"#;

        let error = decode_discovery_envelope(payload).expect_err("magic should fail");
        assert_eq!(
            error,
            NockyConnectDiscoveryError::UnsupportedMagic("OTHER_APP".to_string()),
        );
    }
}
