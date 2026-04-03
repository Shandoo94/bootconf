#[cfg(not(test))]
use nix::unistd::{gethostname, sethostname};

use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const DEFAULT_ETC: &str = "/etc";

#[derive(Deserialize, Debug)]
pub struct HostConfig {
    pub hostname: String,
    #[serde(rename = "ssh_host_keys")]
    pub ssh_keys: Option<SshHostKeys>,
}

#[derive(Deserialize, Debug)]
pub struct SshHostKeys {
    pub ed25519: Option<SshKeyPair>,
}

#[derive(Deserialize, Debug)]
pub struct SshKeyPair {
    pub public: String,
    pub private: String,
}

pub fn apply_host_config(
    file: &PathBuf,
    root: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file)?;
    let config: HostConfig = toml::from_str(&content)?;

    apply_hostname(&config.hostname, root)?;

    if let Some(ssh_keys) = config.ssh_keys {
        if let Some(ed25519) = ssh_keys.ed25519 {
            apply_ssh_key(&ed25519.public, &ed25519.private, root)?;
        }
    }

    Ok(())
}

pub fn apply_hostname(
    hostname: &str,
    root: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(test))]
    {
        let current = gethostname()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_default();

        if current != hostname {
            sethostname(hostname)?;
        }
    }

    let etc_path = root
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ETC));
    let hostname_path = etc_path.join("hostname");
    fs::write(hostname_path, hostname)?;

    Ok(())
}

pub fn apply_ssh_key(
    public: &str,
    private: &str,
    root: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let etc_path = root
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ETC));
    let ssh_dir = etc_path.join("ssh");

    if !ssh_dir.exists() {
        fs::create_dir(&ssh_dir)?;
    }

    let pub_path = ssh_dir.join("ssh_host_ed25519_key.pub");
    let priv_path = ssh_dir.join("ssh_host_ed25519_key");

    if !pub_path.exists() {
        fs::write(&pub_path, public)?;
    }

    if !priv_path.exists() {
        fs::write(&priv_path, private)?;
        let mut perms = fs::metadata(&priv_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&priv_path, perms)?;
    }

    Ok(())
}

