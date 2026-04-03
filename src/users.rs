use std::path::{Path, PathBuf};

pub fn apply_users_config(file: &PathBuf, _root: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Apply users config with {}", file.display());
    Ok(())
}
