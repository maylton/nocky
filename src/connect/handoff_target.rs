//! Resolve where a handoff offer should be sent.
//!
//! LAN discovery gives us the sender address of the discovery packet and the
//! remote device descriptor. The descriptor may advertise a handoff endpoint
//! with a transport, port and path. This module combines both pieces into a
//! concrete local target without opening the network connection yet.

use std::{fmt, net::SocketAddr};

use super::{
    NockyConnectDeviceDescriptor, NockyConnectHandoffEndpoint, NockyConnectHandoffTransport,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NockyConnectHandoffTarget {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub transport: NockyConnectHandoffTransport,
}

impl NockyConnectHandoffTarget {
    pub fn local_http_url(&self) -> Option<String> {
        (self.transport == NockyConnectHandoffTransport::LocalHttp).then(|| {
            let path = if self.path.starts_with('/') {
                self.path.clone()
            } else {
                format!("/{}", self.path)
            };
            format!("http://{}:{}{}", self.host, self.port, path)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NockyConnectHandoffTargetError {
    MissingEndpoint,
    UnsupportedTransport,
}

impl fmt::Display for NockyConnectHandoffTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEndpoint => write!(formatter, "device did not advertise a handoff endpoint"),
            Self::UnsupportedTransport => write!(formatter, "unsupported handoff transport"),
        }
    }
}

impl std::error::Error for NockyConnectHandoffTargetError {}

pub fn resolve_handoff_target(
    descriptor: &NockyConnectDeviceDescriptor,
    discovery_address: SocketAddr,
) -> Result<NockyConnectHandoffTarget, NockyConnectHandoffTargetError> {
    let endpoint = descriptor
        .handoff_endpoint
        .as_ref()
        .ok_or(NockyConnectHandoffTargetError::MissingEndpoint)?;
    target_from_endpoint(endpoint, discovery_address)
}

fn target_from_endpoint(
    endpoint: &NockyConnectHandoffEndpoint,
    discovery_address: SocketAddr,
) -> Result<NockyConnectHandoffTarget, NockyConnectHandoffTargetError> {
    match endpoint.transport {
        NockyConnectHandoffTransport::LocalHttp => Ok(NockyConnectHandoffTarget {
            host: discovery_address.ip().to_string(),
            port: endpoint.port,
            path: endpoint.path.clone(),
            transport: endpoint.transport,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::{
        NockyConnectDeviceDescriptor, NockyConnectHandoffEndpoint, NockyConnectHandoffTransport,
    };

    #[test]
    fn resolves_local_http_target_from_discovery_ip_and_descriptor_endpoint() {
        let descriptor = NockyConnectDeviceDescriptor::linux_desktop(
            "desktop-1",
            "Nocky Desktop",
            Some("dev".to_string()),
        )
        .with_handoff_endpoint(NockyConnectHandoffEndpoint::local_http(35187));
        let target = resolve_handoff_target(
            &descriptor,
            SocketAddr::from(([192, 168, 0, 8], 34987)),
        )
        .expect("target should resolve");

        assert_eq!(target.host, "192.168.0.8");
        assert_eq!(target.port, 35187);
        assert_eq!(target.path, "/nocky-connect/handoff");
        assert_eq!(target.transport, NockyConnectHandoffTransport::LocalHttp);
        assert_eq!(
            target.local_http_url().as_deref(),
            Some("http://192.168.0.8:35187/nocky-connect/handoff"),
        );
    }

    #[test]
    fn returns_missing_endpoint_for_legacy_descriptor() {
        let descriptor = NockyConnectDeviceDescriptor::linux_desktop(
            "desktop-1",
            "Nocky Desktop",
            Some("dev".to_string()),
        );

        let error = resolve_handoff_target(
            &descriptor,
            SocketAddr::from(([192, 168, 0, 8], 34987)),
        )
        .expect_err("legacy descriptor should not resolve");

        assert_eq!(error, NockyConnectHandoffTargetError::MissingEndpoint);
    }
}
