//! Private authenticated networking. `Network` is a bounded handle; `actor` owns the swarm.
mod actor;
mod behaviour;
mod codec;
mod config;
mod discovery;
mod handle;
mod identity;
mod status;

pub use actor::start;
pub use config::NetworkConfig;
pub use handle::{Incoming, Network};
pub use identity::{keypair, peer_id, validate_card};
pub use status::{ConnectionStatus, NetworkStatus};
