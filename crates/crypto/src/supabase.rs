//! Project-key encrypted wrappers for the opaque Supabase endpoint mailbox.
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use chrono::{SecondsFormat, Utc};
use protocol::{EndpointMailboxWrapper, EndpointRecord, ENDPOINT_MAILBOX_AAD};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

const WRAPPER_VERSION: u32 = 1;
const MAX_WRAPPER_BYTES: usize = 64 * 1024;

pub fn endpoint_record_expires_at(record: &EndpointRecord) -> String {
    record
        .expires_at
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

impl super::Identity {
    /// Sign the exact M5d request tuple consumed by Supabase Edge Functions.
    pub fn supabase_auth(
        &self,
        project: uuid::Uuid,
        scope: &str,
        body: &[u8],
    ) -> Result<protocol::SupabaseAuth> {
        ensure!(!project.is_nil(), "Supabase project route cannot be nil");
        ensure!(
            matches!(
                scope,
                "project-claim"
                    | "member-grant"
                    | "member-revoke"
                    | "endpoint-publish"
                    | "endpoint-list"
            ),
            "invalid Supabase request scope"
        );
        let mut auth = protocol::SupabaseAuth {
            version: protocol::PROTOCOL_VERSION,
            project_route: project.to_string(),
            device_id: self.device_id.to_string(),
            public_key: self.public_key(),
            issued_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            nonce: uuid::Uuid::new_v4().to_string(),
            scope: scope.to_owned(),
            body_hash: hex::encode(Sha256::digest(body)),
            signature: String::new(),
        };
        auth.signature = self.sign(&auth.signing_bytes()?);
        Ok(auth)
    }
}

pub fn wrap_endpoint_record(
    project_route: uuid::Uuid,
    record: &EndpointRecord,
    project_key: &[u8; 32],
) -> Result<String> {
    record.validate(Utc::now())?;
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let plaintext = serde_json::to_vec(record)?;
    let ciphertext = XChaCha20Poly1305::new(project_key.into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: ENDPOINT_MAILBOX_AAD,
            },
        )
        .map_err(|_| anyhow::anyhow!("endpoint wrapper encryption failed"))?;
    let wrapper = EndpointMailboxWrapper {
        version: WRAPPER_VERSION,
        project_route: project_route.to_string(),
        writer_device_id: record.device_id.to_string(),
        seq: record.seq.to_string(),
        expires_at: endpoint_record_expires_at(record),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    };
    let serialized = serde_json::to_vec(&wrapper)?;
    ensure!(
        serialized.len() <= MAX_WRAPPER_BYTES,
        "endpoint mailbox wrapper too large"
    );
    Ok(URL_SAFE_NO_PAD.encode(serialized))
}

pub fn unwrap_endpoint_record(
    encoded_wrapper: &str,
    project_route: uuid::Uuid,
    writer_device_id: uuid::Uuid,
    seq: &str,
    expires_at: &str,
    project_key: &[u8; 32],
) -> Result<EndpointRecord> {
    ensure!(
        encoded_wrapper.len() <= MAX_WRAPPER_BYTES * 2,
        "endpoint mailbox wrapper too large"
    );
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded_wrapper)
        .context("invalid endpoint wrapper encoding")?;
    ensure!(
        bytes.len() <= MAX_WRAPPER_BYTES,
        "endpoint mailbox wrapper too large"
    );
    let wrapper: EndpointMailboxWrapper =
        serde_json::from_slice(&bytes).context("invalid endpoint wrapper")?;
    ensure!(
        wrapper.version == WRAPPER_VERSION
            && wrapper.project_route == project_route.to_string()
            && wrapper.writer_device_id == writer_device_id.to_string()
            && wrapper.seq == seq
            && wrapper.expires_at == expires_at,
        "endpoint wrapper metadata mismatch"
    );
    let nonce: [u8; 24] = URL_SAFE_NO_PAD
        .decode(wrapper.nonce)
        .context("invalid endpoint wrapper nonce")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid endpoint wrapper nonce length"))?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(wrapper.ciphertext)
        .context("invalid endpoint wrapper ciphertext")?;
    let plaintext = XChaCha20Poly1305::new(project_key.into())
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: ENDPOINT_MAILBOX_AAD,
            },
        )
        .map_err(|_| anyhow::anyhow!("endpoint wrapper authentication failed"))?;
    let record: EndpointRecord =
        serde_json::from_slice(&plaintext).context("invalid wrapped endpoint record")?;
    ensure!(
        record.device_id == writer_device_id
            && record.seq.to_string() == seq
            && record
                .expires_at
                .to_rfc3339_opts(SecondsFormat::Millis, true)
                == expires_at,
        "wrapped endpoint metadata mismatch"
    );
    Ok(record)
}
