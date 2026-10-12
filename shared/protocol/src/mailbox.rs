//! HTTP authentication is separate from existing envelope/signing serialization.
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_MAILBOX_AUTH_BYTES: usize = 4096;
pub const MAX_MAILBOX_ENVELOPE_BYTES: usize = 128 * 1024;
pub const MAX_MAILBOX_PAGE_RECORDS: usize = 32;
pub const MAX_MAILBOX_PAGE_BYTES: usize = 5 * 1024 * 1024;
pub const SUPABASE_REQUEST_DOMAIN: &str = "trackmesh-supabase-request-v1";
pub const ENDPOINT_MAILBOX_AAD: &[u8] = b"trackmesh-endpoint-mailbox-v1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupabaseAuth {
    pub version: u32,
    pub project_route: String,
    pub device_id: String,
    pub public_key: String,
    pub issued_at: String,
    pub nonce: String,
    pub scope: String,
    pub body_hash: String,
    pub signature: String,
}

impl SupabaseAuth {
    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&(
            SUPABASE_REQUEST_DOMAIN,
            self.version,
            &self.project_route,
            &self.device_id,
            &self.public_key,
            &self.issued_at,
            &self.nonce,
            &self.scope,
            &self.body_hash,
        ))?)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointMailboxWrapper {
    pub version: u32,
    pub project_route: String,
    pub writer_device_id: String,
    pub seq: String,
    pub expires_at: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupabaseEndpointRow {
    pub cursor: String,
    pub writer_device_id: String,
    pub seq: String,
    pub ciphertext: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupabaseAccepted {
    pub accepted: bool,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailboxAuth {
    pub version: u32,
    pub project_id: Uuid,
    pub device_id: Uuid,
    pub public_key: String,
    pub issued_at: DateTime<Utc>,
    pub nonce: Uuid,
    pub scope: String,
    pub body_hash: String,
    pub signature: String,
}

impl MailboxAuth {
    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&(
            "trackmesh-mailbox-request-v1",
            self.version,
            self.project_id,
            self.device_id,
            &self.public_key,
            self.issued_at,
            self.nonce,
            &self.scope,
            &self.body_hash,
        ))?)
    }
}

pub fn mailbox_scope(cursor: Option<i64>) -> String {
    match cursor {
        Some(cursor) => format!("GET announcements after {cursor}"),
        None => "POST announcement".into(),
    }
}
