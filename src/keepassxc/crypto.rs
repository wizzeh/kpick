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
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    BASE64.encode(bytes)
}

/// Generate a random nonce (24 bytes, base64 encoded)
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    BASE64.encode(bytes)
}
