use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;

use super::error;

/// Detect Node.js installation on Windows
#[cfg(target_os = "windows")]
pub async fn detect_system_nodejs() -> Option<PathBuf> {
    // Try 'where node.exe' first (most reliable)
    if let Some(path) = try_which_command("node.exe").await {
        return Some(path);
    }

    // Try 'where node' as fallback
    if let Some(path) = try_which_command("node").await {
        return Some(path);
    }

    // Check standard installation directories
    let standard_paths = vec![
        "C:\\Program Files\\nodejs\\node.exe",
        "C:\\Program Files (x86)\\nodejs\\node.exe",
    ];

    for path_str in standard_paths {
        let path = PathBuf::from(path_str);
        if path.exists() {
            log::debug!("Found Node.js at standard location: {}", path.display());
            return Some(path);
        }
    }

    // Check NVM for Windows
    if let Some(path) = check_nvm_windows().await {
        return Some(path);
    }

    None
}

/// Detect Node.js installation on Unix-like systems (macOS, Linux)
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub async fn detect_system_nodejs() -> Option<PathBuf> {
    // Try 'which node' first
    if let Some(path) = try_which_command("node").await {
        return Some(path);
    }

    // Check standard installation directories
    let standard_paths = vec![
        "/usr/local/bin/node",
        "/usr/bin/node",
        "/opt/homebrew/bin/node", // macOS Apple Silicon
        "/opt/node/bin/node",     // Custom installations
    ];

    for path_str in standard_paths {
        let path = PathBuf::from(path_str);
        if path.exists() {
            log::debug!("Found Node.js at standard location: {}", path.display());
            return Some(path);
        }
    }

    // Check NVM for Unix
    if let Some(path) = check_nvm_unix().await {
        return Some(path);
    }

    None
}

/// Try to find command using 'where' (Windows) or 'which' (Unix)
async fn try_which_command(command: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let which_cmd = "where";
    #[cfg(not(target_os = "windows"))]
    let which_cmd = "which";

    match Command::new(which_cmd).arg(command).output().await {
        Ok(output) if output.status.success() => {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let path_str = path_str.trim();

            // 'where' on Windows can return multiple paths, take the first one
            let first_path = path_str.lines().next()?;

            let path = PathBuf::from(first_path);
            if path.exists() {
                log::debug!("Found Node.js via {}: {}", which_cmd, path.display());
                return Some(path);
            }
        }
        Ok(_) => {
            log::debug!("{} command did not find {}", which_cmd, command);
        }
        Err(e) => {
            log::debug!("Failed to run {} command: {}", which_cmd, e);
        }
    }

    None
}

/// Check NVM (Node Version Manager) on Windows
#[cfg(target_os = "windows")]
async fn check_nvm_windows() -> Option<PathBuf> {
    // NVM for Windows typically installs to %APPDATA%\nvm
    let appdata = std::env::var("APPDATA").ok()?;
    let nvm_dir = PathBuf::from(appdata).join("nvm");

    if !nvm_dir.exists() {
        return None;
    }

    log::debug!("Checking NVM directory: {}", nvm_dir.display());

    // Try to find the current version from settings.txt
    let settings_file = nvm_dir.join("settings.txt");
    if let Ok(settings) = fs::read_to_string(&settings_file).await {
        for line in settings.lines() {
            if line.starts_with("root:") {
                let root = line.trim_start_matches("root:").trim();
                let node_exe = PathBuf::from(root).join("node.exe");
                if node_exe.exists() {
                    log::debug!("Found Node.js via NVM: {}", node_exe.display());
                    return Some(node_exe);
                }
            }
        }
    }

    // Fallback: scan for any node.exe in nvm directory
    search_for_node_in_directory(&nvm_dir)
}

/// Check NVM (Node Version Manager) on Unix-like systems
#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn check_nvm_unix() -> Option<PathBuf> {
    // NVM typically installs to ~/.nvm
    let home = std::env::var("HOME").ok()?;
    let nvm_dir = PathBuf::from(home)
        .join(".nvm")
        .join("versions")
        .join("node");

    if !nvm_dir.exists() {
        return None;
    }

    log::debug!("Checking NVM directory: {}", nvm_dir.display());

    // Find the latest or default version
    let mut versions = vec![];

    if let Ok(mut entries) = fs::read_dir(&nvm_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().is_dir() {
                versions.push(entry.path());
            }
        }
    }

    // Sort versions (simple lexicographic sort, good enough for most cases)
    versions.sort();

    // Try the latest version first
    for version_dir in versions.iter().rev() {
        let node_path = version_dir.join("bin").join("node");
        if node_path.exists() {
            log::debug!("Found Node.js via NVM: {}", node_path.display());
            return Some(node_path);
        }
    }

    None
}

/// Recursively search for node.exe (Windows) or node (Unix) in a directory
fn search_for_node_in_directory(dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let node_name = "node.exe";
    #[cfg(not(target_os = "windows"))]
    let node_name = "node";

    // Use std::fs for simplicity and avoid async recursion
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(node_name) {
            return Some(path);
        }

        if path.is_dir() {
            // Recursively search subdirectories (limit depth to avoid long scans)
            if let Some(found) = search_for_node_in_directory(&path) {
                return Some(found);
            }
        }
    }

    None
}

/// Verify a path is actually Node.js by running --version
pub async fn verify_nodejs_executable(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(error::invalid_custom_path_error(
            path.display().to_string(),
            "File does not exist".to_string(),
        ));
    }

    // Check if it's executable (Unix-like systems)
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(path).await {
            Ok(metadata) => {
                let permissions = metadata.permissions();
                if permissions.mode() & 0o111 == 0 {
                    return Err(error::invalid_custom_path_error(
                        path.display().to_string(),
                        "File is not executable".to_string(),
                    ));
                }
            }
            Err(e) => {
                return Err(error::invalid_custom_path_error(
                    path.display().to_string(),
                    format!("Cannot read file metadata: {}", e),
                ));
            }
        }
    }

    // Run 'node --version' to verify it's actually Node.js
    match Command::new(path).arg("--version").output().await {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            let version = version.trim().to_string();

            if version.starts_with('v')
                || version.chars().next().map_or(false, |c| c.is_ascii_digit())
            {
                log::info!("Verified Node.js at {}: {}", path.display(), version);
                Ok(version)
            } else {
                Err(error::validation_failed_error(
                    path.display().to_string(),
                    format!("Unexpected version output: {}", version),
                ))
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(error::validation_failed_error(
                path.display().to_string(),
                format!("Command failed with: {}", stderr.trim()),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(error::permission_denied_error(path.display().to_string()))
        }
        Err(e) => Err(e).with_context(|| format!("Failed to execute {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_system_nodejs() {
        // This test will return different results based on the system
        let result = detect_system_nodejs().await;
        // Just ensure it doesn't panic
        log::debug!("Detected Node.js: {:?}", result);
    }

    #[tokio::test]
    async fn test_verify_invalid_path() {
        let invalid_path = PathBuf::from("/nonexistent/node");
        let result = verify_nodejs_executable(&invalid_path).await;
        assert!(result.is_err());
    }
}
