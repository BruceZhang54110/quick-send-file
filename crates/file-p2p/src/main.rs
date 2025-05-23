use anyhow::{Ok, Result};
use clap::{Command, Parser};
use file_p2p::cli::Args;
use std::result::Result::Ok as Okk;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    // 1. parse cli agrs
    let args = match Args::try_parse() {
        Okk(args) => args,
        Err(cause) => {
            panic!("command is error: {}", cause);
        }
    };

    

    Ok(())
}