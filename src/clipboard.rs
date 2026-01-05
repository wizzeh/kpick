use arboard::Clipboard;
use std::thread;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClipboardError {
    #[error("Clipboard error: {0}")]
    Arboard(#[from] arboard::Error),
}

/// Copy text to clipboard and clear after timeout
pub fn copy_with_clear(text: &str, clear_after_secs: u64) -> Result<(), ClipboardError> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;

    // Spawn thread to clear clipboard after timeout
    let text_copy = text.to_string();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(clear_after_secs));
        if let Ok(mut clipboard) = Clipboard::new() {
            // Only clear if clipboard still contains our text
            if let Ok(current) = clipboard.get_text() {
                if current == text_copy {
                    let _ = clipboard.set_text("");
                }
            }
        }
    });

    Ok(())
}
