use std::path;

pub fn apply_users_config(dir: &Option<path::PathBuf>) {
    let Some(dir) = dir else {
        return;
    };

    println!("Apply users config with {}", dir.display());
}
