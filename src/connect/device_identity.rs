use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct NockyConnectDeviceIdentity {
    path: PathBuf,
}

impl NockyConnectDeviceIdentity {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            path: base_dir.as_ref().join("nocky-connect").join("device-id"),
        }
    }

    pub fn get_or_create(&self) -> io::Result<String> {
        if let Ok(existing) = fs::read_to_string(&self.path) {
            let existing = existing.trim().to_string();
            if !existing.is_empty() {
                return Ok(existing);
            }
        }

        let generated = generate_device_id();
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, format!("{generated}\n"))?;
        Ok(generated)
    }
}

pub fn default_connect_config_dir() -> PathBuf {
    if let Some(xdg_config) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg_config).join("nocky");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config").join("nocky");
    }
    std::env::temp_dir().join("nocky")
}

fn generate_device_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let process = u128::from(std::process::id());
    let address_entropy = {
        let marker = String::new();
        marker.as_ptr() as usize as u128
    };
    format!(
        "desktop-{:#034x}",
        nanos ^ (process << 64) ^ address_entropy
    )
    .replace("0x", "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn creates_and_reuses_device_id() {
        let root = temp_root();
        let identity = NockyConnectDeviceIdentity::new(&root);

        let first = identity.get_or_create().expect("create device id");
        let second = identity.get_or_create().expect("reuse device id");

        assert!(!first.is_empty());
        assert_eq!(first, second);

        let _ = fs::remove_dir_all(root);
    }

    fn temp_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nocky-connect-identity-test-{stamp}"))
    }
}
