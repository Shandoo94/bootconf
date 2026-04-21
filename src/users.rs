use nix::unistd;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path;
use std::process;

pub const DEFAULT_ROOT: &str = "/";
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

pub fn apply_users_config(
    file: &path::PathBuf,
    root: Option<&path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file)?;
    let config: UsersConfig = toml::from_str(&content)?;

    for user in &config.users {
        apply_user(user, root)?;
    }

    Ok(())
}

pub fn apply_user(
    user: &User,
    root: Option<&path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root_path = root.unwrap_or(path::Path::new(DEFAULT_ROOT));

    let existing = match unistd::User::from_name(&user.name).ok().flatten() {
        Some(u) => u,
        None => {
            create_user(user, root_path)?;
            unistd::User::from_name(&user.name)
                .ok()
                .flatten()
                .ok_or(format!("user '{}' not found after creation", user.name))?
        }
    };

    ensure_home(user, &existing, root_path)?;
    ensure_shell(user, &existing, root_path)?;
    ensure_groups(user, root_path)?;
    ensure_password(user, &existing, root_path)?;
    ensure_authorized_keys(user, root_path)?;

    Ok(())
}

pub fn create_user(user: &User, root: &path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = process::Command::new("useradd")
        .arg("-U")
        .arg("-u")
        .arg(&user.uid.to_string())
        .arg("-R")
        .arg(&root.to_string_lossy().to_string())
        .arg(&user.name)
        .status()?;

    if !status.success() {
        return Err(format!("useradd failed for '{}'", user.name).into());
    }

    Ok(())
}

fn ensure_home(
    user: &User,
    existing: &unistd::User,
    root: &path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
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
        .arg("-R")
        .arg(&root.to_string_lossy().to_string())
        .arg(&user.name)
        .status()?;

    if !status.success() {
        return Err(format!("usermod failed setting home for '{}'", user.name).into());
    }

    Ok(())
}

fn ensure_shell(
    user: &User,
    existing: &unistd::User,
    root: &path::Path,
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
        .arg("-R")
        .arg(&root.to_string_lossy().to_string())
        .arg(&user.name)
        .status()?;

    if !status.success() {
        return Err(format!("usermod failed setting shell for '{}'", user.name).into());
    }

    Ok(())
}

fn ensure_groups(
    user: &User,
    root: &path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let desired_groups = match &user.groups {
        Some(groups) => groups,
        None => return Ok(()),
    };

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
            .arg("-R")
            .arg(&root.to_string_lossy().to_string())
            .arg(&user.name)
            .status()?;

        if !status.success() {
            return Err(
                format!("usermod failed adding '{}' to group '{}'", user.name, group_name).into(),
            );
        }
    }

    Ok(())
}

fn ensure_password(
    user: &User,
    existing: &unistd::User,
    root: &path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let desired = match &user.password {
        Some(hash) => hash,
        None => return Ok(()),
    };

    if *desired == existing.passwd.to_string_lossy() {
        return Ok(());
    }

    set_passwd(&user.name, desired, root)?;

    Ok(())
}

fn read_key_set(path: &path::Path) -> HashSet<String> {
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

pub fn ensure_authorized_keys(
    user: &User,
    root: &path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let keys = match &user.authorized_keys {
        Some(keys) if !keys.is_empty() => keys,
        _ => return Ok(()),
    };

    let target_path = match &user.home {
        Some(home) => root
            .join(home.trim_start_matches('/'))
            .join(SSH_DIR)
            .join(AUTHORIZED_KEYS),
        None => root.join(AUTHORIZED_KEYS_DIR).join(&user.name),
    };

    let parent = target_path.parent().unwrap();
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(SSH_DIR_MODE))?;

    #[cfg(not(test))]
    if user.home.is_some() {
        unistd::chown(
            parent,
            Some(unistd::Uid::from_raw(user.uid)),
            Some(unistd::Gid::from_raw(user.uid)),
        )?;
    }

    let existing = read_key_set(&target_path);
    let new_keys: Vec<&String> = keys.iter().filter(|k| !existing.contains(*k)).collect();

    if !new_keys.is_empty() {
        let mut content: String = fs::read_to_string(&target_path).unwrap_or_default();

        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }

        for key in &new_keys {
            content.push_str(key);
            content.push('\n');
        }

        fs::write(&target_path, &content)?;
        fs::set_permissions(
            &target_path,
            fs::Permissions::from_mode(AUTHORIZED_KEYS_MODE),
        )?;

        #[cfg(not(test))]
        if user.home.is_some() {
            unistd::chown(
                &target_path,
                Some(unistd::Uid::from_raw(user.uid)),
                Some(unistd::Gid::from_raw(user.uid)),
            )?;
        }
    }

    Ok(())
}

fn set_passwd(
    name: &str,
    hash: &str,
    root: &path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = process::Command::new("chpasswd");

    cmd.arg("-e")
        .arg("-R")
        .arg(root.to_string_lossy().to_string())
        .stdin(process::Stdio::piped());

    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().ok_or("Failed to open stdin")?;
    write!(stdin, "{}:{}", name, hash)?;
    drop(stdin);

    child.wait()?;

    Ok(())
}
