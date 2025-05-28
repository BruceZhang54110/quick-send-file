use anyhow::Result;
use clap::Parser;
use transfer::{cli::{Args, Commands}, transfer::{receive_file, send_file}};
use tracing_subscriber::{EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志，设置日志级别为 info
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
            .with_line_number(true)
        .init();

    // 1. parse cli agrs
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(cause) => {
            panic!("command is error: {}", cause);
        }
    };

    let res = match args.command {
        Commands::Send(args) => send_file(args).await,
        Commands::Receive(args) => receive_file(args).await,
    };

    if let Err(e) = & res {
        eprintln!("{e}");
    }

    match res {
        Ok(()) => std::process::exit(0),
        Err(_) => std::process::exit(1),
        
    }

}