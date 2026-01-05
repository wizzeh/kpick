# kpick Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a wofi-style password launcher that queries KeePassXC via browser integration protocol and copies passwords to clipboard.

**Architecture:** Layer-shell overlay window (smithay-client-toolkit + egui rendered via softbuffer/tiny-skia), KeePassXC client using NaCl-encrypted JSON over Unix socket, fuzzy search with frecency ranking.

**Tech Stack:** Rust, smithay-client-toolkit, egui, tiny-skia, softbuffer, crypto_box, nucleo, arboard, serde_json, directories

---

## Task 1: Project Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

**Step 1: Create Cargo.toml with dependencies**

```toml
[package]
name = "kpick"
version = "0.1.0"
edition = "2021"

[dependencies]
# UI
smithay-client-toolkit = { version = "0.19", default-features = false, features = ["calloop"] }
egui = "0.29"
tiny-skia = "0.11"
softbuffer = "0.4"

# KeePassXC protocol
crypto_box = "0.9"
rand = "0.8"
base64 = "0.22"

# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
nucleo-matcher = "0.3"
arboard = { version = "3.4", features = ["wayland-data-control"] }
directories = "5.0"
thiserror = "2.0"

# Async/runtime
calloop = "0.14"
```

**Step 2: Create minimal main.rs**

```rust
fn main() {
    println!("kpick - KeePassXC password picker");
}
```

**Step 3: Verify project compiles**

Run: `cargo build`
Expected: Compiles successfully (dependencies download)

**Step 4: Commit**

```bash
git add Cargo.toml src/main.rs
git commit -m "feat: initialize kpick project with dependencies"
```

---

## Task 2: KeePassXC Protocol Types

**Files:**
- Create: `src/keepassxc/mod.rs`
- Create: `src/keepassxc/protocol.rs`
- Modify: `src/main.rs`

**Step 1: Create module structure**

Create `src/keepassxc/mod.rs`:
```rust
pub mod protocol;

pub use protocol::*;
```

Update `src/main.rs`:
```rust
mod keepassxc;

fn main() {
    println!("kpick - KeePassXC password picker");
}
```

**Step 2: Define protocol message types**

Create `src/keepassxc/protocol.rs`:
```rust
use serde::{Deserialize, Serialize};

/// Unencrypted wrapper sent over the socket
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    // Response fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

/// Encrypted payload for change-public-keys (inside message field)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePublicKeysRequest {
    pub action: String,
    pub public_key: String,
    pub nonce: String,
    pub client_id: String,
}

/// Encrypted payload for associate
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociateRequest {
    pub action: String,
    pub key: String,
    pub id_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociateResponse {
    pub hash: Option<String>,
    pub version: Option<String>,
    pub id: Option<String>,
    pub nonce: Option<String>,
    pub success: Option<String>,
    pub error: Option<String>,
}

/// Encrypted payload for test-associate
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAssociateRequest {
    pub action: String,
    pub id: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAssociateResponse {
    pub success: Option<String>,
    pub error: Option<String>,
    pub hash: Option<String>,
    pub version: Option<String>,
    pub id: Option<String>,
}

/// Encrypted payload for get-logins
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLoginsRequest {
    pub action: String,
    pub url: String,
    pub keys: Vec<DatabaseKey>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseKey {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginEntry {
    pub name: String,
    pub login: String,
    pub password: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub group: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLoginsResponse {
    pub success: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub entries: Vec<LoginEntry>,
    pub count: Option<i32>,
    pub hash: Option<String>,
    pub version: Option<String>,
}
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/keepassxc/
git commit -m "feat: add KeePassXC protocol message types"
```

---

## Task 3: KeePassXC Crypto Layer

**Files:**
- Create: `src/keepassxc/crypto.rs`
- Modify: `src/keepassxc/mod.rs`

**Step 1: Implement encryption helpers**

Create `src/keepassxc/crypto.rs`:
```rust
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use crypto_box::{
    aead::{Aead, AeadCore, OsRng},
    PublicKey, SalsaBox, SecretKey,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Failed to decode base64: {0}")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("Invalid key length")]
    InvalidKeyLength,
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
}

/// Holds our keypair and the server's public key for a session
pub struct SessionKeys {
    pub client_secret: SecretKey,
    pub client_public: PublicKey,
    pub server_public: Option<PublicKey>,
}

impl SessionKeys {
    pub fn generate() -> Self {
        let client_secret = SecretKey::generate(&mut OsRng);
        let client_public = client_secret.public_key();
        Self {
            client_secret,
            client_public,
            server_public: None,
        }
    }

    pub fn client_public_key_b64(&self) -> String {
        BASE64.encode(self.client_public.as_bytes())
    }

    pub fn set_server_public_key(&mut self, b64: &str) -> Result<(), CryptoError> {
        let bytes = BASE64.decode(b64)?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        self.server_public = Some(PublicKey::from(key_bytes));
        Ok(())
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(String, String), CryptoError> {
        let server_pk = self
            .server_public
            .as_ref()
            .ok_or(CryptoError::EncryptionFailed)?;
        let salsa_box = SalsaBox::new(server_pk, &self.client_secret);
        let nonce = SalsaBox::generate_nonce(&mut OsRng);
        let ciphertext = salsa_box
            .encrypt(&nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;
        Ok((BASE64.encode(&ciphertext), BASE64.encode(&nonce)))
    }

    pub fn decrypt(&self, ciphertext_b64: &str, nonce_b64: &str) -> Result<Vec<u8>, CryptoError> {
        let server_pk = self
            .server_public
            .as_ref()
            .ok_or(CryptoError::DecryptionFailed)?;
        let salsa_box = SalsaBox::new(server_pk, &self.client_secret);
        let ciphertext = BASE64.decode(ciphertext_b64)?;
        let nonce_bytes = BASE64.decode(nonce_b64)?;
        let nonce: [u8; 24] = nonce_bytes
            .try_into()
            .map_err(|_| CryptoError::DecryptionFailed)?;
        let nonce = crypto_box::Nonce::from(nonce);
        salsa_box
            .decrypt(&nonce, ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)
    }
}

/// Generate a random client ID (24 bytes, base64 encoded)
pub fn generate_client_id() -> String {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).expect("Failed to generate random bytes");
    BASE64.encode(bytes)
}

/// Generate a random nonce (24 bytes, base64 encoded)
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).expect("Failed to generate random bytes");
    BASE64.encode(bytes)
}
```

**Step 2: Update mod.rs**

Update `src/keepassxc/mod.rs`:
```rust
pub mod crypto;
pub mod protocol;

pub use crypto::*;
pub use protocol::*;
```

**Step 3: Add getrandom dependency**

Update `Cargo.toml` dependencies:
```toml
getrandom = "0.2"
```

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add src/keepassxc/crypto.rs src/keepassxc/mod.rs Cargo.toml
git commit -m "feat: add KeePassXC crypto layer for NaCl encryption"
```

---

## Task 4: KeePassXC Client - Connection

**Files:**
- Create: `src/keepassxc/client.rs`
- Modify: `src/keepassxc/mod.rs`

**Step 1: Implement socket connection and key exchange**

Create `src/keepassxc/client.rs`:
```rust
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
            format!("{}/app/org.keepassxc.KeePassXC/org.keepassxc.KeePassXC.BrowserServer", runtime_dir),
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
                resp.error.unwrap_or_else(|| "Key exchange failed".to_string()),
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
```

**Step 2: Update mod.rs**

Update `src/keepassxc/mod.rs`:
```rust
pub mod client;
pub mod crypto;
pub mod protocol;

pub use client::*;
pub use crypto::*;
pub use protocol::*;
```

**Step 3: Test connection in main.rs**

Update `src/main.rs`:
```rust
mod keepassxc;

use keepassxc::KeePassXCClient;

fn main() {
    println!("kpick - KeePassXC password picker");

    match KeePassXCClient::connect() {
        Ok(client) => {
            println!("Connected to KeePassXC! Client ID: {}", client.client_id());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
```

**Step 4: Test manually**

Run: `cargo run`
Expected: Either "Connected to KeePassXC!" or "Error: KeePassXC is not running"

**Step 5: Commit**

```bash
git add src/
git commit -m "feat: add KeePassXC client with socket connection and key exchange"
```

---

## Task 5: KeePassXC Client - Association

**Files:**
- Modify: `src/keepassxc/client.rs`
- Create: `src/config.rs`
- Modify: `src/main.rs`

**Step 1: Add config module for storing association**

Create `src/config.rs`:
```rust
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Could not determine config directory")]
    NoConfigDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Association {
    pub id: String,
    pub id_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub association: Option<Association>,
}

impl Config {
    fn data_dir() -> Result<PathBuf, ConfigError> {
        ProjectDirs::from("", "", "kpick")
            .map(|p| p.data_dir().to_path_buf())
            .ok_or(ConfigError::NoConfigDir)
    }

    fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::data_dir()?.join("config.json"))
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }
}
```

**Step 2: Add association methods to client**

Add to `src/keepassxc/client.rs` (append to impl block):
```rust
    /// Send an encrypted message and receive decrypted response
    fn send_encrypted<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &mut self,
        action: &str,
        payload: &T,
    ) -> Result<R, ClientError> {
        let payload_json = serde_json::to_string(payload)?;
        let (encrypted, nonce) = self.session_keys.encrypt(payload_json.as_bytes())?;

        let msg = Message {
            action: action.to_string(),
            message: Some(encrypted),
            nonce: Some(nonce),
            client_id: Some(self.client_id.clone()),
            public_key: None,
            success: None,
            version: None,
            error: None,
            error_code: None,
        };

        self.send(&msg)?;
        let resp = self.recv()?;

        if let Some(error) = resp.error {
            if error.contains("locked") {
                return Err(ClientError::DatabaseLocked);
            }
            return Err(ClientError::Protocol(error));
        }

        let resp_message = resp
            .message
            .ok_or_else(|| ClientError::Protocol("No message in response".to_string()))?;
        let resp_nonce = resp
            .nonce
            .ok_or_else(|| ClientError::Protocol("No nonce in response".to_string()))?;

        let decrypted = self.session_keys.decrypt(&resp_message, &resp_nonce)?;
        let response: R = serde_json::from_slice(&decrypted)?;
        Ok(response)
    }

    /// Associate with KeePassXC (first-time setup)
    pub fn associate(&mut self) -> Result<(String, String), ClientError> {
        // Generate a new identification key pair for permanent storage
        let id_keys = SessionKeys::generate();
        let id_key_b64 = id_keys.client_public_key_b64();

        let request = AssociateRequest {
            action: "associate".to_string(),
            key: self.session_keys.client_public_key_b64(),
            id_key: id_key_b64.clone(),
        };

        let response: AssociateResponse = self.send_encrypted("associate", &request)?;

        if response.success.as_deref() != Some("true") {
            return Err(ClientError::Protocol(
                response.error.unwrap_or_else(|| "Association failed".to_string()),
            ));
        }

        let id = response
            .id
            .ok_or_else(|| ClientError::Protocol("No ID in associate response".to_string()))?;

        Ok((id, id_key_b64))
    }

    /// Test if an existing association is still valid
    pub fn test_associate(&mut self, id: &str, id_key: &str) -> Result<bool, ClientError> {
        let request = TestAssociateRequest {
            action: "test-associate".to_string(),
            id: id.to_string(),
            key: id_key.to_string(),
        };

        let response: TestAssociateResponse = self.send_encrypted("test-associate", &request)?;
        Ok(response.success.as_deref() == Some("true"))
    }
```

**Step 3: Update main.rs to test association**

Update `src/main.rs`:
```rust
mod config;
mod keepassxc;

use config::Config;
use keepassxc::KeePassXCClient;

fn main() {
    println!("kpick - KeePassXC password picker");

    let mut config = Config::load().expect("Failed to load config");

    let mut client = match KeePassXCClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Check if we have an existing association
    if let Some(ref assoc) = config.association {
        match client.test_associate(&assoc.id, &assoc.id_key) {
            Ok(true) => {
                println!("Association valid: {}", assoc.id);
            }
            Ok(false) | Err(_) => {
                println!("Association invalid, need to re-associate");
                config.association = None;
            }
        }
    }

    // Associate if needed
    if config.association.is_none() {
        println!("Associating with KeePassXC (check KeePassXC for prompt)...");
        match client.associate() {
            Ok((id, id_key)) => {
                println!("Associated as: {}", id);
                config.association = Some(config::Association { id, id_key });
                config.save().expect("Failed to save config");
            }
            Err(e) => {
                eprintln!("Association failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}
```

**Step 4: Test manually**

Run: `cargo run`
Expected: KeePassXC shows association prompt, user approves, association saved

**Step 5: Commit**

```bash
git add src/
git commit -m "feat: add KeePassXC association and config persistence"
```

---

## Task 6: KeePassXC Client - Get Logins

**Files:**
- Modify: `src/keepassxc/client.rs`
- Modify: `src/main.rs`

**Step 1: Add get_logins method**

Add to `src/keepassxc/client.rs` (append to impl block):
```rust
    /// Get all login entries from KeePassXC
    pub fn get_logins(&mut self, id: &str, id_key: &str) -> Result<Vec<LoginEntry>, ClientError> {
        let request = GetLoginsRequest {
            action: "get-logins".to_string(),
            url: "kpick://all".to_string(), // Special URL to get all entries
            keys: vec![DatabaseKey {
                id: id.to_string(),
                key: id_key.to_string(),
            }],
        };

        let response: GetLoginsResponse = self.send_encrypted("get-logins", &request)?;

        if response.success.as_deref() != Some("true") {
            if let Some(error) = response.error {
                if error.contains("locked") {
                    return Err(ClientError::DatabaseLocked);
                }
                return Err(ClientError::Protocol(error));
            }
        }

        Ok(response.entries)
    }
```

**Step 2: Update main.rs to fetch entries**

Update `src/main.rs`:
```rust
mod config;
mod keepassxc;

use config::Config;
use keepassxc::KeePassXCClient;

fn main() {
    println!("kpick - KeePassXC password picker");

    let mut config = Config::load().expect("Failed to load config");

    let mut client = match KeePassXCClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Check if we have an existing association
    if let Some(ref assoc) = config.association {
        match client.test_associate(&assoc.id, &assoc.id_key) {
            Ok(true) => {
                println!("Association valid: {}", assoc.id);
            }
            Ok(false) | Err(_) => {
                println!("Association invalid, need to re-associate");
                config.association = None;
            }
        }
    }

    // Associate if needed
    if config.association.is_none() {
        println!("Associating with KeePassXC (check KeePassXC for prompt)...");
        match client.associate() {
            Ok((id, id_key)) => {
                println!("Associated as: {}", id);
                config.association = Some(config::Association { id, id_key });
                config.save().expect("Failed to save config");
            }
            Err(e) => {
                eprintln!("Association failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Fetch entries
    let assoc = config.association.as_ref().unwrap();
    match client.get_logins(&assoc.id, &assoc.id_key) {
        Ok(entries) => {
            println!("\nFound {} entries:", entries.len());
            for entry in entries.iter().take(10) {
                println!("  {} - {}", entry.name, entry.login);
            }
            if entries.len() > 10 {
                println!("  ... and {} more", entries.len() - 10);
            }
        }
        Err(e) => {
            eprintln!("Failed to get logins: {}", e);
            std::process::exit(1);
        }
    }
}
```

**Step 3: Test manually**

Run: `cargo run`
Expected: Lists entries from KeePassXC database

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: add get_logins to fetch entries from KeePassXC"
```

---

## Task 7: Fuzzy Search with Frecency

**Files:**
- Create: `src/search.rs`
- Modify: `src/main.rs`

**Step 1: Implement fuzzy search module**

Create `src/search.rs`:
```rust
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::keepassxc::LoginEntry;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrecencyData {
    pub entries: HashMap<String, FrecencyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrecencyEntry {
    pub count: u32,
    pub last_used: u64,
}

impl FrecencyData {
    pub fn record_use(&mut self, uuid: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let entry = self.entries.entry(uuid.to_string()).or_insert(FrecencyEntry {
            count: 0,
            last_used: now,
        });
        entry.count += 1;
        entry.last_used = now;
    }

    pub fn score(&self, uuid: &str) -> f64 {
        let Some(entry) = self.entries.get(uuid) else {
            return 0.0;
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let age_hours = (now - entry.last_used) as f64 / 3600.0;

        // Frecency: count * recency_decay
        // Recency decays by half every 24 hours
        let recency = 0.5_f64.powf(age_hours / 24.0);
        entry.count as f64 * recency
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: LoginEntry,
    pub score: u32,
    pub frecency: f64,
}

pub struct Searcher {
    matcher: Matcher,
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    pub fn search(
        &mut self,
        query: &str,
        entries: &[LoginEntry],
        frecency: &FrecencyData,
    ) -> Vec<SearchResult> {
        if query.is_empty() {
            // Return all entries sorted by frecency
            let mut results: Vec<_> = entries
                .iter()
                .map(|e| SearchResult {
                    entry: e.clone(),
                    score: 0,
                    frecency: frecency.score(&e.uuid),
                })
                .collect();
            results.sort_by(|a, b| b.frecency.partial_cmp(&a.frecency).unwrap());
            return results;
        }

        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut results = Vec::new();
        let mut buf = Vec::new();

        for entry in entries {
            // Search in name and login
            let search_text = format!("{} {}", entry.name, entry.login);
            let haystack = Utf32Str::new(&search_text, &mut buf);

            if let Some(score) = pattern.score(haystack, &mut self.matcher) {
                results.push(SearchResult {
                    entry: entry.clone(),
                    score,
                    frecency: frecency.score(&entry.uuid),
                });
            }
            buf.clear();
        }

        // Sort by: fuzzy score (primary) + frecency boost
        results.sort_by(|a, b| {
            let a_combined = a.score as f64 + a.frecency * 10.0;
            let b_combined = b.score as f64 + b.frecency * 10.0;
            b_combined.partial_cmp(&a_combined).unwrap()
        });

        results
    }
}
```

**Step 2: Update main.rs with search test**

Update `src/main.rs`:
```rust
mod config;
mod keepassxc;
mod search;

use config::Config;
use keepassxc::KeePassXCClient;
use search::{FrecencyData, Searcher};

fn main() {
    println!("kpick - KeePassXC password picker");

    let mut config = Config::load().expect("Failed to load config");

    let mut client = match KeePassXCClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Check/create association (same as before)
    if let Some(ref assoc) = config.association {
        if !client.test_associate(&assoc.id, &assoc.id_key).unwrap_or(false) {
            config.association = None;
        }
    }

    if config.association.is_none() {
        println!("Associating with KeePassXC (check KeePassXC for prompt)...");
        let (id, id_key) = client.associate().expect("Association failed");
        config.association = Some(config::Association { id, id_key });
        config.save().expect("Failed to save config");
    }

    // Fetch entries
    let assoc = config.association.as_ref().unwrap();
    let entries = client
        .get_logins(&assoc.id, &assoc.id_key)
        .expect("Failed to get logins");

    // Test search
    let frecency = FrecencyData::default();
    let mut searcher = Searcher::new();

    let query = std::env::args().nth(1).unwrap_or_default();
    let results = searcher.search(&query, &entries, &frecency);

    println!("\nSearch results for '{}':", query);
    for result in results.iter().take(10) {
        println!(
            "  [{}] {} - {}",
            result.score, result.entry.name, result.entry.login
        );
    }
}
```

**Step 3: Test manually**

Run: `cargo run git`
Expected: Shows filtered entries matching "git"

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: add fuzzy search with frecency ranking"
```

---

## Task 8: Clipboard Management

**Files:**
- Create: `src/clipboard.rs`
- Modify: `src/main.rs`

**Step 1: Implement clipboard module**

Create `src/clipboard.rs`:
```rust
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
```

**Step 2: Update main.rs to copy password**

Update `src/main.rs`:
```rust
mod clipboard;
mod config;
mod keepassxc;
mod search;

use clipboard::copy_with_clear;
use config::Config;
use keepassxc::KeePassXCClient;
use search::{FrecencyData, Searcher};

fn main() {
    println!("kpick - KeePassXC password picker");

    let mut config = Config::load().expect("Failed to load config");

    let mut client = match KeePassXCClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Check/create association
    if let Some(ref assoc) = config.association {
        if !client.test_associate(&assoc.id, &assoc.id_key).unwrap_or(false) {
            config.association = None;
        }
    }

    if config.association.is_none() {
        println!("Associating with KeePassXC (check KeePassXC for prompt)...");
        let (id, id_key) = client.associate().expect("Association failed");
        config.association = Some(config::Association { id, id_key });
        config.save().expect("Failed to save config");
    }

    // Fetch entries
    let assoc = config.association.as_ref().unwrap();
    let entries = client
        .get_logins(&assoc.id, &assoc.id_key)
        .expect("Failed to get logins");

    // Search
    let frecency = FrecencyData::default();
    let mut searcher = Searcher::new();

    let query = std::env::args().nth(1).unwrap_or_default();
    let results = searcher.search(&query, &entries, &frecency);

    if results.is_empty() {
        println!("No matching entries found");
        return;
    }

    // For now, just copy the first result
    let selected = &results[0];
    println!("Copying password for: {} - {}", selected.entry.name, selected.entry.login);

    if let Err(e) = copy_with_clear(&selected.entry.password, 10) {
        eprintln!("Failed to copy to clipboard: {}", e);
        std::process::exit(1);
    }

    println!("Password copied! (will clear in 10 seconds)");
}
```

**Step 3: Test manually**

Run: `cargo run github`
Expected: Copies first matching password, clears after 10s

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: add clipboard management with auto-clear"
```

---

## Task 9: Layer Shell UI Setup

**Files:**
- Create: `src/ui/mod.rs`
- Create: `src/ui/wayland.rs`
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

**Step 1: Add wayland-specific dependencies**

Update `Cargo.toml` dependencies:
```toml
# Add to existing dependencies
wayland-client = "0.31"
wayland-protocols = { version = "0.32", features = ["client", "staging"] }
wayland-protocols-wlr = { version = "0.3", features = ["client"] }
```

**Step 2: Create UI module structure**

Create `src/ui/mod.rs`:
```rust
pub mod wayland;
```

**Step 3: Implement basic layer-shell window**

Create `src/ui/wayland.rs`:
```rust
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

pub struct AppState {
    pub running: bool,
    pub width: u32,
    pub height: u32,

    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: Option<SlotPool>,

    layer_shell: LayerShell,
    layer_surface: Option<LayerSurface>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,

    // Input state
    pub query: String,
    pub selected_index: usize,

    // Callback for when user makes a selection
    pub on_select: Option<Box<dyn FnMut(usize)>>,
    pub on_escape: Option<Box<dyn FnMut()>>,
    pub on_query_change: Option<Box<dyn FnMut(&str)>>,
}

impl AppState {
    pub fn new(conn: &Connection, qh: &QueueHandle<Self>) -> Self {
        let (globals, event_queue) = registry_queue_init::<Self>(conn).unwrap();
        let registry_state = RegistryState::new(&globals);

        let shm = Shm::bind(&globals, qh).expect("wl_shm not available");
        let compositor = CompositorState::bind(&globals, qh).expect("wl_compositor not available");
        let layer_shell = LayerShell::bind(&globals, qh).expect("layer shell not available");
        let seat_state = SeatState::new(&globals, qh);
        let output_state = OutputState::new(&globals, qh);

        // Create our surface
        let surface = compositor.create_surface(qh);

        // Create layer surface
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("kpick"),
            None,
        );

        // Configure layer surface
        layer_surface.set_anchor(Anchor::TOP);
        layer_surface.set_size(600, 400);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.set_margin(100, 0, 0, 0); // 100px from top
        layer_surface.commit();

        Self {
            running: true,
            width: 600,
            height: 400,
            registry_state,
            seat_state,
            output_state,
            shm,
            pool: None,
            layer_shell,
            layer_surface: Some(layer_surface),
            keyboard: None,
            pointer: None,
            query: String::new(),
            selected_index: 0,
            on_select: None,
            on_escape: None,
            on_query_change: None,
        }
    }
}

// Implement all the required handlers
impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.running = false;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 > 0 {
            self.width = configure.new_size.0;
        }
        if configure.new_size.1 > 0 {
            self.height = configure.new_size.1;
        }

        // Create buffer pool if needed
        if self.pool.is_none() {
            self.pool = Some(
                SlotPool::new(self.width as usize * self.height as usize * 4, &self.shm)
                    .expect("Failed to create pool"),
            );
        }

        // Initial draw
        self.draw(qh, layer);
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = Some(self.seat_state.get_keyboard(qh, &seat, None).unwrap());
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = Some(self.seat_state.get_pointer(qh, &seat).unwrap());
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        _capability: Capability,
    ) {
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

impl KeyboardHandler for AppState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        match event.keysym {
            Keysym::Escape => {
                if let Some(ref mut cb) = self.on_escape {
                    cb();
                }
                self.running = false;
            }
            Keysym::Return | Keysym::KP_Enter => {
                if let Some(ref mut cb) = self.on_select {
                    cb(self.selected_index);
                }
                self.running = false;
            }
            Keysym::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Keysym::Down => {
                self.selected_index += 1;
            }
            Keysym::BackSpace => {
                self.query.pop();
                self.selected_index = 0;
                if let Some(ref mut cb) = self.on_query_change {
                    cb(&self.query);
                }
            }
            _ => {
                // Handle text input
                if let Some(c) = event.utf8.as_ref().and_then(|s| s.chars().next()) {
                    if c.is_ascii_graphic() || c == ' ' {
                        self.query.push(c);
                        self.selected_index = 0;
                        if let Some(ref mut cb) = self.on_query_change {
                            cb(&self.query);
                        }
                    }
                }
            }
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}

impl PointerHandler for AppState {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        _events: &[PointerEvent],
    ) {
    }
}

impl ShmHandler for AppState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers!(OutputState, SeatState);
}

impl AppState {
    fn draw(&mut self, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        let pool = self.pool.as_mut().unwrap();
        let stride = self.width as i32 * 4;
        let (buffer, canvas) = pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("Failed to create buffer");

        // Fill with dark background
        for pixel in canvas.chunks_exact_mut(4) {
            pixel[0] = 40;  // B
            pixel[1] = 40;  // G
            pixel[2] = 40;  // R
            pixel[3] = 255; // A
        }

        // TODO: Render egui here

        layer.wl_surface().attach(Some(buffer.wl_buffer()), 0, 0);
        layer.wl_surface().damage_buffer(0, 0, self.width as i32, self.height as i32);
        layer.wl_surface().commit();
    }
}

delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_shm!(AppState);
delegate_seat!(AppState);
delegate_keyboard!(AppState);
delegate_pointer!(AppState);
delegate_layer!(AppState);
delegate_registry!(AppState);
```

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles (may have warnings about unused)

**Step 5: Commit**

```bash
git add src/ui/ Cargo.toml
git commit -m "feat: add basic layer-shell window setup"
```

---

## Task 10: Integrate egui Rendering

**Files:**
- Create: `src/ui/egui_render.rs`
- Modify: `src/ui/mod.rs`
- Modify: `src/ui/wayland.rs`

**Step 1: Create egui rendering helper**

Create `src/ui/egui_render.rs`:
```rust
use egui::{Context, RawInput, Rect, Vec2};
use tiny_skia::{Color, Pixmap, PixmapMut, Transform};

pub struct EguiRenderer {
    ctx: Context,
    pixels_per_point: f32,
}

impl EguiRenderer {
    pub fn new() -> Self {
        Self {
            ctx: Context::default(),
            pixels_per_point: 1.0,
        }
    }

    pub fn render(
        &mut self,
        width: u32,
        height: u32,
        mut run_ui: impl FnMut(&Context),
    ) -> Pixmap {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Default::default(),
                Vec2::new(width as f32, height as f32) / self.pixels_per_point,
            )),
            ..Default::default()
        };

        let full_output = self.ctx.run(input, |ctx| {
            run_ui(ctx);
        });

        let clipped_primitives = self.ctx.tessellate(full_output.shapes, self.pixels_per_point);

        // Create pixmap and render
        let mut pixmap = Pixmap::new(width, height).unwrap();
        pixmap.fill(Color::from_rgba8(40, 40, 40, 255));

        // Use egui_skia or manual rendering here
        // For MVP, we'll do simple rectangle drawing
        for clipped in &clipped_primitives {
            // Basic rendering - full implementation would use epaint
            let _ = clipped;
        }

        pixmap
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }
}
```

**Step 2: Update UI mod**

Update `src/ui/mod.rs`:
```rust
pub mod egui_render;
pub mod wayland;

pub use wayland::AppState;
```

**Note:** Full egui rendering integration is complex. For MVP, we'll use a simpler approach with direct pixel drawing in the next task.

**Step 3: Commit**

```bash
git add src/ui/
git commit -m "feat: add egui renderer scaffold"
```

---

## Task 11: Simple Text Rendering with tiny-skia

**Files:**
- Modify: `src/ui/wayland.rs`
- Modify: `Cargo.toml`

**Step 1: Add fontdue for text rendering**

Update `Cargo.toml`:
```toml
fontdue = "0.9"
```

**Step 2: Update wayland.rs with text rendering**

Add to top of `src/ui/wayland.rs`:
```rust
use fontdue::{Font, FontSettings};
```

Add font field to AppState and implement text rendering in draw():

This is a substantial change - see the full implementation that draws:
- Search input box with query text
- List of entries with selected highlight
- Keyboard hint at bottom

**Step 3: Test manually**

Run: `cargo run`
Expected: Shows layer-shell window with UI

**Step 4: Commit**

```bash
git add src/ui/ Cargo.toml
git commit -m "feat: add text rendering for UI"
```

---

## Task 12: Wire Everything Together

**Files:**
- Modify: `src/main.rs`

**Step 1: Create full application flow**

Update `src/main.rs` to:
1. Connect to KeePassXC
2. Load entries
3. Show UI
4. Handle selection
5. Copy password

**Step 2: Add frecency persistence**

Update `src/config.rs` to load/save frecency data.

**Step 3: Test full flow**

Run: `cargo run`
Expected: Full password picker flow works

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: wire up full kpick application flow"
```

---

## Task 13: Error Handling and Polish

**Files:**
- Modify: `src/main.rs`
- Modify: `src/ui/wayland.rs`

**Step 1: Add user-friendly error messages**

- KeePassXC not running: "KeePassXC is not running. Please start it first."
- Database locked: "Please unlock your KeePassXC database."
- No entries: "No password entries found."

**Step 2: Handle edge cases**

- Empty database
- Search with no results
- Association cancelled

**Step 3: Test error paths**

Run various error scenarios manually.

**Step 4: Commit**

```bash
git add src/
git commit -m "feat: add error handling and user-friendly messages"
```

---

## Summary

This plan implements kpick in 13 tasks:

1. **Project skeleton** - Cargo.toml and basic structure
2. **Protocol types** - KeePassXC message definitions
3. **Crypto layer** - NaCl encryption for protocol
4. **Client connection** - Socket and key exchange
5. **Association** - Database pairing flow
6. **Get logins** - Fetch password entries
7. **Fuzzy search** - nucleo-based search with frecency
8. **Clipboard** - Copy with auto-clear
9. **Layer shell** - Wayland overlay window
10. **egui scaffold** - Rendering setup
11. **Text rendering** - Display entries
12. **Integration** - Wire everything together
13. **Polish** - Error handling

Each task is atomic and commits independently. The KeePassXC client can be tested without UI, and UI can be developed with mock data.
