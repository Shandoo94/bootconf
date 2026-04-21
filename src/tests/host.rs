use crate::host::{self, HostConfig, apply_hostname, apply_ssh_key, apply_timezone};
use nix::unistd;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

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
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITESTKEY root@test";
    let private =
        "-----BEGIN OPENSSH PRIVATE KEY-----\nTESTKEYDATA\n-----END OPENSSH PRIVATE KEY-----";

    let _ = apply_ssh_key(public, private, "ed25519", Some(root));

    let pub_path = root
        .join(host::DEFAULT_SSH_DIR)
        .join(host::SSH_KEY_ED25519_PUB);
    let priv_path = root.join(host::DEFAULT_SSH_DIR).join(host::SSH_KEY_ED25519);

    assert!(pub_path.exists());
    assert!(priv_path.exists());

    apply_ssh_key(
        "different-public",
        "different-private",
        "ed25519",
        Some(root),
    )
    .unwrap();

    assert_eq!(fs::read_to_string(&pub_path).unwrap(), public);
    assert_eq!(fs::read_to_string(&priv_path).unwrap(), private);
}

#[test]
fn test_ssh_key_permissions() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITESTKEY root@test";
    let private =
        "-----BEGIN OPENSSH PRIVATE KEY-----\nTESTKEYDATA\n-----END OPENSSH PRIVATE KEY-----";

    apply_ssh_key(public, private, "ed25519", Some(root)).unwrap();

    let priv_path = root.join(host::DEFAULT_SSH_DIR).join(host::SSH_KEY_ED25519);
    let perms = fs::metadata(&priv_path).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, 0o600);
}

#[test]
fn test_hostname_file_written() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    apply_hostname("test-node.local", Some(root)).unwrap();

    let hostname_path = root.join(host::DEFAULT_HOSTNAME_PATH);
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
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a fake zoneinfo directory structure
    let zoneinfo_dir = root.join(host::DEFAULT_ZONEINFO_DIR.trim_start_matches('/'));
    fs::create_dir_all(&zoneinfo_dir).unwrap();

    // Create the timezone file
    let tz_file = zoneinfo_dir.join("America/New_York");
    fs::create_dir_all(tz_file.parent().unwrap()).unwrap();
    fs::write(&tz_file, "fake timezone data").unwrap();

    // Apply timezone
    apply_timezone("America/New_York", Some(root)).unwrap();

    // Check the symlink was created
    let localtime_path = root.join(host::DEFAULT_TIMEZONE_PATH);
    assert!(localtime_path.exists());
    let symlink_target = fs::read_link(&localtime_path).unwrap();
    assert_eq!(
        symlink_target.to_string_lossy(),
        format!("{}/America/New_York", host::DEFAULT_ZONEINFO_DIR)
    );
}

#[test]
fn test_timezone_idempotency() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a fake zoneinfo directory structure
    let zoneinfo_dir = root.join(host::DEFAULT_ZONEINFO_DIR.trim_start_matches('/'));
    fs::create_dir_all(&zoneinfo_dir).unwrap();

    // Create the timezone file
    let tz_file = zoneinfo_dir.join("Europe/London");
    fs::create_dir_all(tz_file.parent().unwrap()).unwrap();
    fs::write(&tz_file, "fake timezone data").unwrap();

    // Apply timezone twice
    apply_timezone("Europe/London", Some(root)).unwrap();
    let localtime_path = root.join(host::DEFAULT_TIMEZONE_PATH);
    let first_target = fs::read_link(&localtime_path).unwrap();

    // Apply again - should not change
    apply_timezone("Europe/London", Some(root)).unwrap();
    let second_target = fs::read_link(&localtime_path).unwrap();
    assert_eq!(first_target, second_target);
}

#[test]
fn test_timezone_updates_wrong_link() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create zoneinfo directory structure with two timezones
    let zoneinfo_dir = root.join(host::DEFAULT_ZONEINFO_DIR.trim_start_matches('/'));
    fs::create_dir_all(&zoneinfo_dir).unwrap();

    let old_tz_file = zoneinfo_dir.join("America/Los_Angeles");
    fs::create_dir_all(old_tz_file.parent().unwrap()).unwrap();
    fs::write(&old_tz_file, "old timezone data").unwrap();

    let new_tz_file = zoneinfo_dir.join("Asia/Tokyo");
    fs::create_dir_all(new_tz_file.parent().unwrap()).unwrap();
    fs::write(&new_tz_file, "new timezone data").unwrap();

    // Create wrong symlink first
    let localtime_path = root.join(host::DEFAULT_TIMEZONE_PATH);
    fs::create_dir_all(localtime_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&old_tz_file, &localtime_path).unwrap();

    // Apply new timezone
    apply_timezone("Asia/Tokyo", Some(root)).unwrap();

    // Check the symlink was updated
    let symlink_target = fs::read_link(&localtime_path).unwrap();
    assert_eq!(
        symlink_target.to_string_lossy(),
        format!("{}/Asia/Tokyo", host::DEFAULT_ZONEINFO_DIR)
    );
}

#[test]
fn test_timezone_missing_zoneinfo_file() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Don't create the zoneinfo file - it should error
    let result = apply_timezone("Nonexistent/Timezone", Some(root));
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
    apply_hostname(desired, None).unwrap();

    let current = unistd::gethostname()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert_eq!(current, desired);

    let hostname_path = std::path::Path::new(host::DEFAULT_ROOT).join(host::DEFAULT_HOSTNAME_PATH);
    assert_eq!(fs::read_to_string(&hostname_path).unwrap(), desired);
}
