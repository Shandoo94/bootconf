use crate::users::{
    self, AUTHORIZED_KEYS, AUTHORIZED_KEYS_DIR, AUTHORIZED_KEYS_MODE, SSH_DIR, SSH_DIR_MODE, User,
    UsersConfig, ensure_groups, ensure_home, ensure_password, ensure_shell, read_key_set,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process;
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
fn test_parse_users_config() {
    let toml_content = r#"
[[users]]
name = "admin"
uid = 1000
groups = ["sudo", "docker"]
shell = "/bin/bash"
home = "/home/admin"
password = "$6$hash"
authorized_keys = ["ssh-ed25519 KEY admin@host"]
"#;
    let config: UsersConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.users.len(), 1);
    assert_eq!(config.users[0].name, "admin");
    assert_eq!(config.users[0].uid, 1000);
    assert_eq!(config.users[0].groups.as_ref().unwrap().len(), 2);
    assert_eq!(config.users[0].shell.as_ref().unwrap(), "/bin/bash");
    assert_eq!(config.users[0].home.as_ref().unwrap(), "/home/admin");
    assert_eq!(config.users[0].password.as_ref().unwrap(), "$6$hash");
    assert_eq!(config.users[0].authorized_keys.as_ref().unwrap().len(), 1);
}

#[test]
fn test_parse_users_config_minimal() {
    let toml_content = r#"
[[users]]
name = "svc"
uid = 2000
"#;
    let config: UsersConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.users.len(), 1);
    assert!(config.users[0].groups.is_none());
    assert!(config.users[0].shell.is_none());
    assert!(config.users[0].home.is_none());
    assert!(config.users[0].password.is_none());
    assert!(config.users[0].authorized_keys.is_none());
}

#[test]
fn test_parse_users_config_multiple_users() {
    let toml_content = r#"
[[users]]
name = "alice"
uid = 1001

[[users]]
name = "bob"
uid = 1002
"#;
    let config: UsersConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.users.len(), 2);
    assert_eq!(config.users[0].name, "alice");
    assert_eq!(config.users[1].name, "bob");
}

#[test]
fn test_parse_users_config_missing_name() {
    let toml_content = r#"
[[users]]
uid = 1000
"#;
    let result: Result<UsersConfig, _> = toml::from_str(toml_content);
    assert!(result.is_err());
}

#[test]
fn test_parse_users_config_missing_uid() {
    let toml_content = r#"
[[users]]
name = "admin"
"#;
    let result: Result<UsersConfig, _> = toml::from_str(toml_content);
    assert!(result.is_err());
}

#[test]
fn test_read_key_set_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("keys");
    fs::write(&path, "key1\nkey2\nkey3\n").unwrap();

    let keys = read_key_set(&path);
    assert_eq!(keys.len(), 3);
    assert!(keys.contains("key1"));
    assert!(keys.contains("key2"));
    assert!(keys.contains("key3"));
}

#[test]
fn test_read_key_set_empty_lines_ignored() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("keys");
    fs::write(&path, "key1\n\nkey2\n\n").unwrap();

    let keys = read_key_set(&path);
    assert_eq!(keys.len(), 2);
    assert!(keys.contains("key1"));
    assert!(keys.contains("key2"));
}

#[test]
fn test_read_key_set_missing_file() {
    let keys = read_key_set(std::path::Path::new("/nonexistent/path"));
    assert!(keys.is_empty());
}

#[test]
fn test_create_user() {
    let user = make_user("bootconf-test-user", 9900, None, None);
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-test-user")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(existing.uid.as_raw(), 9900);
}

#[test]
fn test_create_user_duplicate() {
    let user = make_user("bootconf-dup-user", 9901, None, None);
    users::create_user(&user).unwrap();
    assert!(users::create_user(&user).is_err());
}

#[test]
fn test_ensure_home_changes_dir() {
    let user = User {
        name: "bootconf-home-user".to_string(),
        uid: 9902,
        groups: None,
        shell: None,
        home: Some("/opt/home/bootconf-home-user".to_string()),
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-home-user")
        .ok()
        .flatten()
        .unwrap();
    ensure_home(&user, &existing).unwrap();

    let updated = nix::unistd::User::from_name("bootconf-home-user")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(
        updated.dir,
        std::path::PathBuf::from("/opt/home/bootconf-home-user")
    );
}

#[test]
fn test_ensure_home_no_change_if_matching() {
    let user = User {
        name: "bootconf-home-idem".to_string(),
        uid: 9912,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-home-idem".to_string()),
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-home-idem")
        .ok()
        .flatten()
        .unwrap();

    let dir_before = existing.dir.clone();
    ensure_home(&user, &existing).unwrap();

    let updated = nix::unistd::User::from_name("bootconf-home-idem")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(updated.dir, dir_before);
}

#[test]
fn test_ensure_home_no_home_specified() {
    let user = make_user("bootconf-nohome", 9913, None, None);
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-nohome")
        .ok()
        .flatten()
        .unwrap();

    let dir_before = existing.dir.clone();
    ensure_home(&user, &existing).unwrap();

    let updated = nix::unistd::User::from_name("bootconf-nohome")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(updated.dir, dir_before);
}

#[test]
fn test_ensure_shell_changes_shell() {
    let user = User {
        name: "bootconf-shell-user".to_string(),
        uid: 9903,
        groups: None,
        shell: Some("/bin/zsh".to_string()),
        home: None,
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-shell-user")
        .ok()
        .flatten()
        .unwrap();
    ensure_shell(&user, &existing).unwrap();

    let updated = nix::unistd::User::from_name("bootconf-shell-user")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(updated.shell, std::path::PathBuf::from("/bin/zsh"));
}

#[test]
fn test_ensure_shell_no_change_if_matching() {
    let user = User {
        name: "bootconf-shell-idem".to_string(),
        uid: 9914,
        groups: None,
        shell: Some("/bin/sh".to_string()),
        home: None,
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-shell-idem")
        .ok()
        .flatten()
        .unwrap();

    let shell_before = existing.shell.clone();
    ensure_shell(&user, &existing).unwrap();

    let updated = nix::unistd::User::from_name("bootconf-shell-idem")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(updated.shell, shell_before);
}

#[test]
fn test_ensure_shell_no_shell_specified() {
    let user = make_user("bootconf-noshell", 9915, None, None);
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-noshell")
        .ok()
        .flatten()
        .unwrap();

    let shell_before = existing.shell.clone();
    ensure_shell(&user, &existing).unwrap();

    let updated = nix::unistd::User::from_name("bootconf-noshell")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(updated.shell, shell_before);
}

#[test]
fn test_ensure_password_sets_hash() {
    let user = User {
        name: "bootconf-pw-user".to_string(),
        uid: 9904,
        groups: None,
        shell: None,
        home: None,
        password: Some("$6$rounds=656000$salt$hash".to_string()),
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-pw-user")
        .ok()
        .flatten()
        .unwrap();
    ensure_password(&user, &existing).unwrap();

    let shadow_content = fs::read_to_string("/etc/shadow").unwrap();
    let line = shadow_content
        .lines()
        .find(|l| l.starts_with("bootconf-pw-user:"))
        .unwrap();
    let hash = line.split(':').nth(1).unwrap();
    assert_eq!(hash, "$6$rounds=656000$salt$hash");
}

#[test]
fn test_ensure_password_idempotent() {
    let user = User {
        name: "bootconf-pw-idem".to_string(),
        uid: 9905,
        groups: None,
        shell: None,
        home: None,
        password: Some("$6$rounds=656000$salt$hash".to_string()),
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-pw-idem")
        .ok()
        .flatten()
        .unwrap();
    ensure_password(&user, &existing).unwrap();

    let shadow_after_first = fs::read_to_string("/etc/shadow").unwrap();
    let line = shadow_after_first
        .lines()
        .find(|l| l.starts_with("bootconf-pw-idem:"))
        .unwrap();
    let hash_after_first = line.split(':').nth(1).unwrap().to_string();

    let after_first = nix::unistd::User::from_name("bootconf-pw-idem")
        .ok()
        .flatten()
        .unwrap();
    ensure_password(&user, &after_first).unwrap();

    let shadow_after_second = fs::read_to_string("/etc/shadow").unwrap();
    let line = shadow_after_second
        .lines()
        .find(|l| l.starts_with("bootconf-pw-idem:"))
        .unwrap();
    let hash_after_second = line.split(':').nth(1).unwrap().to_string();
    assert_eq!(hash_after_second, hash_after_first);
}

#[test]
fn test_ensure_password_no_password_specified() {
    let user = make_user("bootconf-nopw", 9916, None, None);
    users::create_user(&user).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-nopw")
        .ok()
        .flatten()
        .unwrap();

    let passwd_before = existing.passwd.clone();
    ensure_password(&user, &existing).unwrap();

    let updated = nix::unistd::User::from_name("bootconf-nopw")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(updated.passwd, passwd_before);
}

#[test]
fn test_ensure_groups_adds_user() {
    let status = process::Command::new("groupadd")
        .arg("bootconf-test-group")
        .status()
        .unwrap();
    assert!(status.success());

    let user = User {
        name: "bootconf-grp-user".to_string(),
        uid: 9906,
        groups: Some(vec!["bootconf-test-group".to_string()]),
        shell: None,
        home: None,
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();
    ensure_groups(&user).unwrap();

    let group = nix::unistd::Group::from_name("bootconf-test-group")
        .ok()
        .flatten()
        .unwrap();
    assert!(group.mem.contains(&"bootconf-grp-user".to_string()));
}

#[test]
fn test_ensure_groups_idempotent() {
    let status = process::Command::new("groupadd")
        .arg("bootconf-idem-group")
        .status()
        .unwrap();
    assert!(status.success());

    let user = User {
        name: "bootconf-grp-idem".to_string(),
        uid: 9908,
        groups: Some(vec!["bootconf-idem-group".to_string()]),
        shell: None,
        home: None,
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();
    ensure_groups(&user).unwrap();
    ensure_groups(&user).unwrap();

    let group = nix::unistd::Group::from_name("bootconf-idem-group")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(
        group
            .mem
            .iter()
            .filter(|m| *m == "bootconf-grp-idem")
            .count(),
        1
    );
}

#[test]
fn test_ensure_groups_no_groups_specified() {
    let user = make_user("bootconf-nogrp", 9909, None, None);
    users::create_user(&user).unwrap();
    ensure_groups(&user).unwrap();
}

#[test]
fn test_ensure_groups_nonexistent_group_skipped() {
    let user = User {
        name: "bootconf-nogrp-user".to_string(),
        uid: 9910,
        groups: Some(vec!["nonexistent-group-xyz".to_string()]),
        shell: None,
        home: None,
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();
    ensure_groups(&user).unwrap();
}

#[test]
fn test_ensure_groups_removes_stale_membership() {
    let status = process::Command::new("groupadd")
        .arg("bootconf-stale-group")
        .status()
        .unwrap();
    assert!(status.success());

    let status = process::Command::new("groupadd")
        .arg("bootconf-keep-group")
        .status()
        .unwrap();
    assert!(status.success());

    let user = User {
        name: "bootconf-stale-user".to_string(),
        uid: 9911,
        groups: Some(vec![
            "bootconf-stale-group".to_string(),
            "bootconf-keep-group".to_string(),
        ]),
        shell: None,
        home: None,
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();
    ensure_groups(&user).unwrap();

    let stale_group = nix::unistd::Group::from_name("bootconf-stale-group")
        .ok()
        .flatten()
        .unwrap();
    let keep_group = nix::unistd::Group::from_name("bootconf-keep-group")
        .ok()
        .flatten()
        .unwrap();
    assert!(stale_group.mem.contains(&"bootconf-stale-user".to_string()));
    assert!(keep_group.mem.contains(&"bootconf-stale-user".to_string()));

    let user_updated = User {
        name: "bootconf-stale-user".to_string(),
        uid: 9911,
        groups: Some(vec!["bootconf-keep-group".to_string()]),
        shell: None,
        home: None,
        password: None,
        authorized_keys: None,
    };
    ensure_groups(&user_updated).unwrap();

    let stale_group_after = nix::unistd::Group::from_name("bootconf-stale-group")
        .ok()
        .flatten()
        .unwrap();
    let keep_group_after = nix::unistd::Group::from_name("bootconf-keep-group")
        .ok()
        .flatten()
        .unwrap();
    assert!(!stale_group_after.mem.contains(&"bootconf-stale-user".to_string()));
    assert!(keep_group_after.mem.contains(&"bootconf-stale-user".to_string()));
}

#[test]
fn test_ensure_groups_empty_vec_removes_all() {
    let status = process::Command::new("groupadd")
        .arg("bootconf-rmall-group")
        .status()
        .unwrap();
    assert!(status.success());

    let user = User {
        name: "bootconf-rmall-user".to_string(),
        uid: 9931,
        groups: Some(vec!["bootconf-rmall-group".to_string()]),
        shell: None,
        home: None,
        password: None,
        authorized_keys: None,
    };
    users::create_user(&user).unwrap();
    ensure_groups(&user).unwrap();

    let group = nix::unistd::Group::from_name("bootconf-rmall-group")
        .ok()
        .flatten()
        .unwrap();
    assert!(group.mem.contains(&"bootconf-rmall-user".to_string()));

    let user_no_groups = User {
        name: "bootconf-rmall-user".to_string(),
        uid: 9931,
        groups: Some(vec![]),
        shell: None,
        home: None,
        password: None,
        authorized_keys: None,
    };
    ensure_groups(&user_no_groups).unwrap();

    let group_after = nix::unistd::Group::from_name("bootconf-rmall-group")
        .ok()
        .flatten()
        .unwrap();
    assert!(!group_after.mem.contains(&"bootconf-rmall-user".to_string()));
}

#[test]
fn test_authorized_keys_home_dir() {
    let user = User {
        name: "bootconf-ak-home".to_string(),
        uid: 9920,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-home".to_string()),
        password: None,
        authorized_keys: Some(vec![
            "ssh-ed25519 AAAAkey1 user@host".to_string(),
            "ssh-ed25519 AAAAkey2 user@host".to_string(),
        ]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-home")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    assert!(key_path.exists());

    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 AAAAkey1 user@host"));
    assert!(content.contains("ssh-ed25519 AAAAkey2 user@host"));
}

#[test]
fn test_authorized_keys_fallback_dir() {
    let user = User {
        name: "bootconf-ak-fallback".to_string(),
        uid: 9921,
        groups: None,
        shell: None,
        home: None,
        password: None,
        authorized_keys: Some(vec!["ssh-ed25519 AAAAkey1 user@host".to_string()]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::Path::new("/")
        .join(AUTHORIZED_KEYS_DIR)
        .join("bootconf-ak-fallback");
    assert!(key_path.exists());

    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 AAAAkey1 user@host"));
}

#[test]
fn test_authorized_keys_idempotency() {
    let user = User {
        name: "bootconf-ak-idem".to_string(),
        uid: 9922,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-idem".to_string()),
        password: None,
        authorized_keys: Some(vec![
            "ssh-ed25519 AAAAkey1 user@host".to_string(),
            "ssh-ed25519 AAAAkey2 user@host".to_string(),
        ]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-idem")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    let content = fs::read_to_string(&key_path).unwrap();
    assert_eq!(content.matches("ssh-ed25519 AAAAkey1 user@host").count(), 1);
    assert_eq!(content.matches("ssh-ed25519 AAAAkey2 user@host").count(), 1);
}

#[test]
fn test_authorized_keys_additive() {
    let user1 = User {
        name: "bootconf-ak-add".to_string(),
        uid: 9923,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-add".to_string()),
        password: None,
        authorized_keys: Some(vec!["ssh-ed25519 AAAAkey1 user@host".to_string()]),
    };
    users::create_user(&user1).unwrap();
    users::ensure_authorized_keys(&user1).unwrap();

    let user2 = User {
        name: "bootconf-ak-add".to_string(),
        uid: 9923,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-add".to_string()),
        password: None,
        authorized_keys: Some(vec![
            "ssh-ed25519 AAAAkey1 user@host".to_string(),
            "ssh-ed25519 AAAAkey2 user@host".to_string(),
        ]),
    };
    users::ensure_authorized_keys(&user2).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-add")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 AAAAkey1 user@host"));
    assert!(content.contains("ssh-ed25519 AAAAkey2 user@host"));
    assert_eq!(content.matches("ssh-ed25519 AAAAkey1 user@host").count(), 1);
}

#[test]
fn test_authorized_keys_no_keys() {
    let user = make_user(
        "bootconf-ak-nokeys",
        9924,
        Some("/home/bootconf-ak-nokeys"),
        None,
    );
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-nokeys")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    assert!(!key_path.exists());
}

#[test]
fn test_authorized_keys_permissions() {
    let user = User {
        name: "bootconf-ak-perm".to_string(),
        uid: 9925,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-perm".to_string()),
        password: None,
        authorized_keys: Some(vec!["ssh-ed25519 AAAAkey1 user@host".to_string()]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-perm")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    let perms = fs::metadata(&key_path).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, AUTHORIZED_KEYS_MODE);

    let ssh_dir_path = std::path::PathBuf::from("/home/bootconf-ak-perm").join(SSH_DIR);
    let dir_perms = fs::metadata(&ssh_dir_path).unwrap().permissions();
    assert_eq!(dir_perms.mode() & 0o777, SSH_DIR_MODE);
}

#[test]
fn test_authorized_keys_fallback_additive() {
    let user1 = User {
        name: "bootconf-ak-fadd".to_string(),
        uid: 9926,
        groups: None,
        shell: None,
        home: None,
        password: None,
        authorized_keys: Some(vec!["ssh-ed25519 KEY1 host".to_string()]),
    };
    users::create_user(&user1).unwrap();
    users::ensure_authorized_keys(&user1).unwrap();

    let key_path = std::path::Path::new("/")
        .join(AUTHORIZED_KEYS_DIR)
        .join("bootconf-ak-fadd");
    assert!(key_path.exists());

    let user2 = User {
        name: "bootconf-ak-fadd".to_string(),
        uid: 9926,
        groups: None,
        shell: None,
        home: None,
        password: None,
        authorized_keys: Some(vec![
            "ssh-ed25519 KEY1 host".to_string(),
            "ssh-ed25519 KEY2 host".to_string(),
        ]),
    };
    users::ensure_authorized_keys(&user2).unwrap();

    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 KEY1 host"));
    assert!(content.contains("ssh-ed25519 KEY2 host"));
    assert_eq!(content.matches("ssh-ed25519 KEY1 host").count(), 1);
}

#[test]
fn test_authorized_keys_empty_vec() {
    let user = User {
        name: "bootconf-ak-empty".to_string(),
        uid: 9927,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-empty".to_string()),
        password: None,
        authorized_keys: Some(vec![]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-empty")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    assert!(!key_path.exists());
}

#[test]
fn test_authorized_keys_empty_vec_deletes_existing_file() {
    let user = User {
        name: "bootconf-ak-del".to_string(),
        uid: 9929,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-del".to_string()),
        password: None,
        authorized_keys: Some(vec!["ssh-ed25519 KEY1 host".to_string()]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-del")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    assert!(key_path.exists());

    let user_no_keys = User {
        name: "bootconf-ak-del".to_string(),
        uid: 9929,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-del".to_string()),
        password: None,
        authorized_keys: Some(vec![]),
    };
    users::ensure_authorized_keys(&user_no_keys).unwrap();
    assert!(!key_path.exists());
}

#[test]
fn test_authorized_keys_fallback_prunes_stale_key() {
    let user = User {
        name: "bootconf-ak-fprune".to_string(),
        uid: 9930,
        groups: None,
        shell: None,
        home: None,
        password: None,
        authorized_keys: Some(vec![
            "ssh-ed25519 KEY1 host".to_string(),
            "ssh-ed25519 KEY2 host".to_string(),
        ]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let key_path = std::path::Path::new("/")
        .join(AUTHORIZED_KEYS_DIR)
        .join("bootconf-ak-fprune");

    let user_pruned = User {
        name: "bootconf-ak-fprune".to_string(),
        uid: 9930,
        groups: None,
        shell: None,
        home: None,
        password: None,
        authorized_keys: Some(vec!["ssh-ed25519 KEY1 host".to_string()]),
    };
    users::ensure_authorized_keys(&user_pruned).unwrap();

    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 KEY1 host"));
    assert!(!content.contains("ssh-ed25519 KEY2 host"));
}

#[test]
fn test_authorized_keys_prunes_stale_key() {
    let user = User {
        name: "bootconf-ak-prune".to_string(),
        uid: 9928,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-prune".to_string()),
        password: None,
        authorized_keys: Some(vec![
            "ssh-ed25519 KEY1 host".to_string(),
            "ssh-ed25519 KEY2 host".to_string(),
        ]),
    };
    users::create_user(&user).unwrap();
    users::ensure_authorized_keys(&user).unwrap();

    let user_pruned = User {
        name: "bootconf-ak-prune".to_string(),
        uid: 9928,
        groups: None,
        shell: None,
        home: Some("/home/bootconf-ak-prune".to_string()),
        password: None,
        authorized_keys: Some(vec!["ssh-ed25519 KEY1 host".to_string()]),
    };
    users::ensure_authorized_keys(&user_pruned).unwrap();

    let key_path = std::path::PathBuf::from("/home/bootconf-ak-prune")
        .join(SSH_DIR)
        .join(AUTHORIZED_KEYS);
    let content = fs::read_to_string(&key_path).unwrap();
    assert!(content.contains("ssh-ed25519 KEY1 host"));
    assert!(!content.contains("ssh-ed25519 KEY2 host"));
}

#[test]
fn test_apply_users_config_end_to_end() {
    let toml_content = r#"
[[users]]
name = "bootconf-e2e"
uid = 9907
shell = "/bin/bash"
home = "/home/bootconf-e2e"
groups = ["users"]
authorized_keys = ["ssh-ed25519 E2EKEY test@host"]
"#;
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("users.toml");
    fs::write(&config_path, toml_content).unwrap();

    users::apply_users_config(&config_path).unwrap();

    let existing = nix::unistd::User::from_name("bootconf-e2e")
        .ok()
        .flatten()
        .unwrap();
    assert_eq!(existing.uid.as_raw(), 9907);
    assert_eq!(existing.shell, std::path::PathBuf::from("/bin/bash"));
}

#[test]
fn test_apply_users_config_invalid_toml() {
    let temp_dir = TempDir::new().unwrap();

    let config_path = temp_dir.path().join("bad.toml");
    fs::write(&config_path, "not valid toml {{{").unwrap();

    let result = users::apply_users_config(&config_path);
    assert!(result.is_err());
}

#[test]
fn test_apply_users_config_missing_file() {
    let result =
        users::apply_users_config(&std::path::PathBuf::from("/nonexistent/path/users.toml"));
    assert!(result.is_err());
}
