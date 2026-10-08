use anyhow::{ensure, Result};
use clap::{Args, Parser, Subcommand};
use std::{path::PathBuf, sync::Arc, time::Duration};
use trackmesh_core::{
    default_data_directory, peer::PeerRuntime, Engine, MailboxRuntime, PluginRuntime, SessionGrace,
};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "trackmesh-agent",
    about = "Private DAW project snapshots and transfers"
)]
struct Cli {
    #[arg(long, env = "TRACKMESH_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long, default_value = "My studio PC")]
    device_name: String,
    #[command(subcommand)]
    command: Command,
}
#[derive(Args)]
struct NetworkArgs {
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// An explicitly reachable libp2p address to advertise to the private rendezvous.
    #[arg(long)]
    external_address: Vec<libp2p::Multiaddr>,
}
#[derive(Args)]
struct MailboxArgs {
    /// Development only: allow plain HTTP to a literal loopback IP.
    #[arg(long)]
    allow_loopback_http: bool,
}
impl NetworkArgs {
    fn config(self) -> network::NetworkConfig {
        network::NetworkConfig {
            port: self.port,
            external_addresses: self.external_address,
            ..Default::default()
        }
    }
}
#[derive(Subcommand)]
enum Command {
    Link {
        #[arg(long)]
        project_id: Uuid,
        #[arg(long)]
        name: String,
        #[arg(long)]
        folder: PathBuf,
        #[arg(long)]
        mailbox_url: Option<String>,
        #[arg(long)]
        relay_address: Option<String>,
    },
    Snapshot {
        project_id: Uuid,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Local history and last verification; online status is obtained using `check`.
    Status,
    /// Publish up to 64 pending authored announcements; marks PUBLISHED only after ACK.
    Publish {
        project_id: Uuid,
        #[command(flatten)]
        mailbox: MailboxArgs,
    },
    /// Fetch verified encrypted announcements. Does not download files or advance local head.
    Refresh {
        project_id: Uuid,
        #[command(flatten)]
        mailbox: MailboxArgs,
    },
    Card {
        #[arg(long)]
        address: Vec<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Pair {
        project_id: Uuid,
        #[arg(long)]
        card_file: PathBuf,
    },
    Revoke {
        project_id: Uuid,
        #[arg(long)]
        device_id: Uuid,
    },
    Invite {
        project_id: Uuid,
        #[arg(long)]
        address: Vec<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Join {
        #[arg(long)]
        invite_file: PathBuf,
        #[arg(long)]
        folder: PathBuf,
    },
    /// Send invitation possession proof automatically; the inviter approves in Desktop.
    RequestPairing {
        project_id: Uuid,
        #[arg(long)]
        device_id: Uuid,
        #[arg(long, default_value_t = 30)]
        connect_timeout: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    Serve {
        #[command(flatten)]
        network: NetworkArgs,
        #[command(flatten)]
        mailbox: MailboxArgs,
    },
    /// Local VST owner; exits 60 seconds after sessions, work and publication finish.
    PluginServe {
        #[command(flatten)]
        network: NetworkArgs,
        #[command(flatten)]
        mailbox: MailboxArgs,
        #[arg(long, env = "TRACKMESH_IPC_SUFFIX", default_value = "")]
        ipc_suffix: String,
    },
    Check {
        project_id: Uuid,
        #[arg(long)]
        device_id: Uuid,
        #[arg(long, default_value_t = 30)]
        connect_timeout: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Close the DAW before receiving. Repeat with the same snapshot UUID to resume.
    Receive {
        project_id: Uuid,
        #[arg(long)]
        device_id: Uuid,
        #[arg(long)]
        snapshot_id: Option<Uuid>,
        #[arg(long, default_value_t = 30)]
        connect_timeout: u64,
        #[command(flatten)]
        network: NetworkArgs,
    },
    /// Finish an interrupted file apply from the durable local journal. Close the DAW first.
    Recover { project_id: Uuid },
}
fn output_json(value: &impl serde::Serialize, output: Option<PathBuf>) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    if let Some(output) = output {
        std::fs::write(&output, json)?;
        eprintln!("Wrote {}", output.display());
    } else {
        println!("{json}");
    }
    Ok(())
}
fn read_json<T: serde::de::DeserializeOwned>(path: PathBuf) -> Result<T> {
    ensure!(
        std::fs::metadata(&path)?.len() <= 64 * 1024,
        "pairing document is too large"
    );
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "trackmesh_network=info,trackmesh_transfer=info,trackmesh_core=info".into()
            }),
        )
        .init();
    let cli = Cli::parse();
    let engine = Arc::new(Engine::open(
        cli.data_dir
            .map(Ok)
            .unwrap_or_else(default_data_directory)?,
        &cli.device_name,
    )?);
    match cli.command {
        Command::Link {
            project_id,
            name,
            folder,
            mailbox_url,
            relay_address,
        } => {
            engine.link(
                project_id,
                &name,
                &folder,
                mailbox_url.as_deref(),
                relay_address.as_deref(),
            )?;
            println!("Linked {project_id}");
        }
        Command::Snapshot { project_id, output } => {
            output_json(&engine.snapshot(project_id)?, output)?
        }
        Command::Status => output_json(&engine.local_status()?, None)?,
        Command::Publish {
            project_id,
            mailbox,
        } => output_json(
            &engine
                .publish_metadata(project_id, mailbox.allow_loopback_http)
                .await?,
            None,
        )?,
        Command::Refresh {
            project_id,
            mailbox,
        } => output_json(
            &engine
                .refresh_metadata(project_id, mailbox.allow_loopback_http)
                .await?,
            None,
        )?,
        Command::Card { address, output } => output_json(&engine.device_card(address)?, output)?,
        Command::Pair {
            project_id,
            card_file,
        } => {
            engine.pair(project_id, read_json(card_file)?)?;
            println!("Device paired with {project_id}");
        }
        Command::Revoke {
            project_id,
            device_id,
        } => {
            engine.revoke(project_id, device_id)?;
            println!("Device authorization removed from {project_id}");
        }
        Command::Invite {
            project_id,
            address,
            output,
        } => output_json(&engine.export_invite(project_id, address)?, output)?,
        Command::Join {
            invite_file,
            folder,
        } => {
            engine.import_invite(read_json(invite_file)?, &folder)?;
            println!("Project joined; export this device's card for the inviter to pair");
        }
        Command::RequestPairing {
            project_id,
            device_id,
            connect_timeout,
            network,
        } => {
            ensure!(
                (1..=60).contains(&connect_timeout),
                "pairing connection timeout must be 1–60 seconds"
            );
            let peer = engine.paired_peer(project_id, device_id)?;
            let runtime = PeerRuntime::start(engine.clone(), network.config())?;
            let operation = async {
                runtime.wait_for_peer(&peer.peer_id, Duration::from_secs(connect_timeout)).await?;
                let addresses = runtime.network.status().addresses.into_iter()
                    .filter(|a| !a.contains("/ip4/0.0.0.0/") && !a.contains("/ip6/::/"))
                    .take(16).collect();
                let response = engine.request_pairing(&runtime.network, project_id, device_id, addresses).await?;
                match response {
                    protocol::PairingResponse::Pending => output_json(&serde_json::json!({"pairing_state": "pending", "message": "Request delivered automatically. Wait for the inviter to verify and approve this device; no card file handoff is needed."}), None),
                    protocol::PairingResponse::Approved => output_json(&serde_json::json!({"pairing_state": "approved", "message": "The inviter authorized this device; check and receive when their snapshot is ready."}), None),
                    _ => anyhow::bail!("pairing unavailable; ask the inviter to open a pairing window in a current Desktop build and retry"),
                }
            }.await;
            let shutdown = runtime.stop().await;
            operation?;
            shutdown?;
        }
        Command::Recover { project_id } => {
            engine.recover(project_id)?;
            println!("Recovery complete for {project_id}");
        }
        Command::PluginServe {
            network,
            mailbox,
            ipc_suffix,
        } => {
            let runtime = PeerRuntime::start(engine.clone(), network.config())?;
            let plugin = match PluginRuntime::start(
                engine.clone(),
                runtime.network.clone(),
                mailbox.allow_loopback_http,
                &ipc_suffix,
            ) {
                Ok(plugin) => plugin,
                Err(error) => {
                    runtime.stop().await?;
                    return Err(error);
                }
            };
            let metadata = MailboxRuntime::start(engine.clone(), mailbox.allow_loopback_http);
            let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
            let mut grace = SessionGrace::default();
            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        let disk = engine.clone();
                        let pending = tokio::task::spawn_blocking(move || disk.has_pending_publication())
                            .await.ok().and_then(Result::ok).unwrap_or(true);
                        if plugin.is_finished() || runtime.is_finished() || metadata.is_finished()
                            || (grace.ready(&plugin, &runtime, pending) && plugin.close_if_idle()) { break; }
                    },
                    _ = tokio::signal::ctrl_c() => break,
                }
            }
            let plugin_shutdown = plugin.stop().await;
            let metadata_shutdown = metadata.stop().await;
            let peer_shutdown = runtime.stop().await;
            plugin_shutdown?;
            metadata_shutdown?;
            peer_shutdown?;
        }
        Command::Serve { network, mailbox } => {
            let runtime = PeerRuntime::start(engine.clone(), network.config())?;
            let metadata = MailboxRuntime::start(engine.clone(), mailbox.allow_loopback_http);
            println!("PeerId: {}", runtime.network.status().peer_id);
            let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _=heartbeat.tick()=>{if runtime.is_finished() || metadata.is_finished() {break;}},
                    _=tokio::signal::ctrl_c()=>break,
                }
            }
            let metadata_shutdown = metadata.stop().await;
            let peer_shutdown = runtime.stop().await;
            metadata_shutdown?;
            peer_shutdown?;
        }
        Command::Check {
            project_id,
            device_id,
            connect_timeout,
            network,
        } => {
            let peer = engine.paired_peer(project_id, device_id)?;
            let runtime = PeerRuntime::start(engine.clone(), network.config())?;
            let operation = async {
                runtime
                    .wait_for_peer(&peer.peer_id, Duration::from_secs(connect_timeout))
                    .await?;
                output_json(
                    &engine
                        .check_peer(&runtime.network, project_id, &peer.peer_id)
                        .await?,
                    None,
                )
            }
            .await;
            let shutdown = runtime.stop().await;
            operation?;
            shutdown?;
        }
        Command::Receive {
            project_id,
            device_id,
            snapshot_id,
            connect_timeout,
            network,
        } => {
            let peer = engine.paired_peer(project_id, device_id)?;
            let runtime = PeerRuntime::start(engine.clone(), network.config())?;
            let result = {
                let receive = async {
                    runtime
                        .wait_for_peer(&peer.peer_id, Duration::from_secs(connect_timeout))
                        .await?;
                    engine
                        .receive_from_peer(&runtime.network, project_id, peer, snapshot_id)
                        .await
                };
                tokio::pin!(receive);
                tokio::select! {
                    result=&mut receive=>result,
                    _=tokio::signal::ctrl_c()=>{
                        eprintln!("Stopping transfer; a file apply already in progress will finish safely.");
                        runtime.network.shutdown().await;
                        receive.await
                    }
                }
            };
            let connection = runtime.network.status();
            let shutdown = runtime.stop().await;
            let id = result?;
            shutdown?;
            output_json(
                &serde_json::json!({"transfer_id":id,"transfer":engine.store.transfers()?.into_iter().find(|t|t.transfer_id==id),"last_verified_at":engine.store.project(project_id)?.last_verified_at,"network":connection}),
                None,
            )?;
        }
    }
    Ok(())
}
