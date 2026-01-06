use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database file not found: {0}")]
    NotFound(String),
    #[error("Failed to open database: {0}")]
    OpenFailed(String),
    #[error("Invalid password")]
    InvalidPassword,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A password entry from the database
#[derive(Debug, Clone)]
pub struct Entry {
    pub title: String,
    pub username: String,
    pub password: String,
    pub uuid: String,
    #[allow(dead_code)]
    pub group: String,
}

/// Open a KDBX database and return all entries
pub fn open_database(path: &Path, password: &str) -> Result<Vec<Entry>, DatabaseError> {
    use keepass::{Database, DatabaseKey};

    if !path.exists() {
        return Err(DatabaseError::NotFound(path.display().to_string()));
    }

    let key = DatabaseKey::new().with_password(password);

    let db = Database::open(&mut std::fs::File::open(path)?, key)
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("Invalid credentials") || msg.contains("decryption") || msg.contains("password") {
                DatabaseError::InvalidPassword
            } else {
                DatabaseError::OpenFailed(msg)
            }
        })?;

    let mut entries = Vec::new();
    collect_entries(&db.root, "", &mut entries);

    Ok(entries)
}

/// Recursively collect entries from groups
fn collect_entries(group: &keepass::db::Group, path: &str, entries: &mut Vec<Entry>) {
    let current_path = if path.is_empty() {
        group.name.clone()
    } else {
        format!("{}/{}", path, group.name)
    };

    for node in &group.children {
        match node {
            keepass::db::Node::Group(g) => {
                collect_entries(g, &current_path, entries);
            }
            keepass::db::Node::Entry(e) => {
                let title = e.get_title().unwrap_or_default().to_string();
                let username = e.get_username().unwrap_or_default().to_string();
                let password = e.get_password().unwrap_or_default().to_string();
                let uuid = format!("{:?}", e.uuid);

                entries.push(Entry {
                    title,
                    username,
                    password,
                    uuid,
                    group: current_path.clone(),
                });
            }
        }
    }
}

/// Prompt for password securely (hidden input)
/// If KPICK_PASSWORD env var is set, use that instead (for testing)
pub fn prompt_password(prompt: &str) -> Result<String, std::io::Error> {
    if let Ok(pass) = std::env::var("KPICK_PASSWORD") {
        return Ok(pass);
    }
    eprint!("{}", prompt);
    rpassword::read_password()
}
