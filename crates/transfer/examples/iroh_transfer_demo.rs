use std::path::PathBuf;

use anyhow::{Ok, Result};
use iroh::{protocol::Router, Endpoint};
use iroh_blobs::{net_protocol::Blobs, rpc::client::blobs::WrapOption, ticket::BlobTicket, util::SetTagOption};
use iroh_blobs::store::{ExportFormat, ExportMode};

#[tokio::main]
async fn main() -> Result<()> {

    // Create an endpoint, it allows creating and accepting
    // connections in the iroh p2p world
    let endpoint = Endpoint::builder().discovery_n0().bind().await?;

    // We initialize the Blobs protocol in-memory
    let blobs = Blobs::memory().build(&endpoint);

    // Now we build a router that accepts blobs connections & routes them
    // to the blobs protocol.
    let router= Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();


    // do *something*
    let blobs_client = blobs.client();

    // get cli args
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Convert to &str, so we can pattern-match easily
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
match arg_refs.as_slice() {
    ["send", filename] => {
        let filename: PathBuf = filename.parse()?;
        let abs_path = std::path::absolute(&filename)?;
        println!("Hashing file.");

        let in_place = true;
        // keep the file in place and link it, instead of copying it into the in-meoory blobs database
        let blob = blobs_client
            .add_from_path(abs_path, in_place, SetTagOption::Auto, WrapOption::NoWrap)
            .await?
            .finish()
            .await?;
        
        let node_id = router.endpoint().node_id();
        let ticket = BlobTicket::new(node_id.into(), blob.hash, blob.format)?;
        println!("File Hashed. Fetch this file by runing:");
        println!(
                "cargo run --example transfer -- receive {ticket} {}",
                filename.display()
            );
        tokio::signal::ctrl_c().await?;
    }
    ["receive", ticket, filename] => {
        let filename: PathBuf = filename.parse()?;
        let abs_path = std::path::absolute(filename)?;
        let ticket: BlobTicket = ticket.parse()?;

        println!("Starting download.");

        blobs_client
                .download(ticket.hash(), ticket.node_addr().clone())
                .await?
                .finish()
                .await?;
        println!("Finished download.");
        println!("Copying to destination.");

        blobs_client
            .export(
                ticket.hash(),
                abs_path,
                ExportFormat::Blob,
                ExportMode::Copy,
            )
            .await?
            .finish()
            .await?;

        println!("Finished copying.");
        
    }
    _ => {
        println!("Couldn't parse command line arguments: {args:?}");
        println!("Usage:");
        println!("    # to send:");
        println!("    cargo run --example transfer -- send [FILE]");
        println!("    # this will print a ticket.");
        println!();
        println!("    # to receive:");
        println!("    cargo run --example transfer -- receive [TICKET] [FILE]");
    }
}

    // Gracefully shut down the router
    println!("Shutting down.");
    router.shutdown().await?;
    Ok(())
}