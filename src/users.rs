use nix::unistd;
use serde::Deserialize;
use std::fs;
use std::path;
use std::process;

pub const DEFAULT_ROOT: &str = "/";
pub const SSH_DIR: &str = ".ssh";
pub const AUTHORIZED_KEYS: &str = "authorized_keys";
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

    match unistd::User::from_name(&user.name).ok().flatten() {
        Some(_) => modify_user(user, root_path)?,
        None => create_user(user, root_path)?,
    };

    Ok(())
}

pub fn create_user(user: &User, root: &path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = process::Command::new("useradd");

    cmd.arg("-U")
        .arg("-u")
        .arg(&user.uid.to_string())
        .arg("R")
        .arg(&root.to_string_lossy().to_string());

    set_shell_home_group_args(user, &mut cmd)?;

    cmd.arg(&user.name);

    let _ = cmd.status();

    Ok(())
}

pub fn modify_user(user: &User, root: &path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = process::Command::new("usermod");

    if let Some(_) = &user.groups {
        cmd.arg("-a");
    };

    set_shell_home_group_args(user, &mut cmd)?;

    cmd.arg(&user.name);

    let _ = cmd.status();

    Ok(())
}

fn set_shell_home_group_args(
    user: &User,
    cmd: &mut process::Command,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(shell) = &user.shell {
        cmd.arg("-s").arg(shell);
    };

    if let Some(home_dir) = &user.home {
        cmd.arg("-m").arg("-d").arg(home_dir);
    };

    if let Some(groups) = &user.groups {
        cmd.arg("-G").arg(groups.join(","));
    };

    Ok(())
}
