//! Data directories derived from the checkout in development and the OS in packaged builds.

use std::path::{Path, PathBuf};

use tauri::Manager;

/// Resolves a root without changing existing development data or the process working directory.
pub(super) fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        development(Path::new(env!("CARGO_MANIFEST_DIR")))
    } else {
        app.path()
            .app_local_data_dir()
            .map_err(|error| format!("cannot resolve application data directory: {error}"))
    }
}

/// Keeps the checkout-relative data layout stable when the repository is moved.
fn development(manifest: &Path) -> Result<PathBuf, String> {
    manifest
        .ancestors()
        .nth(3)
        .map(|root| root.join("data"))
        .ok_or_else(|| "native manifest is outside the workspace layout".into())
}

/// Creates a named application-owned subdirectory and propagates filesystem failures.
pub(super) fn directory(app: &tauri::AppHandle, name: &str) -> Result<PathBuf, String> {
    let directory = root(app)?.join(name);
    std::fs::create_dir_all(&directory).map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_layout_is_independent_of_drive_and_working_directory() {
        assert_eq!(development(Path::new("checkout/apps/windows/native")).unwrap(), PathBuf::from("checkout/data"));
        assert!(development(Path::new("native")).is_err());
    }
}
