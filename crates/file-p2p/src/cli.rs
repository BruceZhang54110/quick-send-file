use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;

/// parser cli command for send and receive file
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {

    #[clap(subcommand)]
    pub command: Commands,
    
}

#[derive(Subcommand, Debug)]
enum Commands {
    // send file
    Send(SendCommand),
    // receive file
    Receive(ReceiveCommand),
}

#[derive(Parser, Debug)]
struct SendCommand {

    // 文件路径
    #[clap(short, long, value_parser)]
    pub path: PathBuf,

    // 文件分享码
    #[clap(short, long, value_parser)]
    pub code: String,
}

#[derive(Parser, Debug)]
struct ReceiveCommand {
    // 文件路径
    #[clap(short, long, value_parser)]
    pub path: PathBuf,
}