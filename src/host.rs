#[cfg(not(test))]
use nix::unistd;

use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path;

pub const DEFAULT_ROOT: &str = "/";
pub const DEFAULT_HOSTNAME_PATH: &str = "etc/hostname";
pub const DEFAULT_SSH_DIR: &str = "etc/ssh";
pub const SSH_KEY_ED25519: &str = "ssh_host_ed25519_key";
pub const SSH_KEY_ED25519_PUB: &str = "ssh_host_ed25519_key.pub";

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
    file: &path::PathBuf,
    root: Option<&path::Path>,
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
    root: Option<&path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(test))]
    {
        let current = unistd::gethostname()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_default();

        if current != hostname {
            unistd::sethostname(hostname)?;
        }
    }

    let hostname_path = root
        .unwrap_or(path::Path::new(DEFAULT_ROOT))
        .join(path::Path::new(DEFAULT_HOSTNAME_PATH));
    if let Some(parent) = hostname_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(hostname_path, hostname)?;

    Ok(())
}

pub fn apply_ssh_key(
    public: &str,
    private: &str,
    root: Option<&path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ssh_dir = root
        .unwrap_or(path::Path::new(DEFAULT_ROOT))
        .join(path::Path::new(DEFAULT_SSH_DIR));

    fs::create_dir_all(&ssh_dir)?;

    let pub_path = ssh_dir.join(path::Path::new(SSH_KEY_ED25519_PUB));
    let priv_path = ssh_dir.join(path::Path::new(SSH_KEY_ED25519));

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
