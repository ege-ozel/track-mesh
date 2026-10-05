//! Signed application identity <-> libp2p PeerId binding and device-card validation.
use anyhow::{ensure, Result};
use crypto::Identity;
use libp2p::{identity, multiaddr::Protocol, Multiaddr};
use protocol::DeviceCard;
pub fn keypair(identity: &Identity) -> Result<identity::Keypair> {
    Ok(identity::Keypair::ed25519_from_bytes(
        *identity.secret_bytes(),
    )?)
}
pub fn peer_id(identity: &Identity) -> Result<String> {
    Ok(keypair(identity)?.public().to_peer_id().to_string())
}
pub fn validate_card(card: &DeviceCard) -> Result<()> {
    protocol::require_protocol(card.version)?;
    ensure!(
        !card.name.trim().is_empty() && card.name.len() <= 320 && card.addresses.len() <= 16,
        "invalid device card"
    );
    crypto::verify(&card.public_key, &card.signing_bytes()?, &card.signature)?;
    let public = identity::ed25519::PublicKey::try_from_bytes(&hex::decode(&card.public_key)?)?;
    let public: identity::PublicKey = public.into();
    ensure!(
        public.to_peer_id().to_string() == card.peer_id,
        "PeerId does not match signed application identity"
    );
    for address in &card.addresses {
        ensure!(address.len() <= 1024, "address too long");
        let address: Multiaddr = address.parse()?;
        if let Some(Protocol::P2p(peer)) = address.iter().last() {
            ensure!(
                peer.to_string() == card.peer_id,
                "address belongs to another peer"
            );
        }
    }
    Ok(())
}
