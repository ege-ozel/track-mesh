//! HTTP authentication is separate from existing envelope/signing serialization.
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_MAILBOX_AUTH_BYTES: usize = 4096;
pub const MAX_MAILBOX_ENVELOPE_BYTES: usize = 128 * 1024;
pub const MAX_MAILBOX_PAGE_RECORDS: usize = 32;
pub const MAX_MAILBOX_PAGE_BYTES: usize = 5 * 1024 * 1024;

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
