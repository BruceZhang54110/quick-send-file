use clap::Parser;
use clap::Subcommand;
use iroh_blobs::ticket::BlobTicket;
use std::path::PathBuf;

/// parser cli command for send and receive file
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {

    #[clap(subcommand)]
    pub command: Commands,
    
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    // send file
    Send(SendArgs),
    // receive file
    Receive(ReceiveArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct SendArgs {

    // 文件路径
    #[clap(short, long, value_parser, default_value = None)]
    pub path: PathBuf,

}

#[derive(Parser, Debug, Clone)]
pub struct ReceiveArgs {
    // 文件分享码
    #[clap(short, long, value_parser, default_value = None)]
    pub code: BlobTicket,
}