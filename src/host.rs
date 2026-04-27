use log::{debug, info};

use nix::unistd;
use serde::Deserialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path;

pub const DEFAULT_HOSTNAME_PATH: &str = "/etc/hostname";
pub const DEFAULT_SSH_DIR: &str = "/etc/ssh";
pub const DEFAULT_TIMEZONE_PATH: &str = "/etc/localtime";
pub const DEFAULT_ZONEINFO_DIR: &str = "/usr/share/zoneinfo";
pub const SSH_KEY_ED25519: &str = "ssh_host_ed25519_key";
pub const SSH_KEY_ED25519_PUB: &str = "ssh_host_ed25519_key.pub";
pub const SSH_KEY_RSA: &str = "ssh_host_rsa_key";
pub const SSH_KEY_RSA_PUB: &str = "ssh_host_rsa_key.pub";
pub const SSH_KEY_ECDSA: &str = "ssh_host_ecdsa_key";
pub const SSH_KEY_ECDSA_PUB: &str = "ssh_host_ecdsa_key.pub";

#[derive(Deserialize, Debug)]
pub struct HostConfig {
    pub hostname: String,
    pub locale: Option<Locale>,
    #[serde(rename = "ssh_host_keys")]
    pub ssh_keys: Option<SshHostKeys>,
}

#[derive(Deserialize, Debug)]
pub struct Locale {
    pub timezone: String,
}

#[derive(Deserialize, Debug)]
pub struct SshHostKeys {
    pub ed25519: Option<SshKeyPair>,
    pub rsa: Option<SshKeyPair>,
    pub ecdsa: Option<SshKeyPair>,
}

#[derive(Deserialize, Debug)]
pub struct SshKeyPair {
    pub public: String,
    pub private: String,
}

pub fn apply_host_config(
    file: &path::PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file)?;
    let config: HostConfig = toml::from_str(&content)?;

    apply_hostname(&config.hostname)?;

    if let Some(locale) = config.locale {
        apply_timezone(&locale.timezone)?;
    }

    if let Some(ssh_keys) = config.ssh_keys {
        if let Some(ed25519) = ssh_keys.ed25519 {
            apply_ssh_key(&ed25519.public, &ed25519.private, "ed25519")?;
        }
        if let Some(rsa) = ssh_keys.rsa {
            apply_ssh_key(&rsa.public, &rsa.private, "rsa")?;
        }
        if let Some(ecdsa) = ssh_keys.ecdsa {
            apply_ssh_key(&ecdsa.public, &ecdsa.private, "ecdsa")?;
        }
    }

    Ok(())
}

pub fn apply_hostname(
    hostname: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let current = unistd::gethostname()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_default();

    if current != hostname {
        info!("setting hostname from '{current}' to '{hostname}'");
        unistd::sethostname(hostname)?;
    } else {
        debug!("hostname already set to '{hostname}'");
    }

    let hostname_path = path::PathBuf::from(DEFAULT_HOSTNAME_PATH);
    if let Some(parent) = hostname_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(hostname_path, hostname)?;

    Ok(())
}

pub fn apply_timezone(
    zone: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let zoneinfo_file = format!("{}/{}", DEFAULT_ZONEINFO_DIR, zone);
    if !path::Path::new(&zoneinfo_file).exists() {
        return Err(format!(
            "Timezone file not found: {} (for zone: {})",
            zoneinfo_file, zone
        )
        .into());
    }

    let localtime_path = path::PathBuf::from(DEFAULT_TIMEZONE_PATH);

    if let Some(parent) = localtime_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let needs_update = if localtime_path.exists() {
        match fs::read_link(&localtime_path) {
            Ok(current_target) => {
                let current_target_str = current_target.to_string_lossy();
                current_target_str != zoneinfo_file
            }
            Err(_) => true,
        }
    } else {
        true
    };

    if needs_update {
        info!("updating timezone symlink to '{zone}'");
        if localtime_path.exists() {
            fs::remove_file(&localtime_path)?;
        }

        std::os::unix::fs::symlink(&zoneinfo_file, &localtime_path)?;
    } else {
        debug!("timezone already set to '{zone}'");
    }

    Ok(())
}

pub fn apply_ssh_key(
    public: &str,
    private: &str,
    key_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ssh_dir = path::PathBuf::from(DEFAULT_SSH_DIR);

    fs::create_dir_all(&ssh_dir)?;

    let (pub_filename, priv_filename) = match key_type {
        "ed25519" => (SSH_KEY_ED25519_PUB, SSH_KEY_ED25519),
        "rsa" => (SSH_KEY_RSA_PUB, SSH_KEY_RSA),
        "ecdsa" => (SSH_KEY_ECDSA_PUB, SSH_KEY_ECDSA),
        _ => return Err("Unknown SSH key type".into()),
    };

    let pub_path = ssh_dir.join(path::Path::new(pub_filename));
    let priv_path = ssh_dir.join(path::Path::new(priv_filename));

    if !pub_path.exists() {
        info!("writing {key_type} SSH host public key");
        fs::write(&pub_path, public)?;
    } else {
        debug!("{key_type} SSH host public key already exists, skipping");
    }

    if !priv_path.exists() {
        info!("writing {key_type} SSH host private key");
        fs::write(&priv_path, private)?;
        let mut perms = fs::metadata(&priv_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&priv_path, perms)?;
    } else {
        debug!("{key_type} SSH host private key already exists, skipping");
    }

    Ok(())
}
