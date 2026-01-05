use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use thiserror::Error;

use super::crypto::{generate_client_id, generate_nonce, SessionKeys};
use super::protocol::*;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("KeePassXC is not running (socket not found)")]
    NotRunning,
    #[error("Database is locked")]
    DatabaseLocked,
    #[error("Not associated with database")]
    NotAssociated,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Crypto error: {0}")]
    Crypto(#[from] super::crypto::CryptoError),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

pub struct KeePassXCClient {
    stream: UnixStream,
    session_keys: SessionKeys,
    client_id: String,
}

impl KeePassXCClient {
    /// Find the KeePassXC socket path
    fn socket_path() -> Option<PathBuf> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;

        // Try paths in order of preference
        let candidates = [
            format!(
                "{}/app/org.keepassxc.KeePassXC/org.keepassxc.KeePassXC.BrowserServer",
                runtime_dir
            ),
            format!("{}/org.keepassxc.KeePassXC.BrowserServer", runtime_dir),
            format!("{}/kpxc_server", runtime_dir),
        ];

        for path in candidates {
            let p = PathBuf::from(&path);
            if p.exists() {
                return Some(p);
            }
        }

        // Fallback
        let fallback = PathBuf::from("/tmp/org.keepassxc.KeePassXC.BrowserServer");
        if fallback.exists() {
            return Some(fallback);
        }

        None
    }

    /// Connect to KeePassXC and perform key exchange
    pub fn connect() -> Result<Self, ClientError> {
        let socket_path = Self::socket_path().ok_or(ClientError::NotRunning)?;
        let stream = UnixStream::connect(&socket_path)?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

        let session_keys = SessionKeys::generate();
        let client_id = generate_client_id();

        let mut client = Self {
            stream,
            session_keys,
            client_id,
        };

        client.change_public_keys()?;
        Ok(client)
    }

    fn send(&mut self, msg: &Message) -> Result<(), ClientError> {
        let json = serde_json::to_string(msg)?;
        writeln!(self.stream, "{}", json)?;
        self.stream.flush()?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Message, ClientError> {
        let mut reader = BufReader::new(&self.stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let msg: Message = serde_json::from_str(&line)?;
        Ok(msg)
    }

    fn change_public_keys(&mut self) -> Result<(), ClientError> {
        let msg = Message {
            action: "change-public-keys".to_string(),
            public_key: Some(self.session_keys.client_public_key_b64()),
            nonce: Some(generate_nonce()),
            client_id: Some(self.client_id.clone()),
            message: None,
            success: None,
            version: None,
            error: None,
            error_code: None,
        };

        self.send(&msg)?;
        let resp = self.recv()?;

        if resp.success.as_deref() != Some("true") {
            return Err(ClientError::Protocol(
                resp.error
                    .unwrap_or_else(|| "Key exchange failed".to_string()),
            ));
        }

        let server_public = resp
            .public_key
            .ok_or_else(|| ClientError::Protocol("No public key in response".to_string()))?;
        self.session_keys.set_server_public_key(&server_public)?;

        Ok(())
    }

    /// Get the client ID for this session
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}
