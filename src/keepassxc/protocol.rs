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
#[allow(dead_code)]
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
