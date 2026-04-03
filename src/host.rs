use std::path;

pub fn apply_host_config(dir: &Option<path::PathBuf>) {
    let Some(dir) = dir else {
        return;
    };

    println!("Apply host config with {}", dir.display());
}
