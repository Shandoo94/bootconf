use nix::unistd::{gethostname, sethostname};
use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const SSH_HOST_KEYS_DIR: &str = "/etc/ssh";
const SSH_HOST_KEY_FILE_PUB: &str = "/etc/ssh/ssh_host_ed25519_key.pub";
const SSH_HOST_KEY_FILE_PRIV: &str = "/etc/ssh/ssh_host_ed25519_key";
const HOSTNAME_FILE: &str = "/etc/hostname";

#[derive(Deserialize)]
struct HostConfig {
    hostname: String,
    #[serde(rename = "ssh_host_keys")]
    ssh_keys: Option<SshHostKeys>,
}

#[derive(Deserialize)]
struct SshHostKeys {
    ed25519: Option<SshKeyPair>,
}

#[derive(Deserialize)]
struct SshKeyPair {
    public: String,
    private: String,
}

pub fn apply_host_config(file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file)?;
    let config: HostConfig = toml::from_str(&content)?;

    apply_hostname(&config.hostname)?;

    if let Some(ssh_keys) = config.ssh_keys {
        if let Some(ed25519) = ssh_keys.ed25519 {
            apply_ssh_key(&ed25519.public, &ed25519.private)?;
        }
    }

    Ok(())
}

fn apply_hostname(hostname: &str) -> Result<(), Box<dyn std::error::Error>> {
    let current = gethostname()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_default();

    if current != hostname {
        sethostname(hostname)?;
        fs::write(HOSTNAME_FILE, hostname)?;
    }

    Ok(())
}

fn apply_ssh_key(public: &str, private: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(SSH_HOST_KEYS_DIR);
    if !dir.exists() {
        fs::create_dir(&dir)?;
    }

    let pub_path = PathBuf::from(SSH_HOST_KEY_FILE_PUB);
    let priv_path = PathBuf::from(SSH_HOST_KEY_FILE_PRIV);

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

