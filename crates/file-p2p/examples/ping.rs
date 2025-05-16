use anyhow::Result;

use libp2p::{noise, ping, tcp, yamux};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // create new identity
    
    let swarm = libp2p::SwarmBuilder::with_new_identity().with_tokio()
    // Transport
    .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
        // Network behaviour
        .with_behaviour(|_| ping::Behaviour::default())?.build();
    
    

    Ok(())
}