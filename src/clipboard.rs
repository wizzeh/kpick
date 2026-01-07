use std::io::{Read, Write};
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

    // Spawn a child process to clear clipboard after timeout.
    // We re-exec ourselves with --internal-clipboard-clear because threads die
    // when the main process exits, but child processes survive.
    let exe = std::env::current_exe().map_err(ClipboardError::Io)?;
    let mut clear_child = Command::new(exe)
        .arg("--internal-clipboard-clear")
        .arg(clear_after_secs.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(ClipboardError::Io)?;

    // Pass the expected clipboard content via stdin (avoids shell injection)
    if let Some(mut stdin) = clear_child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }

    // Don't wait - let the child process outlive us

    Ok(())
}

/// Daemon entry point: sleep, check clipboard, clear if unchanged.
/// Called when invoked with --internal-clipboard-clear.
pub fn run_clear_daemon(timeout_secs: u64) {
    // Read expected value from stdin
    let mut expected = String::new();
    if std::io::stdin().read_to_string(&mut expected).is_err() {
        return;
    }

    // Sleep for the timeout
    thread::sleep(Duration::from_secs(timeout_secs));

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
                .status();
        }
    }
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
