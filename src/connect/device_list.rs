use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use super::{NockyConnectDeviceDescriptor, NockyConnectDiscoveredDevice};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NockyConnectDeviceListEntry {
    pub descriptor: NockyConnectDeviceDescriptor,
    pub address: SocketAddr,
    pub last_seen: Instant,
}

#[derive(Clone, Debug, Default)]
pub struct NockyConnectDeviceList {
    devices: BTreeMap<String, NockyConnectDeviceListEntry>,
}

impl NockyConnectDeviceList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.devices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn update_with_discovered<I>(&mut self, devices: I, now: Instant)
    where
        I: IntoIterator<Item = NockyConnectDiscoveredDevice>,
    {
        for device in devices {
            self.upsert(device, now);
        }
    }

    pub fn upsert(&mut self, device: NockyConnectDiscoveredDevice, now: Instant) {
        let device_id = device.descriptor.device_id.clone();
        self.devices.insert(
            device_id,
            NockyConnectDeviceListEntry {
                descriptor: device.descriptor,
                address: device.address,
                last_seen: now,
            },
        );
    }

    pub fn remove_stale(&mut self, now: Instant, max_age: Duration) {
        self.devices
            .retain(|_, entry| match now.checked_duration_since(entry.last_seen) {
                Some(age) => age <= max_age,
                None => true,
            });
    }

    pub fn entries(&self) -> Vec<&NockyConnectDeviceListEntry> {
        let mut entries = self.devices.values().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .last_seen
                .cmp(&left.last_seen)
                .then_with(|| left.descriptor.device_name.cmp(&right.descriptor.device_name))
        });
        entries
    }

    pub fn get(&self, device_id: &str) -> Option<&NockyConnectDeviceListEntry> {
        self.devices.get(device_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::{
        NockyConnectDevicePlatform, NockyConnectFeature, DEVICE_DESCRIPTOR_SCHEMA,
        NOCKY_CONNECT_PROTOCOL_VERSION,
    };

    fn descriptor(device_id: &str, device_name: &str) -> NockyConnectDeviceDescriptor {
        NockyConnectDeviceDescriptor {
            schema: DEVICE_DESCRIPTOR_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            platform: NockyConnectDevicePlatform::Android,
            app_name: "Nocky Android".to_string(),
            app_version: None,
            protocol_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            features: vec![NockyConnectFeature::SnapshotExport],
            handoff_endpoint: None,
        }
    }

    fn discovered(device_id: &str, device_name: &str, port: u16) -> NockyConnectDiscoveredDevice {
        NockyConnectDiscoveredDevice {
            descriptor: descriptor(device_id, device_name),
            address: SocketAddr::from(([192, 168, 0, 8], port)),
        }
    }

    #[test]
    fn upserts_devices_by_device_id() {
        let now = Instant::now();
        let mut list = NockyConnectDeviceList::new();

        list.upsert(discovered("android-1", "Samsung", 34987), now);
        list.upsert(
            discovered("android-1", "Samsung Renamed", 40000),
            now + Duration::from_secs(2),
        );

        assert_eq!(list.len(), 1);
        let entry = list.get("android-1").expect("device should exist");
        assert_eq!(entry.descriptor.device_name, "Samsung Renamed");
        assert_eq!(entry.address, SocketAddr::from(([192, 168, 0, 8], 40000)));
    }

    #[test]
    fn removes_stale_devices() {
        let now = Instant::now();
        let mut list = NockyConnectDeviceList::new();

        list.upsert(discovered("fresh", "Fresh", 34987), now);
        list.upsert(discovered("old", "Old", 34988), now - Duration::from_secs(60));

        list.remove_stale(now, Duration::from_secs(30));

        assert!(list.get("fresh").is_some());
        assert!(list.get("old").is_none());
    }

    #[test]
    fn entries_are_ordered_by_most_recent_first() {
        let now = Instant::now();
        let mut list = NockyConnectDeviceList::new();

        list.upsert(discovered("old", "Old", 34987), now);
        list.upsert(discovered("fresh", "Fresh", 34988), now + Duration::from_secs(5));

        let entries = list.entries();

        assert_eq!(entries[0].descriptor.device_id, "fresh");
        assert_eq!(entries[1].descriptor.device_id, "old");
    }
}
