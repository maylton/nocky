use serde::{Deserialize, Serialize};

use super::{NOCKY_CONNECT_PROTOCOL_VERSION};

pub const DEVICE_DESCRIPTOR_SCHEMA: &str = "io.github.maylton.nocky.connect.DeviceDescriptor";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectDeviceDescriptor {
    pub schema: String,
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "device_id")]
    pub device_id: String,
    #[serde(rename = "device_name")]
    pub device_name: String,
    pub platform: NockyConnectDevicePlatform,
    #[serde(rename = "app_name")]
    pub app_name: String,
    #[serde(rename = "app_version")]
    pub app_version: Option<String>,
    #[serde(rename = "protocol_version")]
    pub protocol_version: u32,
    pub features: Vec<NockyConnectFeature>,
}

impl NockyConnectDeviceDescriptor {
    pub fn linux_desktop(
        device_id: impl Into<String>,
        device_name: impl Into<String>,
        app_version: Option<String>,
    ) -> Self {
        Self {
            schema: DEVICE_DESCRIPTOR_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            device_id: device_id.into(),
            device_name: device_name.into(),
            platform: NockyConnectDevicePlatform::LinuxDesktop,
            app_name: "Nocky Desktop".to_string(),
            app_version,
            protocol_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            features: vec![
                NockyConnectFeature::SnapshotExport,
                NockyConnectFeature::SnapshotImportPaused,
                NockyConnectFeature::FileRoundTrip,
            ],
        }
    }

    pub fn require_supported(&self) -> Result<(), NockyConnectDeviceDescriptorError> {
        if self.schema != DEVICE_DESCRIPTOR_SCHEMA {
            return Err(NockyConnectDeviceDescriptorError::UnsupportedSchema(
                self.schema.clone(),
            ));
        }
        if self.schema_version != NOCKY_CONNECT_PROTOCOL_VERSION {
            return Err(NockyConnectDeviceDescriptorError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.protocol_version != NOCKY_CONNECT_PROTOCOL_VERSION {
            return Err(NockyConnectDeviceDescriptorError::UnsupportedProtocolVersion(
                self.protocol_version,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectDevicePlatform {
    #[serde(rename = "android")]
    Android,
    #[serde(rename = "linux_desktop")]
    LinuxDesktop,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectFeature {
    #[serde(rename = "snapshot_export")]
    SnapshotExport,
    #[serde(rename = "snapshot_import_paused")]
    SnapshotImportPaused,
    #[serde(rename = "file_round_trip")]
    FileRoundTrip,
    #[serde(rename = "lan_pairing")]
    LanPairing,
    #[serde(rename = "handoff_ack")]
    HandoffAck,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NockyConnectDeviceDescriptorError {
    UnsupportedSchema(String),
    UnsupportedSchemaVersion(u32),
    UnsupportedProtocolVersion(u32),
    Json(String),
}

impl std::fmt::Display for NockyConnectDeviceDescriptorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => write!(formatter, "unsupported descriptor schema {schema}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported descriptor schema version {version}")
            }
            Self::UnsupportedProtocolVersion(version) => {
                write!(formatter, "unsupported protocol version {version}")
            }
            Self::Json(error) => write!(formatter, "invalid descriptor JSON: {error}"),
        }
    }
}

impl std::error::Error for NockyConnectDeviceDescriptorError {}

pub fn encode_device_descriptor(
    descriptor: &NockyConnectDeviceDescriptor,
) -> Result<String, NockyConnectDeviceDescriptorError> {
    serde_json::to_string_pretty(descriptor)
        .map_err(|error| NockyConnectDeviceDescriptorError::Json(error.to_string()))
}

pub fn decode_device_descriptor(
    payload: &str,
) -> Result<NockyConnectDeviceDescriptor, NockyConnectDeviceDescriptorError> {
    let descriptor = serde_json::from_str::<NockyConnectDeviceDescriptor>(payload)
        .map_err(|error| NockyConnectDeviceDescriptorError::Json(error.to_string()))?;
    descriptor.require_supported()?;
    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_desktop_descriptor() {
        let descriptor = NockyConnectDeviceDescriptor::linux_desktop(
            "desktop-device",
            "Linux desktop",
            Some("dev".to_string()),
        );

        let payload = encode_device_descriptor(&descriptor).expect("encode descriptor");
        let decoded = decode_device_descriptor(&payload).expect("decode descriptor");

        assert_eq!(decoded.schema, DEVICE_DESCRIPTOR_SCHEMA);
        assert_eq!(decoded.schema_version, NOCKY_CONNECT_PROTOCOL_VERSION);
        assert_eq!(decoded.device_id, "desktop-device");
        assert_eq!(decoded.platform, NockyConnectDevicePlatform::LinuxDesktop);
        assert!(decoded.features.contains(&NockyConnectFeature::SnapshotExport));
        assert!(decoded.features.contains(&NockyConnectFeature::SnapshotImportPaused));
    }

    #[test]
    fn rejects_unsupported_protocol_version() {
        let payload = r#"{
            "schema":"io.github.maylton.nocky.connect.DeviceDescriptor",
            "schema_version":1,
            "device_id":"future-device",
            "device_name":"Future device",
            "platform":"linux_desktop",
            "app_name":"Nocky Desktop",
            "app_version":"future",
            "protocol_version":99,
            "features":["snapshot_export"]
        }"#;

        let error = decode_device_descriptor(payload).expect_err("version should fail");
        assert_eq!(
            error,
            NockyConnectDeviceDescriptorError::UnsupportedProtocolVersion(99),
        );
    }
}
