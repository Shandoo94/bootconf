use crate::users::{
    self, AUTHORIZED_KEYS, AUTHORIZED_KEYS_DIR, AUTHORIZED_KEYS_MODE, SSH_DIR, SSH_DIR_MODE, User,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn make_user(name: &str, uid: u32, home: Option<&str>, authorized_keys: Option<Vec<&str>>) -> User {
    User {
        name: name.to_string(),
        uid,
        groups: None,
        shell: None,
        home: home.map(String::from),
        password: None,
        authorized_keys: authorized_keys.map(|keys| keys.iter().map(|k| k.to_string()).collect()),
    }
}

#[test]
fn test_authorized_keys_home_dir() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let user = make_user(
        "testuser",
        1000,
        Some("/home/testuser"),
        Some(vec![
            "ssh-ed25519 AAAAkey1 user@host",
            "ssh-ed25519 AAAAkey2 user@host",
        ]),
    );

    users::ensure_authorized_keys(&user, root).unwrap();

    let key_path = root
        .join("home/testuser")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    assert!(key_path.exists());

    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 AAAAkey1 user@host"));
    assert!(content.contains("ssh-ed25519 AAAAkey2 user@host"));
}

#[test]
fn test_authorized_keys_fallback_dir() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let user = make_user(
        "testuser",
        1000,
        None,
        Some(vec!["ssh-ed25519 AAAAkey1 user@host"]),
    );

    users::ensure_authorized_keys(&user, root).unwrap();

    let key_path = root.join(AUTHORIZED_KEYS_DIR).join("testuser");
    assert!(key_path.exists());

    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 AAAAkey1 user@host"));
}

#[test]
fn test_authorized_keys_idempotency() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let user = make_user(
        "testuser",
        1000,
        Some("/home/testuser"),
        Some(vec![
            "ssh-ed25519 AAAAkey1 user@host",
            "ssh-ed25519 AAAAkey2 user@host",
        ]),
    );

    users::ensure_authorized_keys(&user, root).unwrap();
    users::ensure_authorized_keys(&user, root).unwrap();

    let key_path = root
        .join("home/testuser")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    let content = fs::read_to_string(&key_path).unwrap();
    assert_eq!(content.matches("ssh-ed25519 AAAAkey1 user@host").count(), 1);
    assert_eq!(content.matches("ssh-ed25519 AAAAkey2 user@host").count(), 1);
}

#[test]
fn test_authorized_keys_additive() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let user1 = make_user(
        "testuser",
        1000,
        Some("/home/testuser"),
        Some(vec!["ssh-ed25519 AAAAkey1 user@host"]),
    );
    users::ensure_authorized_keys(&user1, root).unwrap();

    let user2 = make_user(
        "testuser",
        1000,
        Some("/home/testuser"),
        Some(vec![
            "ssh-ed25519 AAAAkey1 user@host",
            "ssh-ed25519 AAAAkey2 user@host",
        ]),
    );
    users::ensure_authorized_keys(&user2, root).unwrap();

    let key_path = root
        .join("home/testuser")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 AAAAkey1 user@host"));
    assert!(content.contains("ssh-ed25519 AAAAkey2 user@host"));
    assert_eq!(content.matches("ssh-ed25519 AAAAkey1 user@host").count(), 1);
}

#[test]
fn test_authorized_keys_no_keys() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let user = make_user("testuser", 1000, Some("/home/testuser"), None);
    users::ensure_authorized_keys(&user, root).unwrap();

    let key_path = root
        .join("home/testuser")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    assert!(!key_path.exists());
}

#[test]
fn test_authorized_keys_permissions() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let user = make_user(
        "testuser",
        1000,
        Some("/home/testuser"),
        Some(vec!["ssh-ed25519 AAAAkey1 user@host"]),
    );

    users::ensure_authorized_keys(&user, root).unwrap();

    let key_path = root
        .join("home/testuser")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    let perms = fs::metadata(&key_path).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, AUTHORIZED_KEYS_MODE);

    let ssh_dir_path = root.join("home/testuser").join(SSH_DIR);
    let dir_perms = fs::metadata(&ssh_dir_path).unwrap().permissions();
    assert_eq!(dir_perms.mode() & 0o777, SSH_DIR_MODE);
}

#[test]
fn test_authorized_keys_fallback_additive() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let user1 = make_user("svcuser", 1001, None, Some(vec!["ssh-ed25519 KEY1 host"]));
    users::ensure_authorized_keys(&user1, root).unwrap();

    let key_path = root.join(AUTHORIZED_KEYS_DIR).join("svcuser");
    assert!(key_path.exists());

    let user2 = make_user(
        "svcuser",
        1001,
        None,
        Some(vec!["ssh-ed25519 KEY1 host", "ssh-ed25519 KEY2 host"]),
    );
    users::ensure_authorized_keys(&user2, root).unwrap();

    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 KEY1 host"));
    assert!(content.contains("ssh-ed25519 KEY2 host"));
    assert_eq!(content.matches("ssh-ed25519 KEY1 host").count(), 1);
}

