use color_eyre::eyre::{Result, eyre};
use std::{collections::HashMap, ffi::OsStr, fs, path::PathBuf};

/// Attempts to find the first file with a particular extension
/// in the same directory as the running binary. Returns Ok(None) if none found.
pub(crate) fn find_first_extension_file_in_exe_dir(extension: &str) -> Result<PathBuf> {
    let mut exe_dir = std::env::current_exe()?;
    exe_dir.pop();

    if let Ok(read_dir) = fs::read_dir(&exe_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension() == Some(OsStr::new(extension)) {
                return Ok(path)
            }
        }
    }
    Err(eyre!(format!("No .{} file found in {:?}", extension, exe_dir)))
}

/// Loads a CSV of `Error Code,Name` rows (with a header line) into a lookup map.
pub(crate) fn load_error_map(path: PathBuf) -> Result<HashMap<u32, String>> {
    let data = fs::read_to_string(path)?;
    let mut map = HashMap::new();

    // skips a header
    for line in data.lines().skip(1) {
        if let Some((code, name)) = line.trim().split_once(',') {
            map.insert(code.trim().parse()?, name.trim().to_string());
        }
    }

    Ok(map)
}
