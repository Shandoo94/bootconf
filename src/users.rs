use nix::unistd;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path;
use std::process;

pub const SSH_DIR: &str = ".ssh";
pub const AUTHORIZED_KEYS: &str = "authorized_keys";
pub const AUTHORIZED_KEYS_DIR: &str = "etc/ssh/authorized_keys.d";
pub const SSH_DIR_MODE: u32 = 0o700;
pub const AUTHORIZED_KEYS_MODE: u32 = 0o600;

#[derive(Deserialize, Debug)]
pub struct UsersConfig {
    pub users: Vec<User>,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub name: String,
    pub uid: u32,
    pub groups: Option<Vec<String>>,
    pub shell: Option<String>,
    pub home: Option<String>,
    pub password: Option<String>,
    pub authorized_keys: Option<Vec<String>>,
}

pub fn apply_users_config(file: &path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file)?;
    let config: UsersConfig = toml::from_str(&content)?;

    for user in &config.users {
        apply_user(user)?;
    }

    Ok(())
}

pub fn apply_user(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let existing = match unistd::User::from_name(&user.name).ok().flatten() {
        Some(u) => u,
        None => {
            create_user(user)?;
            unistd::User::from_name(&user.name)
                .ok()
                .flatten()
                .ok_or(format!("user '{}' not found after creation", user.name))?
        }
    };

    ensure_home(user, &existing)?;
    ensure_shell(user, &existing)?;
    ensure_groups(user)?;
    ensure_password(user, &existing)?;
    ensure_authorized_keys(user)?;

    Ok(())
}

pub fn create_user(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let status = process::Command::new("useradd")
        .arg("-U")
        .arg("-u")
        .arg(&user.uid.to_string())
        .arg(&user.name)
        .status()?;

    if !status.success() {
        return Err(format!("useradd failed for '{}'", user.name).into());
    }

    Ok(())
}

pub fn ensure_home(user: &User, existing: &unistd::User) -> Result<(), Box<dyn std::error::Error>> {
    let desired = match &user.home {
        Some(home) => home,
        None => return Ok(()),
    };

    if existing.dir == path::PathBuf::from(desired) {
        return Ok(());
    }

    let status = process::Command::new("usermod")
        .arg("-d")
        .arg(desired)
        .arg("-m")
        .arg(&user.name)
        .status()?;

    if !status.success() {
        return Err(format!("usermod failed setting home for '{}'", user.name).into());
    }

    Ok(())
}

pub fn ensure_shell(
    user: &User,
    existing: &unistd::User,
) -> Result<(), Box<dyn std::error::Error>> {
    let desired = match &user.shell {
        Some(shell) => shell,
        None => return Ok(()),
    };

    if existing.shell == path::PathBuf::from(desired) {
        return Ok(());
    }

    let status = process::Command::new("usermod")
        .arg("-s")
        .arg(desired)
        .arg(&user.name)
        .status()?;

    if !status.success() {
        return Err(format!("usermod failed setting shell for '{}'", user.name).into());
    }

    Ok(())
}

pub fn ensure_groups(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let desired_groups = match &user.groups {
        Some(groups) => groups,
        None => return Ok(()),
    };

    let desired_set: HashSet<&str> = desired_groups.iter().map(|s| s.as_str()).collect();

    for group_name in desired_groups {
        let group = match unistd::Group::from_name(group_name).ok().flatten() {
            Some(g) => g,
            None => continue,
        };

        if group.mem.contains(&user.name) {
            continue;
        }

        let status = process::Command::new("usermod")
            .arg("-a")
            .arg("-G")
            .arg(group_name)
            .arg(&user.name)
            .status()?;

        if !status.success() {
            return Err(format!(
                "usermod failed adding '{}' to group '{}'",
                user.name, group_name
            )
            .into());
        }
    }

    let primary_gid = unistd::User::from_name(&user.name)
        .ok()
        .flatten()
        .map(|u| u.gid);

    let current_groups = get_user_supplementary_groups(&user.name)?;

    for group_name in &current_groups {
        if desired_set.contains(group_name.as_str()) {
            continue;
        }

        let group = match unistd::Group::from_name(group_name).ok().flatten() {
            Some(g) => g,
            None => continue,
        };

        if primary_gid == Some(group.gid) {
            continue;
        }

        let status = process::Command::new("gpasswd")
            .arg("-d")
            .arg(&user.name)
            .arg(group_name)
            .status()?;

        if !status.success() {
            return Err(format!(
                "gpasswd failed removing '{}' from group '{}'",
                user.name, group_name
            )
            .into());
        }
    }

    Ok(())
}

fn get_user_supplementary_groups(name: &str) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("/etc/group")?;
    let mut groups = HashSet::new();

    for line in content.lines() {
        let mut parts = line.splitn(4, ':');
        let group_name = match parts.next() {
            Some(n) => n,
            None => continue,
        };
        let _ = parts.next();
        let _ = parts.next();
        let members = parts.next().unwrap_or("");

        if members.split(',').any(|m| m.trim() == name) {
            groups.insert(group_name.to_string());
        }
    }

    Ok(groups)
}

pub fn ensure_password(
    user: &User,
    existing: &unistd::User,
) -> Result<(), Box<dyn std::error::Error>> {
    let desired = match &user.password {
        Some(hash) => hash,
        None => return Ok(()),
    };

    if *desired == existing.passwd.to_string_lossy() {
        return Ok(());
    }

    if existing.passwd.to_string_lossy() == "x" {
        if let Some(current_hash) = read_shadow_hash(&user.name)? {
            if *desired == current_hash {
                return Ok(());
            }
        }
    }

    set_passwd(&user.name, desired)?;

    Ok(())
}

fn read_shadow_hash(name: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("/etc/shadow")?;
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        if parts.next() == Some(name) {
            return Ok(parts.next().map(String::from));
        }
    }
    Ok(None)
}

pub fn read_key_set(path: &path::Path) -> HashSet<String> {
    fs::read_to_string(path)
        .map(|content| {
            content
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn ensure_authorized_keys(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let keys = match &user.authorized_keys {
        Some(keys) => keys,
        None => return Ok(()),
    };

    let target_path = match &user.home {
        Some(home) => path::PathBuf::from(home)
            .join(SSH_DIR)
            .join(AUTHORIZED_KEYS),
        None => path::Path::new("/")
            .join(AUTHORIZED_KEYS_DIR)
            .join(&user.name),
    };

    let desired_set: HashSet<String> = keys.iter().cloned().collect();

    if desired_set.is_empty() {
        if target_path.exists() {
            fs::remove_file(&target_path)?;
            if user.home.is_some() {
                let _ = fs::remove_dir(target_path.parent().unwrap());
            }
        }
        return Ok(());
    }

    let existing_set = read_key_set(&target_path);

    if existing_set == desired_set {
        return Ok(());
    }

    let parent = target_path.parent().unwrap();
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(SSH_DIR_MODE))?;

    if user.home.is_some() {
        unistd::chown(
            parent,
            Some(unistd::Uid::from_raw(user.uid)),
            Some(unistd::Gid::from_raw(user.uid)),
        )?;
    }

    let content = keys
        .iter()
        .map(|k| k.as_str())
        .collect::<Vec<&str>>()
        .join("\n")
        + "\n";

    fs::write(&target_path, &content)?;
    fs::set_permissions(
        &target_path,
        fs::Permissions::from_mode(AUTHORIZED_KEYS_MODE),
    )?;

    if user.home.is_some() {
        unistd::chown(
            &target_path,
            Some(unistd::Uid::from_raw(user.uid)),
            Some(unistd::Gid::from_raw(user.uid)),
        )?;
    }

    Ok(())
}

pub fn set_passwd(name: &str, hash: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = process::Command::new("chpasswd");

    cmd.arg("-e").stdin(process::Stdio::piped());

    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().ok_or("Failed to open stdin")?;
    write!(stdin, "{}:{}", name, hash)?;
    drop(stdin);

    child.wait()?;

    Ok(())
}
