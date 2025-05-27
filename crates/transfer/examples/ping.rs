use anyhow::Result;

use libp2p::{noise, ping, tcp, yamux, Multiaddr};
use tracing_subscriber::EnvFilter;
use futures::prelude::*;
use libp2p::swarm::SwarmEvent;
use std::time::Duration;


/// cargo run --package transfer --example ping
#[tokio::main]
async fn main() -> Result<()> {
    // 日志追踪默认设置
    let _  = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // create new identity
    let mut swarm = libp2p::SwarmBuilder::with_new_identity().with_tokio()
    // Transport
    .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
        // Network behaviour
        .with_behaviour(|_| ping::Behaviour::default())?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
        .build();
    
    // Tell the swarm to listen on all interfaces and a random, OS-assigned
    // port.
    // 监听
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Dial the peer identified by the multi-address given as the second
    // command-line argument, if any.
    if let Some(addr) = std::env::args().nth(1) {
        let remote: Multiaddr = addr.parse()?;
        // 拨号
        swarm.dial(remote)?;
        println!("Dialed {addr}");
    }

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {address:?}"),
            SwarmEvent::ConnectionEstablished { peer_id, connection_id, endpoint, num_established, concurrent_dial_errors, established_in }
                =>  {
                    println!("connection established: {:?}, {:?}", peer_id, connection_id);
                },
                
            SwarmEvent::Behaviour(event) => println!("{event:?}"),
            _ => {}
        }
    }

}