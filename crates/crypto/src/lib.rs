use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use protocol::{
    Announcement, DeviceCard, EndpointRecord, Envelope, Manifest, SignedSnapshot, PROTOCOL_VERSION,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};
mod mailbox;
mod pairing;
pub use mailbox::verify_mailbox_auth;
pub use pairing::{pairing_challenge, pairing_proof, verify_pairing_proof};

pub struct Identity {
    pub device_id: Uuid,
    pub name: String,
    signing_key: SigningKey,
}
#[derive(Serialize, Deserialize)]
struct IdentityData {
    device_id: Uuid,
    name: String,
    secret: [u8; 32],
}

impl Identity {
    pub fn load_or_create(directory: &Path, name: &str) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let path = directory.join("identity.protected");
        if !path.exists() {
            let key = SigningKey::generate(&mut OsRng);
            let mut data = IdentityData {
                device_id: Uuid::new_v4(),
                name: name.chars().take(80).collect(),
                secret: key.to_bytes(),
            };
            let serialized = Zeroizing::new(serde_json::to_vec(&data)?);
            data.secret.zeroize();
            write_secret(&path, &serialized)?;
        }
        let serialized = read_secret(&path)?;
        let mut data: IdentityData = serde_json::from_slice(&serialized)?;
        let signing_key = SigningKey::from_bytes(&data.secret);
        data.secret.zeroize();
        Ok(Self {
            device_id: data.device_id,
            name: data.name,
            signing_key,
        })
    }
    pub fn secret_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.signing_key.to_bytes())
    }
    pub fn public_key(&self) -> String {
        hex::encode(self.signing_key.verifying_key().as_bytes())
    }
    pub fn sign(&self, message: &[u8]) -> String {
        hex::encode(self.signing_key.sign(message).to_bytes())
    }
    pub fn sign_card(&self, peer_id: String, addresses: Vec<String>) -> Result<DeviceCard> {
        let mut card = DeviceCard {
            version: PROTOCOL_VERSION,
            device_id: self.device_id,
            name: self.name.clone(),
            peer_id,
            public_key: self.public_key(),
            addresses,
            signature: String::new(),
        };
        card.signature = self.sign(&card.signing_bytes()?);
        Ok(card)
    }
    pub fn sign_endpoint(&self, mut record: EndpointRecord) -> Result<EndpointRecord> {
        ensure!(
            record.device_id == self.device_id,
            "endpoint device id does not match identity"
        );
        record.signature = self.sign(&record.signing_bytes()?);
        Ok(record)
    }
    pub fn sign_current_endpoint(
        &self,
        peer_id: String,
        addresses: Vec<String>,
        seq: u64,
    ) -> Result<EndpointRecord> {
        let issued_at = chrono::Utc::now();
        self.sign_endpoint(EndpointRecord {
            device_id: self.device_id,
            peer_id,
            addresses,
            issued_at,
            expires_at: issued_at + protocol::PREFERRED_ENDPOINT_TTL,
            seq,
            signature: String::new(),
        })
    }
    pub fn sign_snapshot(&self, manifest: Manifest) -> Result<SignedSnapshot> {
        manifest.validate()?;
        let mut snapshot = SignedSnapshot {
            manifest_hash: manifest.digest()?,
            manifest,
            author_public_key: self.public_key(),
            signature: String::new(),
        };
        snapshot.signature = self.sign(&snapshot.signing_bytes()?);
        snapshot.validate_structure()?;
        Ok(snapshot)
    }
    pub fn seal(&self, snapshot: &SignedSnapshot, key: &[u8; 32]) -> Result<Envelope> {
        let m = &snapshot.manifest;
        let announcement = Announcement {
            parent_snapshot_id: m.parent_snapshot_id,
            manifest_hash: snapshot.manifest_hash.clone(),
            total_size: m.total_size,
            changed_bytes: m.changed_bytes,
            file_count: m.files.len() as u32,
            author_name: self.name.clone(),
            description: "Saved project snapshot".into(),
        };
        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let mut envelope = Envelope {
            version: PROTOCOL_VERSION,
            project_id: m.project_id,
            snapshot_id: m.snapshot_id,
            author_device_id: self.device_id,
            created_at: m.created_at,
            nonce: STANDARD.encode(nonce),
            encrypted_payload: String::new(),
            public_key: self.public_key(),
            signature: String::new(),
        };
        let plaintext = Zeroizing::new(serde_json::to_vec(&announcement)?);
        let ciphertext = XChaCha20Poly1305::new(key.into())
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: &envelope.aad()?,
                },
            )
            .map_err(|_| anyhow::anyhow!("metadata encryption failed"))?;
        envelope.encrypted_payload = STANDARD.encode(ciphertext);
        envelope.signature = self.sign(&envelope.signing_bytes()?);
        Ok(envelope)
    }
}

pub fn verify(public_key: &str, message: &[u8], signature: &str) -> Result<()> {
    let key: [u8; 32] = hex::decode(public_key)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid public key"))?;
    let sig: [u8; 64] = hex::decode(signature)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid signature"))?;
    VerifyingKey::from_bytes(&key)?
        .verify_strict(message, &Signature::from_bytes(&sig))
        .context("signature verification failed")
}

pub fn verify_endpoint(
    record: &EndpointRecord,
    public_key: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    record.validate(now)?;
    verify(public_key, &record.signing_bytes()?, &record.signature)
}
pub fn verify_endpoint_current(record: &EndpointRecord, public_key: &str) -> Result<()> {
    verify_endpoint(record, public_key, chrono::Utc::now())
}
pub fn verify_snapshot(snapshot: &SignedSnapshot) -> Result<()> {
    snapshot.validate_structure()?;
    verify(
        &snapshot.author_public_key,
        &snapshot.signing_bytes()?,
        &snapshot.signature,
    )
}
pub fn verify_envelope(envelope: &Envelope) -> Result<()> {
    ensure!(
        envelope.version == PROTOCOL_VERSION,
        "unsupported metadata version"
    );
    ensure!(
        envelope.encrypted_payload.len() <= protocol::MAX_MAILBOX_PAYLOAD,
        "metadata too large"
    );
    ensure!(
        STANDARD.decode(&envelope.nonce)?.len() == 24,
        "invalid nonce"
    );
    verify(
        &envelope.public_key,
        &envelope.signing_bytes()?,
        &envelope.signature,
    )
}
pub fn open(envelope: &Envelope, key: &[u8; 32]) -> Result<Announcement> {
    verify_envelope(envelope)?;
    let nonce = STANDARD.decode(&envelope.nonce)?;
    let ciphertext = STANDARD.decode(&envelope.encrypted_payload)?;
    let plaintext = Zeroizing::new(
        XChaCha20Poly1305::new(key.into())
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &envelope.aad()?,
                },
            )
            .map_err(|_| anyhow::anyhow!("metadata authentication failed"))?,
    );
    let announcement: Announcement = serde_json::from_slice(&plaintext)?;
    ensure!(
        protocol::valid_hash(&announcement.manifest_hash),
        "invalid announced manifest hash"
    );
    ensure!(
        announcement.parent_snapshot_id != Some(envelope.snapshot_id),
        "invalid announced parent"
    );
    ensure!(
        announcement.changed_bytes <= announcement.total_size
            && announcement.total_size <= protocol::MAX_PROJECT_BYTES
            && announcement.file_count as usize <= protocol::MAX_FILES,
        "invalid announced size"
    );
    ensure!(
        announcement.author_name.len() <= 320 && announcement.description.len() <= 4096,
        "metadata text too long"
    );
    Ok(announcement)
}
pub fn project_key(
    directory: &Path,
    project: Uuid,
    supplied: Option<&[u8; 32]>,
) -> Result<Zeroizing<[u8; 32]>> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!("{project}.protected"));
    if path.exists() {
        let bytes = read_secret(&path)?;
        let key: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .context("invalid protected project key")?;
        if let Some(supplied) = supplied {
            ensure!(
                &key == supplied,
                "project key differs from the existing collaboration key"
            );
        }
        return Ok(Zeroizing::new(key));
    }
    let mut key = Zeroizing::new([0u8; 32]);
    if let Some(supplied) = supplied {
        *key = *supplied;
    } else {
        OsRng.fill_bytes(&mut *key);
    }
    write_secret(&path, &*key)?;
    Ok(key)
}

pub fn write_secret(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let protected = protect(bytes)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&protected)?;
    file.sync_all()?;
    Ok(())
}
pub fn read_secret(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    unprotect(&fs::read(path)?)
}

#[cfg(windows)]
fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into()?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // Current-user DPAPI, UI forbidden; never use CRYPTPROTECT_LOCAL_MACHINE.
    unsafe {
        ensure!(
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output
            ) != 0,
            "DPAPI protect failed: {}",
            std::io::Error::last_os_error()
        );
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Ok(result)
    }
}
#[cfg(windows)]
fn unprotect(bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into()?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        ensure!(
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output
            ) != 0,
            "DPAPI unlock failed: {}",
            std::io::Error::last_os_error()
        );
        let result = Zeroizing::new(
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec(),
        );
        std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize).zeroize();
        LocalFree(output.pbData as _);
        Ok(result)
    }
}
#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>> {
    anyhow::bail!("TrackMesh device secrets require Windows DPAPI")
}
#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    anyhow::bail!("TrackMesh device secrets require Windows DPAPI")
}
