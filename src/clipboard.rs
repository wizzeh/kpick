use std::io::Write;
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClipboardError {
    #[error("Clipboard error: {0}")]
    Io(#[from] std::io::Error),
    #[error("wl-copy not found - please install wl-clipboard")]
    WlCopyNotFound,
}

/// Copy text to clipboard using wl-copy, clearing after timeout
pub fn copy_with_clear(text: &str, clear_after_secs: u64) -> Result<(), ClipboardError> {
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ClipboardError::WlCopyNotFound
            } else {
                ClipboardError::Io(e)
            }
        })?;

    // Write text and close stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }

    // Wait for wl-copy to fork its daemon
    child.wait()?;

    // Spawn background process to clear after timeout
    let escaped = text.replace('\'', "'\"'\"'");
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"sleep {} && [ "$(wl-paste -n 2>/dev/null)" = '{}' ] && wl-copy --clear"#,
            clear_after_secs, escaped,
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

// Tests require a running Wayland session with wl-clipboard installed
// Run manually: cargo test clipboard -- --ignored
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires interactive Wayland session
    fn test_copy_spawns_without_error() {
        copy_with_clear("test_password", 60).expect("Failed to spawn wl-copy");
    }
}
