use crate::host::{self, HostConfig, apply_hostname, apply_ssh_key, apply_timezone};
use nix::unistd;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path;

#[test]
fn test_parse_valid_config() {
    let toml_content = r#"
hostname = "test-node.local"

[ssh_host_keys.ed25519]
public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITESTKEY root@test"
private = """-----BEGIN OPENSSH PRIVATE KEY-----
TESTKEYDATA
-----END OPENSSH PRIVATE KEY-----"""
"#;
    let config: HostConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.hostname, "test-node.local");
    assert!(config.ssh_keys.is_some());
}

#[test]
fn test_parse_missing_hostname() {
    let toml_content = r#"
[ssh_host_keys.ed25519]
public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITESTKEY root@test"
private = "test"
"#;
    let result: Result<HostConfig, _> = toml::from_str(toml_content);
    assert!(result.is_err());
}

#[test]
fn test_ssh_key_idempotency() {
    let public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITESTKEY root@test";
    let private =
        "-----BEGIN OPENSSH PRIVATE KEY-----\nTESTKEYDATA\n-----END OPENSSH PRIVATE KEY-----";

    apply_ssh_key(public, private, "ed25519").unwrap();

    let pub_path = path::PathBuf::from(host::DEFAULT_SSH_DIR).join(host::SSH_KEY_ED25519_PUB);
    let priv_path = path::PathBuf::from(host::DEFAULT_SSH_DIR).join(host::SSH_KEY_ED25519);

    assert!(pub_path.exists());
    assert!(priv_path.exists());

    apply_ssh_key(
        "different-public",
        "different-private",
        "ed25519",
    )
    .unwrap();

    assert_eq!(fs::read_to_string(&pub_path).unwrap(), public);
    assert_eq!(fs::read_to_string(&priv_path).unwrap(), private);
}

#[test]
fn test_ssh_key_permissions() {
    let public = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAAATESTKEY root@test";
    let private =
        "-----BEGIN RSA PRIVATE KEY-----\nTESTKEYDATA\n-----END RSA PRIVATE KEY-----";

    apply_ssh_key(public, private, "rsa").unwrap();

    let priv_path = path::PathBuf::from(host::DEFAULT_SSH_DIR).join(host::SSH_KEY_RSA);
    let perms = fs::metadata(&priv_path).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, 0o600);
}

#[test]
fn test_hostname_file_written() {
    apply_hostname("test-node.local").unwrap();

    let hostname_path = path::PathBuf::from(host::DEFAULT_HOSTNAME_PATH);
    assert_eq!(
        fs::read_to_string(&hostname_path).unwrap(),
        "test-node.local"
    );
}

#[test]
fn test_parse_timezone_config() {
    let toml_content = r#"
hostname = "test-node.local"

[locale]
timezone = "America/New_York"
"#;
    let config: HostConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.hostname, "test-node.local");
    assert!(config.locale.is_some());
    assert_eq!(config.locale.unwrap().timezone, "America/New_York");
}

#[test]
fn test_timezone_symlink_created() {
    apply_timezone("UTC").unwrap();

    let localtime_path = path::PathBuf::from(host::DEFAULT_TIMEZONE_PATH);
    assert!(localtime_path.exists());
    let symlink_target = fs::read_link(&localtime_path).unwrap();
    assert_eq!(
        symlink_target.to_string_lossy(),
        format!("{}/UTC", host::DEFAULT_ZONEINFO_DIR)
    );
}

#[test]
fn test_timezone_idempotency() {
    apply_timezone("Europe/London").unwrap();
    let localtime_path = path::PathBuf::from(host::DEFAULT_TIMEZONE_PATH);
    let first_target = fs::read_link(&localtime_path).unwrap();

    apply_timezone("Europe/London").unwrap();
    let second_target = fs::read_link(&localtime_path).unwrap();
    assert_eq!(first_target, second_target);
}

#[test]
fn test_timezone_updates_wrong_link() {
    apply_timezone("America/Los_Angeles").unwrap();
    let localtime_path = path::PathBuf::from(host::DEFAULT_TIMEZONE_PATH);

    apply_timezone("Asia/Tokyo").unwrap();

    let symlink_target = fs::read_link(&localtime_path).unwrap();
    assert_eq!(
        symlink_target.to_string_lossy(),
        format!("{}/Asia/Tokyo", host::DEFAULT_ZONEINFO_DIR)
    );
}

#[test]
fn test_timezone_missing_zoneinfo_file() {
    let result = apply_timezone("Nonexistent/Timezone");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Timezone file not found")
    );
}

#[test]
fn test_hostname_system_call() {
    let desired = "bootconf-test-host";
    apply_hostname(desired).unwrap();

    let current = unistd::gethostname()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert_eq!(current, desired);

    let hostname_path = path::PathBuf::from(host::DEFAULT_HOSTNAME_PATH);
    assert_eq!(fs::read_to_string(&hostname_path).unwrap(), desired);
}
