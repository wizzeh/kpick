use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
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

    // Skip auto-clear if timeout is 0
    if clear_after_secs == 0 {
        return Ok(());
    }

    // Spawn thread to clear clipboard after timeout
    // This avoids shell injection by never using shell escaping
    let expected = text.to_string();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(clear_after_secs));

        // Read current clipboard content
        let output = Command::new("wl-paste")
            .arg("-n")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output {
            let current = String::from_utf8_lossy(&output.stdout);
            // Only clear if clipboard still contains our text
            if current == expected {
                let _ = Command::new("wl-copy")
                    .arg("--clear")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        }
    });

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
