use crate::host::{self, HostConfig, apply_hostname, apply_ssh_key};
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

    let _ = apply_ssh_key(public, private, Some(root));

    let pub_path = root
        .join(host::DEFAULT_SSH_DIR)
        .join(host::SSH_KEY_ED25519_PUB);
    let priv_path = root.join(host::DEFAULT_SSH_DIR).join(host::SSH_KEY_ED25519);

    assert!(pub_path.exists());
    assert!(priv_path.exists());

    apply_ssh_key("different-public", "different-private", Some(root)).unwrap();

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

    apply_ssh_key(public, private, Some(root)).unwrap();

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
