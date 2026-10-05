//! Connection reporting. Online equality is recorded by core workflows, not inferred here.
use libp2p::{swarm::ConnectionId, PeerId};
use serde::Serialize;
use std::{collections::HashMap, sync::RwLock};

pub(super) type Connections = HashMap<ConnectionId, (PeerId, bool, String)>;
#[derive(Clone, Debug, Serialize, Default)]
pub struct NetworkStatus {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub peers: HashMap<String, ConnectionStatus>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ConnectionStatus {
    pub online: bool,
    pub route: String,
    pub transport: String,
}
pub(super) fn refresh_status(
    status: &RwLock<NetworkStatus>,
    connections: &HashMap<ConnectionId, (PeerId, bool, String)>,
) {
    if let Ok(mut status) = status.write() {
        for state in status.peers.values_mut() {
            state.online = false;
            state.route = "OFFLINE".into();
        }
        for (peer, relayed, transport) in connections.values() {
            let state = status
                .peers
                .entry(peer.to_string())
                .or_insert(ConnectionStatus {
                    online: false,
                    route: String::new(),
                    transport: String::new(),
                });
            if !state.online || !relayed {
                state.online = true;
                state.route = if *relayed { "RELAYED" } else { "DIRECT" }.into();
                state.transport = transport.clone();
            }
        }
    }
}
